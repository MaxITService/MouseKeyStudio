//! The Apps dialog against a real `AppState`, opened from each editor's
//! profile bar the way a user opens it.

use gpui::{
    AppContext as _, Bounds, Context, Entity, IntoElement, Modifiers, ParentElement as _, Pixels,
    Render, ScrollDelta, ScrollWheelEvent, Size, Styled as _, Subscription, TestAppContext,
    TouchPhase, VisualTestContext, Window, div, point, px, size,
};
use gpui_component::Root;
use openlogi_core::binding::{Action, ActionRingLayout, ButtonId};
use openlogi_core::config::Config;

use super::super::catalog::{AppCatalogPicker, ProfileIconCache};
use super::super::{action_ring_profile_scope_bar, button_profile_scope_bar};
use crate::services::assets::AssetResolver;
use crate::services::i18n::LOCALE_LOCK;
use crate::state::tests::{KNOWN_MOUSE_KEY, direct_inventory};
use crate::state::{AppState, Sources};
use crate::ui::theme;

/// Another profile of the same device: never part of the selected one's list.
const OTHER: &str = r"c:\other vendor\other.exe";
/// A profile only the Actions Ring has.
const RING_ONLY: &str = r"c:\games\ring only.exe";

#[derive(Clone, Copy, Debug)]
enum Editor {
    Buttons,
    Ring,
}

impl Editor {
    fn apps_button(self) -> &'static str {
        match self {
            Self::Buttons => "button-profile:apps",
            Self::Ring => "action-ring-profile:apps",
        }
    }
}

/// An editor's profile bar over the dialog layer, as the detail page hosts it.
struct Host {
    editor: Editor,
    icons: ProfileIconCache,
    catalog: Entity<AppCatalogPicker>,
    _repaint: Subscription,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bar = match self.editor {
            Editor::Buttons => button_profile_scope_bar(&self.icons, &self.catalog, cx),
            Editor::Ring => action_ring_profile_scope_bar(&self.icons, &self.catalog, cx),
        };
        div()
            .size_full()
            .children(bar)
            .children(Root::render_dialog_layer(window, cx))
    }
}

/// A long, realistic executable path, lower-cased as Windows identifiers are.
fn executable(index: usize) -> String {
    format!(
        r"c:\program files\example vendor\creative suite {index:02}\application binaries\x64\editor-{index:02}.exe"
    )
}

/// The known mouse with one profile that `count` executables select, saved
/// for both editors, plus [`OTHER`] and [`RING_ONLY`]. Returns the profile key.
fn config(count: usize) -> (Config, String) {
    let mut config = Config::ephemeral();
    config.set_per_app_binding(
        KNOWN_MOUSE_KEY,
        &executable(0),
        ButtonId::Back,
        Some(Action::Undo),
    );
    let mut key = executable(0);
    for index in 1..count {
        key = config
            .assign_profile_application(&key, "Editor", &executable(index))
            .expect("fixture assignment");
    }
    config.set_per_app_binding(KNOWN_MOUSE_KEY, OTHER, ButtonId::Back, Some(Action::Copy));
    let ring = &mut config
        .devices
        .get_mut(KNOWN_MOUSE_KEY)
        .expect("fixture device")
        .action_ring
        .per_app;
    ring.insert(key.clone(), ActionRingLayout::default());
    ring.insert(RING_ONLY.into(), ActionRingLayout::default());
    (config, key)
}

fn open_editor<'a>(
    cx: &'a mut TestAppContext,
    config: Config,
    editor: Editor,
    profile: Option<&str>,
    window: Size<Pixels>,
) -> &'a mut VisualTestContext {
    let profile = profile.map(str::to_string);
    cx.update(|cx| {
        gpui_component::init(cx);
        theme::register_builtin_themes(cx);
        cx.set_reduce_motion(true);
        let resolver = AssetResolver::new();
        let (commands, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut state = AppState::new(Sources {
            inventories: &[direct_inventory([0xa3, 0x93, 0xca, 0xe0])],
            ..Sources::in_memory(config, &resolver, commands)
        });
        let _ = match editor {
            Editor::Buttons => state.set_editing_app(profile),
            Editor::Ring => state.set_editing_action_ring_app(profile),
        };
        AppState::set_global(cx.new(|_| state), cx);
    });
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            let icons = ProfileIconCache::default();
            let catalog = cx.new(|cx| AppCatalogPicker::new(icons.clone(), window, cx));
            Host {
                editor,
                icons,
                catalog,
                _repaint: AppState::repaint_on(cx, |_| true),
            }
        });
        Root::new(view, window, cx)
    });
    visual.simulate_resize(window);
    redraw(visual);
    visual
}

fn redraw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn selector(prefix: &str, identifier: &str) -> &'static str {
    Box::leak(format!("{prefix}:{identifier}").into_boxed_str())
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} renders"))
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let center = bounds(cx, selector).center();
    cx.simulate_click(center, Modifiers::none());
    redraw(cx);
}

fn listed(cx: &mut VisualTestContext, identifier: &str) -> bool {
    cx.debug_bounds(selector("assigned-app-path", identifier))
        .is_some()
}

const WINDOW: Size<Pixels> = Size {
    width: px(1000.),
    height: px(720.),
};

#[gpui::test]
fn duplicate_profile_selects_an_unassigned_copy_in_the_invoking_editor(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    for editor in [Editor::Buttons, Editor::Ring] {
        for default in [false, true] {
            let (config, source) = config(3);
            let cx = open_editor(
                cx,
                config,
                editor,
                (!default).then_some(source.as_str()),
                WINDOW,
            );
            let trigger = match editor {
                Editor::Buttons => "button-profile:duplicate",
                Editor::Ring => "action-ring-profile:duplicate",
            };
            click(cx, trigger);
            cx.update(|_, cx| {
                let state = AppState::try_read(cx).unwrap();
                let selected = match editor {
                    Editor::Buttons => state.editing_app(),
                    Editor::Ring => state.editing_action_ring_app(),
                }
                .unwrap();
                assert_ne!(selected, source);
                let alias = state.app_aliases().for_profile_key(selected).unwrap().1;
                assert!(alias.identifiers_here().is_empty());
                assert!(alias.display_name().unwrap().contains("copy"));
            });
            click(cx, editor.apps_button());
            for index in 0..3 {
                assert!(!listed(cx, &executable(index)));
            }
            cx.update(|window, _| window.remove_window());
        }
    }
}

#[gpui::test]
fn apps_lists_only_the_profile_selected_in_the_editor_it_opens_from(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    for editor in [Editor::Buttons, Editor::Ring] {
        let (config, key) = config(3);
        let cx = open_editor(cx, config, editor, Some(&key), WINDOW);
        click(cx, editor.apps_button());
        for index in 0..3 {
            let identifier = executable(index);
            let path = bounds(cx, selector("assigned-app-path", &identifier));
            let copy = bounds(cx, selector("assigned-app-copy", &identifier));
            assert!(
                path.right() <= copy.left(),
                "{editor:?}: the full path wraps beside its commands instead of running under them"
            );
            assert!(
                path.size.height > px(20.),
                "{editor:?}: a long path wraps onto further lines rather than being clipped"
            );
        }
        for absent in [OTHER, RING_ONLY] {
            assert!(
                !listed(cx, absent),
                "{editor:?}: {absent} belongs to another profile"
            );
        }
        cx.update(|window, _| window.remove_window());
    }
}

#[gpui::test]
fn the_default_profile_offers_no_application_list(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    for editor in [Editor::Buttons, Editor::Ring] {
        let (config, _) = config(2);
        let cx = open_editor(cx, config, editor, None, WINDOW);
        assert!(
            cx.debug_bounds("profile-scope-bar").is_some(),
            "{editor:?}: the profile bar renders"
        );
        assert!(
            cx.debug_bounds(editor.apps_button()).is_none(),
            "{editor:?}: no application selects the Default profile"
        );
        cx.update(|window, _| window.remove_window());
    }
}

#[gpui::test]
fn a_long_list_scrolls_between_a_fixed_title_and_footer(cx: &mut TestAppContext) {
    const COUNT: usize = 40;
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let viewport = size(px(1000.), px(560.));
    let (config, key) = config(COUNT);
    let cx = open_editor(cx, config, Editor::Buttons, Some(&key), viewport);
    click(cx, Editor::Buttons.apps_button());

    let window = Bounds::new(point(px(0.), px(0.)), viewport);
    let done = bounds(cx, "assigned-apps-done");
    assert!(
        window.contains(&done.origin) && window.contains(&done.bottom_right()),
        "the footer stays inside the window: {done:?}"
    );
    let last = selector("assigned-app-path", &executable(COUNT - 1));
    assert!(
        bounds(cx, last).top() >= done.top(),
        "the list overflows the capped dialog body"
    );

    let first = bounds(cx, selector("assigned-app-path", &executable(0)));
    for _ in 0..20 {
        cx.simulate_event(ScrollWheelEvent {
            position: first.center(),
            delta: ScrollDelta::Lines(point(0., -10.)),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        redraw(cx);
    }
    let last = bounds(cx, last);
    let done_after = bounds(cx, "assigned-apps-done");
    assert_eq!(done_after, done, "scrolling moves the list, not the footer");
    assert!(
        last.bottom() <= done.top() && last.top() >= px(0.),
        "the last path scrolls fully into view above the footer: {last:?}"
    );
}

#[gpui::test]
fn copy_puts_the_full_path_on_the_clipboard_and_says_so(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let (config, key) = config(2);
    let cx = open_editor(cx, config, Editor::Buttons, Some(&key), WINDOW);
    click(cx, Editor::Buttons.apps_button());
    assert!(cx.debug_bounds("assigned-apps-status").is_none());

    let identifier = executable(1);
    click(cx, selector("assigned-app-copy", &identifier));
    assert_eq!(
        cx.read_from_clipboard().and_then(|item| item.text()),
        Some(identifier)
    );
    assert!(
        cx.debug_bounds("assigned-apps-status").is_some(),
        "the footer confirms a change the user cannot see"
    );
}

#[gpui::test]
fn done_and_escape_close_the_dialog(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let (config, key) = config(2);
    let cx = open_editor(cx, config, Editor::Buttons, Some(&key), WINDOW);
    click(cx, Editor::Buttons.apps_button());
    click(cx, "assigned-apps-done");
    assert!(cx.debug_bounds("assigned-apps-done").is_none());

    click(cx, Editor::Buttons.apps_button());
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(cx.debug_bounds("assigned-apps-done").is_none());
}

#[cfg(target_os = "windows")]
mod windows {
    use std::path::PathBuf;

    use super::*;

    fn identifiers(cx: &mut VisualTestContext, key: &str) -> Vec<String> {
        cx.update(|_, cx| {
            super::super::assigned_identifiers(AppState::try_read(cx).unwrap().app_aliases(), key)
        })
    }

    fn editing_app(cx: &mut VisualTestContext) -> Option<String> {
        cx.update(|_, cx| {
            AppState::try_read(cx)
                .unwrap()
                .editing_app()
                .map(str::to_string)
        })
    }

    fn pick(cx: &mut VisualTestContext, selection: Option<&str>) {
        click(cx, "assigned-apps-add");
        assert!(cx.did_prompt_for_paths(), "Add opens the executable picker");
        cx.simulate_path_prompt_response(|_| selection.map(|path| vec![PathBuf::from(path)]));
        redraw(cx);
    }

    #[gpui::test]
    fn add_attaches_an_executable_to_the_selected_profile_only(cx: &mut TestAppContext) {
        let _locale = LOCALE_LOCK.lock().unwrap();
        rust_i18n::set_locale("en");
        let (config, key) = config(1);
        assert_eq!(key, executable(0), "a single executable keys its profile");
        let cx = open_editor(cx, config, Editor::Buttons, Some(&key), WINDOW);
        click(cx, Editor::Buttons.apps_button());

        pick(cx, None);
        assert_eq!(
            editing_app(cx).as_deref(),
            Some(key.as_str()),
            "cancel changes nothing"
        );
        assert!(cx.debug_bounds("assigned-apps-status").is_none());

        pick(cx, Some(r"C:\Tools\New Tool.EXE"));
        let added = r"c:\tools\new tool.exe";
        let alias = editing_app(cx).expect("the editor keeps the profile selected");
        assert!(
            alias.starts_with("app:"),
            "a second executable turns the profile into an alias: {alias}"
        );
        assert_eq!(identifiers(cx, &alias), [executable(0), added.to_string()]);
        assert!(listed(cx, added) && listed(cx, &executable(0)));
        assert!(!listed(cx, OTHER), "another profile stays out of the list");
        assert_eq!(
            identifiers(cx, OTHER),
            [OTHER],
            "and keeps its own application"
        );
        assert!(cx.debug_bounds("assigned-apps-status").is_some());

        pick(cx, Some(r"C:\Tools\New Tool.exe"));
        assert_eq!(
            identifiers(cx, &alias),
            [executable(0), added.to_string()],
            "choosing a listed executable again is reported, not duplicated"
        );
        assert!(cx.debug_bounds("assigned-apps-status").is_some());
    }

    #[gpui::test]
    fn an_unsaved_profile_cannot_take_more_applications(cx: &mut TestAppContext) {
        let _locale = LOCALE_LOCK.lock().unwrap();
        rust_i18n::set_locale("en");
        let draft = r"c:\drafts\unsaved.exe";
        let (config, _) = config(1);
        let cx = open_editor(cx, config, Editor::Buttons, Some(draft), WINDOW);
        click(cx, Editor::Buttons.apps_button());
        assert!(listed(cx, draft), "the draft lists its own application");
        click(cx, "assigned-apps-add");
        assert!(
            !cx.did_prompt_for_paths(),
            "Add is disabled until the profile is saved"
        );
        cx.update(|_, cx| {
            assert!(
                AppState::try_read(cx)
                    .unwrap()
                    .app_aliases()
                    .for_profile_key(draft)
                    .is_none()
            );
        });
    }
}
