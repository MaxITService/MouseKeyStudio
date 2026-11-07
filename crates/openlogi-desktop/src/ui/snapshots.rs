//! Offscreen UI snapshots: render named scenes to PNG for visual review.
//!
//! A debug-only mode of the real desktop binary. It runs on the host's real
//! GPUI platform (DirectX / Metal / Vulkan text and rendering, the app's own
//! assets and themes), builds each scene's view in a fixed state, and saves
//! the frame with `Window::render_to_image` — no screen capture, no focus
//! stealing, no agent or hardware needed.
//!
//! ```sh
//! MKS_UI_SNAPSHOTS=macro cargo run -p openlogi-desktop --features ui-snapshots
//! ```
//!
//! - `MKS_UI_SNAPSHOTS`: comma-separated substrings of scene names;
//!   `all` (or empty) renders every scene. Unknown filters list the scenes.
//! - `MKS_UI_SNAPSHOT_THEMES`: `light`, `dark`, or `light,dark` (default).
//! - `MKS_UI_SNAPSHOT_SCALES`: UI scale percents, e.g. `100,125`
//!   (default `100`).
//! - `MKS_UI_SNAPSHOT_DIR`: output directory (default `target/ui-snapshots`).
//!
//! Files are named `<scene>--<theme>--<scale>.png`, with `/` in scene names
//! replaced by `_`. Scenes are registered by the module that owns the view
//! (they reach into private state the way that module's tests do); add one
//! there and list it in [`scenes`].

use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use anyhow::{Context as _, Result};
use gpui::{
    AnyView, AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, Context, IntoElement,
    ParentElement as _, Pixels, Render, Size, Styled as _, Window, WindowBounds, WindowKind,
    WindowOptions, point, px,
};
use gpui_component::{ActiveTheme as _, Root, Theme, ThemeMode, v_flex};
use openlogi_core::config::{Config, UiScale};

use crate::services::assets::AssetResolver;
use crate::state::{AppState, Sources};
use crate::ui::theme;

/// One reviewable UI state.
pub(crate) struct Scene {
    /// Stable, filterable name, `area/state` by convention.
    pub(crate) name: &'static str,
    /// Logical window size the scene is laid out in.
    pub(crate) size: Size<Pixels>,
    /// Build the view already driven into the state under review.
    pub(crate) build: fn(&mut Window, &mut App) -> AnyView,
}

fn scenes() -> Vec<Scene> {
    let mut all = Vec::new();
    all.extend(crate::features::mouse::snapshots::scenes());
    all.extend(crate::features::keyboard::function_row::snapshots::scenes());
    all.extend(crate::features::binding_editor::command_snapshot_scenes());
    all.extend(super::gallery::snapshot_scenes());
    all.extend(super::scroll::snapshots::scenes());
    all.extend(crate::windows::settings::snapshot_scenes());
    all
}

/// Frames a scene gets to settle (async image/SVG loads, deferred layout)
/// before it is captured.
const SETTLE: Duration = Duration::from_millis(600);

/// Run the snapshot mode until every selected scene is written, then quit.
pub(crate) fn run(filter: &str) -> Result<()> {
    rust_i18n::set_locale("en");
    let all = scenes();
    let filters: Vec<String> = filter
        .split(',')
        .map(|f| f.trim().to_ascii_lowercase())
        .filter(|f| !f.is_empty() && f != "all")
        .collect();
    let selected: Vec<Scene> = all
        .into_iter()
        .filter(|scene| filters.is_empty() || filters.iter().any(|f| scene.name.contains(f)))
        .collect();
    if selected.is_empty() {
        eprintln!("no scene matches {filter:?}; available scenes:");
        for scene in scenes() {
            eprintln!("  {}", scene.name);
        }
        anyhow::bail!("no snapshot scene matched");
    }

    let themes = themes_from_env()?;
    let scales = scales_from_env()?;
    let out_dir = std::env::var_os("MKS_UI_SNAPSHOT_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let expected = selected.len() * themes.len() * scales.len();
    let completed = Arc::new(AtomicUsize::new(0));
    let captured = Arc::clone(&completed);

    let app = gpui_platform::application().with_assets(crate::app_assets::AppAssets);
    app.run(move |cx| {
        gpui_component::init(cx);
        crate::ui::scroll::init(cx);
        theme::register_builtin_themes(cx);
        super::gallery::install_openlogi_themes(cx);
        install_app_state(Config::ephemeral(), cx);

        cx.spawn(async move |cx| {
            let mut failures = 0usize;
            // The Windows platform quits when its last window closes, so each
            // scene's window stays open until the next one exists.
            let mut previous: Option<AnyWindowHandle> = None;
            for scene in &selected {
                for &mode in &themes {
                    for &scale in &scales {
                        let file = out_dir.join(format!(
                            "{}--{}--{}.png",
                            scene.name.replace('/', "_"),
                            if mode.is_dark() { "dark" } else { "light" },
                            scale.percent()
                        ));
                        let result = capture(scene, mode, scale, &file, cx).await;
                        if let Some(window) = previous.take() {
                            let _ = cx.update_window(window, |_, window, _| window.remove_window());
                        }
                        match result {
                            Ok(window) => {
                                captured.fetch_add(1, Ordering::Relaxed);
                                previous = Some(window);
                                println!("snapshot: {}", file.display());
                            }
                            Err(error) => {
                                failures += 1;
                                eprintln!("snapshot {} failed: {error:#}", scene.name);
                            }
                        }
                    }
                }
            }
            if failures > 0 {
                eprintln!("{failures} snapshot(s) failed");
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    let completed = completed.load(Ordering::Relaxed);
    anyhow::ensure!(
        completed == expected,
        "captured {completed} of {expected} snapshots"
    );
    Ok(())
}

async fn capture(
    scene: &Scene,
    mode: ThemeMode,
    scale: UiScale,
    file: &std::path::Path,
    cx: &mut AsyncApp,
) -> Result<AnyWindowHandle> {
    let build = scene.build;
    let options = WindowOptions {
        // Shown (a hidden window never gets a drawable size) but cloaked and
        // unfocused, so it is never seen. The bounds start on a display, or
        // the platform swaps in its default size.
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.), px(0.)),
            size: scene.size,
        })),
        focus: false,
        show: true,
        // Borderless: a captioned window is clamped to the display's height,
        // and tall scenes need to show the whole panel.
        kind: WindowKind::PopUp,
        is_resizable: false,
        ..WindowOptions::default()
    };
    let handle = cx.update(|cx| {
        // Views apply the stored scale during render, including auxiliary
        // windows. Keep their source of truth in step with the capture size.
        AppState::apply(cx, |state| state.commit_ui_scale(scale));
        cx.open_window(options, move |window, cx| {
            cloak(window);
            Theme::change(mode, Some(window), cx);
            theme::apply_scale(window, scale);
            let scene = build(window, cx);
            let frame = cx.new(|_| SceneFrame(scene));
            cx.new(|cx| Root::new(frame, window, cx).bg(cx.theme().background))
        })
    })?;

    for _ in 0..2 {
        cx.background_executor().timer(SETTLE / 2).await;
        handle.update(cx, |_, window, _| {
            theme::apply_scale(window, scale);
            window.refresh();
        })?;
    }

    // Through the untyped handle: a typed `update` leases the `Root` entity,
    // and drawing renders that same entity.
    let image = cx.update_window(handle.into(), |_, window, cx| {
        anyhow::ensure!(
            window.viewport_size() == scene.size,
            "platform changed scene size from {:?} to {:?}",
            scene.size,
            window.viewport_size()
        );
        window.draw(cx).clear(cx);
        anyhow::ensure!(
            window.rem_size() == theme::rem_size(scale),
            "scene reset the requested interface scale"
        );
        window.render_to_image()
    })??;
    image
        .save(file)
        .with_context(|| format!("writing {}", file.display()))?;
    Ok(handle.into())
}

/// Keep a capture window off the screen while it still renders normally.
#[cfg(target_os = "windows")]
#[expect(unsafe_code, reason = "one DWM attribute write on our own HWND")]
fn cloak(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let cloaked: i32 = 1;
    // SAFETY: the HWND belongs to this live window, and the attribute takes a
    // BOOL-sized buffer that outlives the call.
    unsafe {
        DwmSetWindowAttribute(
            handle.hwnd.get() as _,
            DWMWA_CLOAK as u32,
            (&raw const cloaked).cast(),
            u32::try_from(size_of::<i32>()).unwrap_or_default(),
        );
    }
}

/// Other platforms render scenes in windows that may briefly appear.
#[cfg(not(target_os = "windows"))]
fn cloak(_: &Window) {}

/// Gives the scene the full-size flex column the app's windows give their
/// content, so views that fill with `flex_1`/`h_full` lay out as they ship.
struct SceneFrame(AnyView);

impl Render for SceneFrame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Component Root sets its default rem size before rendering its child.
        // Apply the app preference here, as the production window root does.
        theme::apply_ui_scale(window, cx);
        v_flex().size_full().child(self.0.clone())
    }
}

/// Scenes read and write [`AppState`] like the running app does, without an
/// agent, a config file, or devices.
pub(crate) fn install_app_state(config: Config, cx: &mut App) {
    let resolver = AssetResolver::new();
    // Commands go nowhere: a snapshot never talks to the agent.
    let (commands, receiver) = tokio::sync::mpsc::unbounded_channel();
    std::mem::forget(receiver);
    let state = cx.new(|_| AppState::new(Sources::in_memory(config, &resolver, commands)));
    AppState::set_global(state, cx);
}

fn themes_from_env() -> Result<Vec<ThemeMode>> {
    let raw = std::env::var("MKS_UI_SNAPSHOT_THEMES").unwrap_or_else(|_| "light,dark".into());
    raw.split(',')
        .map(|name| match name.trim() {
            "light" => Ok(ThemeMode::Light),
            "dark" => Ok(ThemeMode::Dark),
            other => anyhow::bail!("unknown theme {other:?}; use light or dark"),
        })
        .collect()
}

fn scales_from_env() -> Result<Vec<UiScale>> {
    let raw = std::env::var("MKS_UI_SNAPSHOT_SCALES").unwrap_or_else(|_| "100".into());
    raw.split(',')
        .map(|percent| {
            let percent: u16 = percent.trim().parse().context("scale must be a percent")?;
            UiScale::ALL
                .into_iter()
                .find(|scale| scale.percent() == percent)
                .with_context(|| format!("unsupported UI scale {percent}%"))
        })
        .collect()
}
