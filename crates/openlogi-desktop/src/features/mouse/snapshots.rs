//! Snapshot scenes for the mouse buttons panel and its binding editors.

use gpui::{AnyView, App, AppContext as _, Entity, Window, px, size};
use openlogi_core::binding::{ButtonId, Macro, MacroStep};

use super::hotspots::MouseControlId;
use super::view::MouseModelView;
use crate::state::AppState;
use crate::ui::snapshots::Scene;

/// A realistic three-phase macro, so every editor row has content.
fn sample_macro() -> Macro {
    // A typo here shows up as an empty phase in every macro snapshot.
    let steps = |text| MacroStep::parse_sequence(text).unwrap_or_default();
    Macro {
        name: "Crouch hop".into(),
        on_press: steps("+RightCtrl, 50ms, Space"),
        while_held: steps("W, 30ms"),
        on_release: steps("-RightCtrl"),
        repeat_while_held: true,
    }
}

fn mouse_view(window: &mut Window, cx: &mut App) -> Entity<MouseModelView> {
    cx.new(|cx| MouseModelView::new(window, cx))
}

/// The Forward button's action picker, open.
fn picker(window: &mut Window, cx: &mut App) -> Entity<MouseModelView> {
    let view = mouse_view(window, cx);
    view.update(cx, |view, cx| {
        view.select(MouseControlId::Button(ButtonId::Forward), window, cx);
    });
    view
}

/// The picker with a sample macro loaded into the step editor.
fn macro_steps(window: &mut Window, cx: &mut App) -> Entity<MouseModelView> {
    AppState::apply(cx, |state| {
        state.commit_binding(
            ButtonId::Forward,
            openlogi_core::binding::Action::Macro(Box::new(sample_macro())),
        )
    });
    let view = picker(window, cx);
    view.update(cx, |view, cx| {
        view.macro_draft.seed(&sample_macro(), window, cx);
    });
    view
}

pub(crate) fn scenes() -> Vec<Scene> {
    let wide = size(px(1280.), px(1500.));
    let narrow = size(px(760.), px(1500.));
    let mut scenes = vec![
        Scene {
            name: "mouse/macro-unsaved",
            size: wide,
            build: |window, cx| {
                let view = macro_steps(window, cx);
                view.update(cx, |view, cx| {
                    view.macro_draft.name_input.update(cx, |input, cx| {
                        input.set_value("Edited macro", window, cx);
                    });
                    view.macro_draft.pending_action =
                        Some(super::macro_editor::DraftChange::Assign(
                            openlogi_core::binding::Action::Copy,
                        ));
                    cx.notify();
                });
                view.into()
            },
        },
        Scene {
            name: "mouse/buttons",
            size: size(px(1280.), px(820.)),
            build: |window, cx| AnyView::from(mouse_view(window, cx)),
        },
        Scene {
            name: "mouse/picker",
            size: wide,
            build: |window, cx| AnyView::from(picker(window, cx)),
        },
        Scene {
            name: "mouse/macro-steps",
            size: wide,
            build: |window, cx| AnyView::from(macro_steps(window, cx)),
        },
        Scene {
            name: "mouse/macro-steps-narrow",
            size: narrow,
            build: |window, cx| macro_steps(window, cx).into(),
        },
        Scene {
            name: "mouse/macro-steps-long",
            size: wide,
            build: |window, cx| {
                let view = macro_steps(window, cx);
                view.update(cx, |view, cx| {
                    let mut long = sample_macro();
                    long.on_press = MacroStep::parse_sequence(
                        &["+LeftCtrl, 50ms, +C, 50ms, -C, 50ms, -LeftCtrl"; 4].join(", "),
                    )
                    .unwrap_or_default();
                    view.macro_draft.seed(&long, window, cx);
                });
                view.into()
            },
        },
        Scene {
            name: "mouse/macro-text",
            size: wide,
            build: |window, cx| {
                let view = macro_steps(window, cx);
                view.update(cx, |view, cx| view.macro_draft.open_text_editor(window, cx));
                view.into()
            },
        },
        Scene {
            name: "mouse/macro-text-error",
            size: wide,
            build: |window, cx| {
                let view = macro_steps(window, cx);
                view.update(cx, |view, cx| {
                    view.macro_draft.open_text_editor(window, cx);
                    let text = super::macro_text::format_macro(&sample_macro())
                        .replace("+RightCtrl", "+Ctlr");
                    view.macro_draft
                        .text
                        .input
                        .update(cx, |input, cx| input.set_value(text.clone(), window, cx));
                    view.macro_draft.text.error = super::macro_text::parse_macro(&text).err();
                });
                view.into()
            },
        },
        Scene {
            name: "mouse/macro-text-narrow",
            size: narrow,
            build: |window, cx| {
                let view = macro_steps(window, cx);
                view.update(cx, |view, cx| view.macro_draft.open_text_editor(window, cx));
                view.into()
            },
        },
    ];
    scenes.extend(profile_scenes());
    scenes.extend(guide_scenes());
    scenes.extend(tooltip_scenes());
    scenes.extend(scroll_scenes());
    scenes
}

fn profile_scenes() -> [Scene; 2] {
    [
        Scene {
            name: "mouse/g502-profile-diagnostic",
            size: size(px(1280.), px(548.)),
            build: gaming_profile,
        },
        Scene {
            name: "mouse/g502-profile-diagnostic-narrow",
            size: size(px(1000.), px(600.)),
            build: gaming_profile,
        },
    ]
}

/// A remembered gaming mouse, with the last game still identified after the
/// GUI takes focus. Everything is in memory; no agent or user config is read.
fn gaming_profile(window: &mut Window, cx: &mut App) -> AnyView {
    use openlogi_core::app::ForegroundApp;
    use openlogi_core::config::{Config, DeviceIdentity};
    use openlogi_core::device::{Capabilities, DeviceKind, DeviceModelInfo, DeviceTransports};
    use openlogi_ipc::ForegroundApps;

    let key = "unit:4f4c4701";
    let mut config = Config::ephemeral();
    config.app_settings.ui_scale =
        AppState::try_read(cx).map_or_else(Default::default, |state| state.app_settings().ui_scale);
    config.devices.entry(key.into()).or_default().identity = Some(DeviceIdentity {
        display_name: "G502 X LIGHTSPEED".into(),
        model_info: Some(DeviceModelInfo {
            entity_count: 1,
            serial_number: None,
            unit_id: [0x4f, 0x4c, 0x47, 0x01],
            model_ids: [0x409f, 0, 0],
            extended_model_id: 1,
            transports: DeviceTransports::default(),
        }),
        codename: Some("G502 X LIGHTSPEED".into()),
        kind: DeviceKind::Mouse,
        capabilities: Capabilities::from_feature_ids(&[0x8100, 0x8110]),
        light_capabilities: None,
        driver_id: None,
        registry_model_id: None,
    });
    let app = ForegroundApp {
        id: "exe:enshrouded.exe".into(),
        display_name: "Enshrouded".into(),
    };
    config.set_per_app_binding(
        key,
        &app.id,
        ButtonId::Back,
        Some(openlogi_core::binding::Action::MouseBack),
    );
    crate::ui::snapshots::install_app_state(config, cx);
    AppState::apply(cx, |state| {
        state.set_foreground(ForegroundApps {
            current: Some(app.clone()),
            recent: vec![app.clone()],
        })
    });
    AppState::apply(cx, |state| state.set_editing_app(Some(app.id.clone())));
    AppState::apply(cx, |state| {
        state.set_foreground(ForegroundApps {
            current: Some(ForegroundApp::unnamed(openlogi_core::brand::APP_ID.into())),
            recent: vec![app],
        })
    });
    mouse_view(window, cx).into()
}

fn scroll_scenes() -> Vec<Scene> {
    vec![
        Scene {
            name: "mouse/scroll-sidebar",
            size: size(px(1280.), px(820.)),
            build: scrolled_sidebar,
        },
        Scene {
            name: "mouse/scroll-sidebar-narrow",
            size: size(px(760.), px(640.)),
            build: scrolled_sidebar,
        },
    ]
}

/// Exercise the real wheel/animation path before capturing a scrolled panel.
fn scrolled_sidebar(window: &mut Window, cx: &mut App) -> AnyView {
    let view = macro_steps(window, cx);
    window
        .spawn(cx, async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let _ = cx.update(|window, cx| {
                use gpui::InputEvent as _;
                for _ in 0..24 {
                    window.dispatch_event(
                        gpui::ScrollWheelEvent {
                            position: gpui::point(px(8.), px(300.)),
                            delta: gpui::ScrollDelta::Lines(gpui::point(0., -1.)),
                            modifiers: gpui::Modifiers::default(),
                            touch_phase: gpui::TouchPhase::Moved,
                        }
                        .to_platform_input(),
                        cx,
                    );
                }
            });
        })
        .detach();
    view.into()
}

fn tooltip_scenes() -> Vec<Scene> {
    vec![
        Scene {
            name: "mouse/macro-tooltip-save",
            size: size(px(440.), px(160.)),
            build: |window, cx| {
                crate::ui::components::detailed_tooltip(
                    tr!("actions.macro_save_tooltip"),
                    window,
                    cx,
                )
            },
        },
        Scene {
            name: "profiles/duplicate-tooltip",
            size: size(px(440.), px(200.)),
            build: |window, cx| {
                crate::ui::components::detailed_tooltip(
                    tr!("profiles.duplicate_profile_tooltip"),
                    window,
                    cx,
                )
            },
        },
    ]
}

fn guide_scenes() -> Vec<Scene> {
    vec![
        Scene {
            name: "mouse/macro-guide-bottom",
            size: size(px(740.), px(680.)),
            build: |window, cx| guide_at(window, cx, 100_000.),
        },
        Scene {
            name: "mouse/macro-guide",
            size: size(px(740.), px(680.)),
            build: |window, cx| {
                cx.new(|cx| super::macro_guide::MacroGuide::new(window, cx))
                    .into()
            },
        },
        Scene {
            name: "mouse/macro-guide-1",
            size: size(px(740.), px(680.)),
            build: |window, cx| guide_at(window, cx, 900.),
        },
        Scene {
            name: "mouse/macro-guide-2",
            size: size(px(740.), px(680.)),
            build: |window, cx| guide_at(window, cx, 1800.),
        },
        Scene {
            name: "mouse/macro-guide-3",
            size: size(px(740.), px(680.)),
            build: |window, cx| guide_at(window, cx, 2700.),
        },
        Scene {
            name: "mouse/macro-guide-4",
            size: size(px(740.), px(680.)),
            build: |window, cx| guide_at(window, cx, 3600.),
        },
        Scene {
            name: "mouse/macro-guide-5",
            size: size(px(740.), px(680.)),
            build: |window, cx| guide_at(window, cx, 4500.),
        },
    ]
}

fn guide_at(window: &mut Window, cx: &mut App, offset: f32) -> AnyView {
    let view = cx.new(|cx| super::macro_guide::MacroGuide::new(window, cx));
    let scroll = view.read(cx).scroll.clone();
    window
        .spawn(cx, async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(250))
                .await;
            let _ = cx.update(|window, _| {
                scroll.set_offset(gpui::point(px(0.), px(-offset)));
                window.refresh();
            });
        })
        .detach();
    view.into()
}
