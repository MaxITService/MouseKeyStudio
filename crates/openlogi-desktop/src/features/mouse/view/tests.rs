use gpui::{Entity, KeyDownEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext, size};
use openlogi_core::binding::default_binding;
use openlogi_core::config::Config;

use super::ShortcutError::{ShiftedSymbol, Unrecognized};
use super::*;
use crate::features::mouse::macro_editor::STEP_LIST_MAX_H;
use crate::services::assets::AssetResolver;
use crate::services::i18n::LOCALE_LOCK;
use crate::state::Sources;

fn install_app_state(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let resolver = AssetResolver::new();
        let (commands, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let state =
            cx.new(|_| AppState::new(Sources::in_memory(Config::ephemeral(), &resolver, commands)));
        AppState::set_global(state, cx);
    });
}

#[gpui::test]
fn mouse_scope_change_discards_the_previous_macro_draft(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.update(|cx| {
        let resolver = AssetResolver::new();
        let (commands, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let inventory = crate::state::tests::direct_inventory([0x4f, 0x4c, 0x44, 0x02]);
        let state = cx.new(|_| {
            AppState::new(Sources {
                inventories: &[inventory],
                ..Sources::in_memory(Config::ephemeral(), &resolver, commands)
            })
        });
        AppState::set_global(state, cx);
    });
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.sync_selection(window, cx);
            view.selected = Some(MouseControlId::Button(ButtonId::Forward));
            view.action_picker_open = true;
            view.macro_draft
                .name_input
                .update(cx, |input, cx| input.set_value("Old profile", window, cx));
            AppState::apply(cx, |state| state.set_editing_app(Some("editor".into())));
            view.sync_selection(window, cx);
            assert!(
                !view.action_picker_open,
                "a draft cannot move to another profile"
            );
            assert_eq!(
                view.selected,
                Some(MouseControlId::Button(ButtonId::Forward))
            );
            assert_eq!(view.macro_draft.name_input.read(cx).value(), "");
            view.action_picker_open = true;
            AppState::apply(cx, |state| state.set_binding_layer(BindingLayer::GShift));
            view.sync_selection(window, cx);
            assert!(
                !view.action_picker_open,
                "a draft cannot move to another layer"
            );
        });
    });
}

fn click_picker_control(cx: &mut VisualTestContext, selector: &'static str) {
    // Debug bounds include offscreen elements. Scroll the owning inspector
    // before clicking, just as a user must in a short window or a long editor.
    for _ in 0..3 {
        let bounds = cx.debug_bounds(selector).expect(selector);
        let Some(inspector) = cx.debug_bounds("button-inspector") else {
            break;
        };
        if bounds.center().x < inspector.left()
            || bounds.center().x > inspector.right()
            || (bounds.top() >= inspector.top() && bounds.bottom() <= inspector.bottom())
        {
            break;
        }
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: gpui::point(inspector.left() + px(8.), inspector.center().y),
            delta: gpui::ScrollDelta::Pixels(gpui::point(
                px(0.),
                inspector.center().y - bounds.center().y,
            )),
            modifiers: Modifiers::default(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    let bounds = cx.debug_bounds(selector).expect(selector);
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Press `key` with Cmd+Shift held, in the order GPUI's platforms report a
/// shifted digit or symbol: a modifier change that holds Shift, then the
/// shifted character without Shift (#2121).
fn press_shifted_symbol(cx: &mut VisualTestContext, key: &str) {
    let cmd = Modifiers {
        platform: true,
        ..Modifiers::default()
    };
    cx.simulate_modifiers_change(Modifiers { shift: true, ..cmd });
    cx.simulate_event(KeyDownEvent {
        keystroke: Keystroke {
            modifiers: cmd,
            key: key.into(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_modifiers_change(Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn assert_forward_binding(cx: &mut VisualTestContext, expected: &Action) {
    cx.update(|_, cx| {
        assert_eq!(
            AppState::try_read(cx)
                .unwrap()
                .button_bindings()
                .get(&ButtonId::Forward),
            Some(expected)
        );
    });
}

#[gpui::test]
fn long_bindings_stay_inside_their_label_card(cx: &mut TestAppContext) {
    // #1401: the card's Button base centres its children on the cross axis,
    // so without `items_stretch` the value row keeps its natural width and
    // overflows both edges once the binding name is wider than the card. The
    // English defaults are enough to trip it under the test text system
    // ("Forward (Button 5)" measures a 206px row over a 156px card). Pinned
    // to English under the lock: another test in this binary leaves the
    // process locale at zh-CN, whose labels are short enough to fit and
    // would have made this a false pass.
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    // Wide enough for labels on both sides (`model_layout` hides them under 960).
    cx.simulate_resize(size(px(1200.), px(800.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // Selectors are `label-card-{MouseControlId:?}` / `label-value-row-{MouseControlId:?}`.
    for (card_selector, row_selector) in [
        (
            "label-card-Button(Forward)",
            "label-value-row-Button(Forward)",
        ),
        (
            "label-card-Button(MiddleClick)",
            "label-value-row-Button(MiddleClick)",
        ),
    ] {
        let card = cx
            .debug_bounds(card_selector)
            .expect("the synthetic model renders a label card for this control");
        let row = cx
            .debug_bounds(row_selector)
            .expect("the label card renders its value row");
        assert!(
            card.contains(&row.origin) && card.contains(&row.bottom_right()),
            "{row_selector}: value row {row:?} must sit inside its card {card:?}"
        );
    }

    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

/// At the smallest window with cards the synthetic model shrinks by its width
/// until its five cards no longer fit beside it. The canvas grows to hold the
/// ones that reach past the model, rather than leaving them over its edge
/// where the clipped tab can cut them off.
#[gpui::test]
fn cards_that_reach_past_a_shrunk_model_stay_on_the_canvas(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(960.), px(680.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let canvas = cx
        .debug_bounds("mouse-canvas")
        .expect("the model canvas renders");
    for selector in [
        "label-card-Button(MiddleClick)",
        "label-card-Button(Back)",
        "label-card-Button(DpiToggle)",
        "label-card-Button(Forward)",
        "label-card-Button(GestureButton)",
    ] {
        let card = cx.debug_bounds(selector).expect(selector);
        let inside = card.top() >= canvas.top() - px(0.5)
            && card.bottom() <= canvas.bottom() + px(0.5)
            && card.left() >= canvas.left() - px(0.5)
            && card.right() <= canvas.right() + px(0.5);
        assert!(
            inside,
            "{selector}: card {card:?} leaves the canvas {canvas:?}"
        );
    }

    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn action_picker_opens_on_selection_and_assigns_without_an_extra_click(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(1200.), px(1000.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    click_picker_control(cx, "label-card-Button(Forward)");
    view.read_with(cx, |view, _| assert!(view.action_picker_open));
    assert!(cx.debug_bounds("inspector-action-Copy").is_some());

    click_picker_control(cx, "inspector-action-custom-shortcut-add");
    view.read_with(cx, |view, _| {
        assert_eq!(view.invalid_drafts.shortcut, Some(Unrecognized));
        assert!(!view.invalid_drafts.application);
    });
    assert!(
        cx.debug_bounds("inspector-action-custom-shortcut-error")
            .is_some()
    );

    click_picker_control(cx, "inspector-action-custom-application-add");
    view.read_with(cx, |view, _| {
        assert!(view.action_picker_open);
        assert_eq!(view.invalid_drafts.shortcut, Some(Unrecognized));
        assert!(view.invalid_drafts.application);
    });
    assert!(
        cx.debug_bounds("inspector-action-custom-application-error")
            .is_some()
    );
    assert_forward_binding(cx, &default_binding(ButtonId::Forward));

    click_picker_control(cx, "inspector-action-custom-shortcut-input");
    cx.simulate_input("Cmd+K");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    view.read_with(cx, |view, _| {
        assert_eq!(view.invalid_drafts.shortcut, None);
        assert!(view.invalid_drafts.application);
    });
    assert!(
        cx.debug_bounds("inspector-action-custom-shortcut-error")
            .is_none()
    );
    assert!(
        cx.debug_bounds("inspector-action-custom-application-error")
            .is_some()
    );
    click_picker_control(cx, "inspector-action-custom-shortcut-add");
    view.read_with(cx, |view, _| assert!(!view.action_picker_open));
    assert_forward_binding(cx, &Action::CustomShortcut("Cmd+K".parse().unwrap()));

    click_picker_control(cx, "inspector-current-action");
    view.read_with(cx, |view, cx| {
        assert_eq!(view.custom_shortcut_input.read(cx).value(), "");
        assert_eq!(view.custom_application_input.read(cx).value(), "");
        assert!(!view.invalid_drafts.application);
    });
    assert!(
        cx.debug_bounds("inspector-action-custom-application-error")
            .is_none()
    );
    click_picker_control(cx, "inspector-action-custom-application-input");
    cx.simulate_input("/Applications/Calculator.app");
    click_picker_control(cx, "inspector-action-custom-application-add");
    view.read_with(cx, |view, _| assert!(!view.action_picker_open));
    cx.update(|_, cx| {
        let Action::OpenApplication(target) =
            &AppState::try_read(cx).unwrap().button_bindings()[&ButtonId::Forward]
        else {
            panic!("the application editor must commit an application action");
        };
        assert_eq!(target.path(), "/Applications/Calculator.app");
    });

    click_picker_control(cx, "inspector-current-action");
    click_picker_control(cx, "inspector-action-shortcut-hold");
    view.read_with(cx, |view, _| {
        assert_eq!(view.shortcut_mode, ShortcutMode::Hold);
    });
    click_picker_control(cx, "inspector-action-custom-shortcut-input");
    cx.simulate_input("Cmd+K");
    click_picker_control(cx, "inspector-action-custom-shortcut-add");
    view.read_with(cx, |view, _| assert!(!view.action_picker_open));
    assert_forward_binding(cx, &Action::HoldShortcut("Cmd+K".parse().unwrap()));

    click_picker_control(cx, "inspector-current-action");
    view.read_with(cx, |view, _| {
        assert_eq!(view.shortcut_mode, ShortcutMode::Tap);
    });
    cx.update(|window, cx| {
        view.read(cx)
            .action_search
            .focus_handle(cx)
            .focus(window, cx);
    });
    cx.simulate_input("Copy");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "inspector-action-Copy");
    view.read_with(cx, |view, _| assert!(!view.action_picker_open));
    assert_forward_binding(cx, &Action::Copy);

    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

/// #2117: Record fills the custom shortcut field from the next key press, and
/// Add commits the field as before.
#[gpui::test]
fn record_fills_the_custom_shortcut_from_the_next_key_press(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(1200.), px(1000.)));
    // Focus events, which end a recording, only fire in an active window.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "label-card-Button(Forward)");
    let shortcut_text = |cx: &mut VisualTestContext| {
        view.read_with(cx, |view, cx| {
            view.custom_shortcut_input.read(cx).value().to_string()
        })
    };
    let press = |cx: &mut VisualTestContext, keystroke: &str| {
        cx.simulate_keystrokes(keystroke);
        cx.update(|window, cx| window.draw(cx).clear(cx));
    };

    click_picker_control(cx, "inspector-action-custom-shortcut-record");
    assert!(
        cx.debug_bounds("inspector-action-custom-shortcut-recording")
            .is_some()
    );
    press(cx, "ctrl-shift-pageup");
    assert_eq!(shortcut_text(cx), "Ctrl+Shift+PageUp");
    assert!(
        cx.debug_bounds("inspector-action-custom-shortcut-recording")
            .is_none()
    );
    view.read_with(cx, |view, _| {
        assert!(view.action_picker_open, "recording alone commits nothing");
    });
    assert_forward_binding(cx, &default_binding(ButtonId::Forward));

    click_picker_control(cx, "inspector-action-custom-shortcut-record");
    press(cx, "escape");
    assert_eq!(shortcut_text(cx), "Ctrl+Shift+PageUp", "Escape cancels");
    assert!(
        cx.debug_bounds("inspector-action-custom-shortcut-recording")
            .is_none()
    );

    click_picker_control(cx, "inspector-action-custom-shortcut-record");
    press(cx, "f21");
    view.read_with(cx, |view, _| {
        assert_eq!(view.invalid_drafts.shortcut, Some(Unrecognized));
    });
    assert!(
        cx.debug_bounds("inspector-action-custom-shortcut-error")
            .is_some()
    );

    // #2121: Cmd+Shift+] arrives as Cmd+`}`. Record refuses it and leaves
    // the field as it was.
    click_picker_control(cx, "inspector-action-custom-shortcut-record");
    press_shifted_symbol(cx, "}");
    view.read_with(cx, |view, _| {
        assert_eq!(view.invalid_drafts.shortcut, Some(ShiftedSymbol));
        assert!(view.shortcut_recorder.is_none());
    });
    assert_eq!(shortcut_text(cx), "Ctrl+Shift+PageUp");

    // Moving focus out of the Record area ends the recording, so the next
    // key press reaches the newly focused field (#2138 review).
    click_picker_control(cx, "inspector-action-custom-shortcut-record");
    click_picker_control(cx, "inspector-action-custom-application-input");
    view.read_with(cx, |view, _| assert!(view.shortcut_recorder.is_none()));
    assert!(
        cx.debug_bounds("inspector-action-custom-shortcut-recording")
            .is_none()
    );
    cx.simulate_input("x");
    view.read_with(cx, |view, cx| {
        assert_eq!(view.custom_application_input.read(cx).value(), "x");
    });
    assert_eq!(shortcut_text(cx), "Ctrl+Shift+PageUp");

    click_picker_control(cx, "inspector-action-custom-shortcut-add");
    view.read_with(cx, |view, _| assert!(!view.action_picker_open));
    assert_forward_binding(
        cx,
        &Action::CustomShortcut("Ctrl+Shift+PageUp".parse().unwrap()),
    );

    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

/// #2122: a mouse button can be bound to a shell command from the picker, the
/// way the Actions Ring and keyboard editors already allow.
#[gpui::test]
fn the_action_picker_binds_a_button_to_a_shell_command(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(1200.), px(1000.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    click_picker_control(cx, "label-card-Button(Forward)");

    click_picker_control(cx, "inspector-action-custom-command-add");
    view.read_with(cx, |view, _| {
        assert!(view.action_picker_open);
        assert_eq!(
            view.invalid_drafts,
            InvalidDrafts {
                command: true,
                ..InvalidDrafts::default()
            }
        );
    });
    assert!(
        cx.debug_bounds("inspector-action-custom-command-error")
            .is_some()
    );
    assert_forward_binding(cx, &default_binding(ButtonId::Forward));

    click_picker_control(cx, "inspector-action-custom-command-input");
    click_picker_control(cx, "shell-mode-cmd");
    click_picker_control(cx, "inspector-action-custom-command-input");
    cx.simulate_input("snipaste snip");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    view.read_with(cx, |view, _| assert!(!view.invalid_drafts.command));
    assert!(
        cx.debug_bounds("inspector-action-custom-command-error")
            .is_none()
    );
    click_picker_control(cx, "inspector-action-custom-command-add");
    view.read_with(cx, |view, _| assert!(!view.action_picker_open));
    assert_forward_binding(cx, &Action::RunShellCommand("snipaste snip".into()));

    click_picker_control(cx, "inspector-current-action");
    view.read_with(cx, |view, cx| {
        assert_eq!(view.custom_command_input.read(cx).value(), "");
    });

    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn a_selected_gesture_can_render_in_the_binding_inspector(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.run_until_parked();

    view.update(cx, |view, cx| {
        view.set_gesture_selected_dir(Some(GestureDirection::Up));
        let gesture_maps = BTreeMap::from([(
            ButtonId::MiddleClick,
            BTreeMap::from([(
                GestureDirection::Click,
                default_binding(ButtonId::MiddleClick),
            )]),
        )]);
        let bindings = BTreeMap::new();
        let entity = cx.entity();

        binding_inspector(
            BindingInspectorData {
                selected: Some(MouseControlId::Button(ButtonId::MiddleClick)),
                gesture_direction: Some(GestureDirection::Up),
                bindings: &bindings,
                layout: None,
                gesture_maps: &gesture_maps,
                dpi_gestures: false,
                virtual_thumbwheel: false,
                wheel_hold: &BTreeSet::new(),
                editing_app: None,
                overridden: None,
                press_force: None,
                extra_actions: &[],
                hidden_actions: &[],
            },
            ActionPickerContext {
                selected: None,
                direction: None,
                open: true,
                search: &view.action_search,
                shortcut_input: &view.custom_shortcut_input,
                application_input: &view.custom_application_input,
                command_input: &view.custom_command_input,
                command_draft: &view.custom_command_draft,
                macro_draft: &view.macro_draft,
                shortcut_mode: view.shortcut_mode,
                shortcut_recording: false,
                shortcut_record_focus: &view.shortcut_record_focus,
                invalid: view.invalid_drafts,
                view: &entity,
            },
            cx,
        );
    });
    cx.run_until_parked();
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn selecting_another_control_opens_the_action_picker_and_resets_drafts(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(1200.), px(1000.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "label-card-Button(Back)");
    click_picker_control(cx, "inspector-action-custom-shortcut-input");
    cx.simulate_input("Ctrl+K");
    click_picker_control(cx, "label-card-Button(Forward)");
    view.read_with(cx, |view, cx| {
        assert!(view.action_picker_open);
        assert_eq!(view.custom_shortcut_input.read(cx).value(), "");
    });
    assert!(cx.debug_bounds("inspector-action-Copy").is_some());
    click_picker_control(cx, "inspector-action-custom-shortcut-input");
    cx.simulate_input("S");
    click_picker_control(cx, "inspector-action-custom-shortcut-add");
    assert_forward_binding(cx, &Action::CustomShortcut("S".parse().unwrap()));
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn clearing_custom_action_drafts_resets_text_and_invalid_state(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.run_until_parked();

    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.custom_shortcut_input
                .update(cx, |input, cx| input.set_value("Cmd+K", window, cx));
            view.custom_application_input
                .update(cx, |input, cx| input.set_value("/bin/true", window, cx));
            view.custom_command_input
                .update(cx, |input, cx| input.set_value("echo hi", window, cx));
            view.invalid_drafts = InvalidDrafts {
                shortcut: Some(ShiftedSymbol),
                application: true,
                command: true,
            };
            view.shortcut_mode = ShortcutMode::Hold;

            view.clear_custom_action_drafts(window, cx);
        });
    });

    view.update(cx, |view, cx| {
        assert_eq!(view.custom_shortcut_input.read(cx).value(), "");
        assert_eq!(view.custom_application_input.read(cx).value(), "");
        assert_eq!(view.custom_command_input.read(cx).value(), "");
        assert_eq!(view.macro_draft.name_input.read(cx).value(), "");
        assert_eq!(view.macro_draft.on_press_input.read(cx).value(), "");
        assert!(!view.macro_draft.repeat_while_held);
        assert!(!view.macro_draft.invalid);
        assert_eq!(view.invalid_drafts, InvalidDrafts::default());
        assert_eq!(view.shortcut_mode, ShortcutMode::Tap);
    });
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn macro_editor_builds_validates_and_saves_multi_phase_macro(cx: &mut TestAppContext) {
    use openlogi_core::binding::{KeyboardUsage, Macro, MacroStep};

    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(1200.), px(1400.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    click_picker_control(cx, "label-card-Button(Forward)");

    // Saving an empty macro flags an inline error and keeps the picker open.
    click_picker_control(cx, "inspector-action-macro-save");
    view.read_with(cx, |view, _| {
        assert!(view.action_picker_open);
        assert!(view.macro_draft.invalid);
    });
    assert!(cx.debug_bounds("inspector-action-macro-error").is_some());
    assert_forward_binding(cx, &default_binding(ButtonId::Forward));

    // Enter macro name.
    click_picker_control(cx, "inspector-action-macro-name-input");
    cx.simulate_input("Crouch Hop");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    view.read_with(cx, |view, _| assert!(!view.macro_draft.invalid));
    assert!(cx.debug_bounds("inspector-action-macro-error").is_none());

    // Add RightCtrl KeyDown, 50ms Delay, RightCtrl KeyUp to On press via step controls.
    click_picker_control(cx, "inspector-action-macro-key-input");
    cx.simulate_input("RightCtrl");
    click_picker_control(cx, "inspector-action-macro-add-key-down");

    click_picker_control(cx, "inspector-action-macro-delay-input");
    cx.simulate_input("50ms");
    click_picker_control(cx, "inspector-action-macro-add-delay");

    click_picker_control(cx, "inspector-action-macro-add-key-up");

    view.read_with(cx, |view, cx| {
        assert_eq!(
            view.macro_draft.on_press_input.read(cx).value(),
            "+RightCtrl, 50ms, -RightCtrl"
        );
    });
    assert!(
        cx.debug_bounds("inspector-action-macro-on-press-step-0")
            .is_some()
    );
    assert!(
        cx.debug_bounds("inspector-action-macro-on-press-step-1")
            .is_some()
    );
    assert!(
        cx.debug_bounds("inspector-action-macro-on-press-step-2")
            .is_some()
    );

    // Switch step builder target to While holding, add a 50ms delay, and enable Repeat while held.
    click_picker_control(cx, "inspector-action-macro-phase-while-held");
    click_picker_control(cx, "inspector-action-macro-add-delay");
    click_picker_control(cx, "inspector-action-macro-repeat-toggle");

    // Type On release steps directly into the sequence input.
    click_picker_control(cx, "inspector-action-macro-on-release-input");
    cx.simulate_input("-RightCtrl");
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // Save the macro and verify it commits to AppState.
    click_picker_control(cx, "inspector-action-macro-save");
    view.read_with(cx, |view, _| assert!(!view.action_picker_open));

    let right_ctrl: KeyboardUsage = "RightCtrl".parse().unwrap();
    let expected = Action::Macro(Box::new(Macro {
        name: "Crouch Hop".into(),
        on_press: vec![
            MacroStep::KeyDown(right_ctrl),
            MacroStep::Delay { millis: 50 },
            MacroStep::KeyUp(right_ctrl),
        ],
        while_held: vec![MacroStep::Delay { millis: 50 }],
        on_release: vec![MacroStep::KeyUp(right_ctrl)],
        repeat_while_held: true,
    }));
    assert_forward_binding(cx, &expected);

    // Returning to a button opens its editor and restores the existing macro immediately.
    click_picker_control(cx, "label-card-Button(Back)");
    click_picker_control(cx, "label-card-Button(Forward)");
    view.read_with(cx, |view, cx| {
        assert_eq!(view.macro_draft.name_input.read(cx).value(), "Crouch Hop");
        assert_eq!(
            view.macro_draft.on_press_input.read(cx).value(),
            "+RightCtrl, 50ms, -RightCtrl"
        );
        assert_eq!(view.macro_draft.while_held_input.read(cx).value(), "50ms");
        assert_eq!(
            view.macro_draft.on_release_input.read(cx).value(),
            "-RightCtrl"
        );
        assert!(view.macro_draft.repeat_while_held);
    });

    // Reordering and removing a step updates the sequence.
    click_picker_control(cx, "inspector-action-macro-on-press-step-down-0");
    view.read_with(cx, |view, cx| {
        assert_eq!(
            view.macro_draft.on_press_input.read(cx).value(),
            "50ms, +RightCtrl, -RightCtrl"
        );
    });
    click_picker_control(cx, "inspector-action-macro-on-press-step-remove-0");
    view.read_with(cx, |view, cx| {
        assert_eq!(
            view.macro_draft.on_press_input.read(cx).value(),
            "+RightCtrl, -RightCtrl"
        );
    });

    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[test]
fn active_thumbwheel_directions_highlight_the_paired_control() {
    assert_eq!(
        MouseControlId::from_active_button(ButtonId::ThumbwheelScrollUp),
        MouseControlId::ThumbwheelRotation
    );
    assert_eq!(
        MouseControlId::from_active_button(ButtonId::ThumbwheelScrollDown),
        MouseControlId::ThumbwheelRotation
    );
}

#[test]
fn fallback_model_only_adds_thumbwheel_when_capability_is_measured() {
    let (_, _, without, _) = scaled_model(
        None,
        None,
        560.,
        560.,
        420.,
        false,
        LabelDistribution::LeftOnly,
    );
    let (_, _, with, _) = scaled_model(
        None,
        None,
        560.,
        560.,
        420.,
        true,
        LabelDistribution::LeftOnly,
    );
    assert_eq!(
        without
            .iter()
            .filter(|hotspot| hotspot.id == MouseControlId::ThumbwheelRotation)
            .count(),
        0
    );
    assert_eq!(
        with.iter()
            .filter(|hotspot| hotspot.id == MouseControlId::ThumbwheelRotation)
            .count(),
        1
    );
}

/// Installs the known mouse, with or without raw wheel on Back/Forward, and
/// opens the mouse view on it with `button` selected. Callers hold
/// `LOCALE_LOCK` in English: a locale change from another test between a
/// click's mouse-down redraw and its mouse-up reflows the inspector, and the
/// release then misses the switch.
fn wheel_hold_view(
    cx: &mut TestAppContext,
    raw_wheel: bool,
    config: Config,
    button: ButtonId,
) -> (Entity<MouseModelView>, &mut VisualTestContext) {
    cx.update(gpui_component::init);
    cx.update(|cx| {
        let resolver = AssetResolver::new();
        let (commands, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut inventory = crate::state::tests::direct_inventory([0xa3, 0x93, 0xca, 0xe0]);
        inventory.paired[0]
            .capabilities
            .as_mut()
            .expect("the fixture mouse has capabilities")
            .virtual_thumbwheel = raw_wheel;
        let state = cx.new(|_| {
            AppState::new(Sources {
                inventories: &[inventory],
                ..Sources::in_memory(config, &resolver, commands)
            })
        });
        AppState::set_global(state, cx);
    });
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(1200.), px(1000.)));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.select(MouseControlId::Button(button), window, cx);
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (view, cx)
}

fn holds_the_wheel(cx: &mut VisualTestContext, button: ButtonId) -> bool {
    cx.update(|_, cx| {
        AppState::try_read(cx)
            .expect("state is installed")
            .wheel_hold_buttons()
            .contains(&button)
    })
}

#[gpui::test]
fn the_wheel_hold_switch_turns_holding_back_into_sideways_scrolling(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let (view, cx) = wheel_hold_view(cx, true, Config::ephemeral(), ButtonId::Back);
    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_some(),
        "Back offers the switch on a mouse that reports raw wheel"
    );
    assert!(!holds_the_wheel(cx, ButtonId::Back));

    click_picker_control(cx, "inspector-wheel-hold-row-control");
    assert!(holds_the_wheel(cx, ButtonId::Back));

    click_picker_control(cx, "inspector-wheel-hold-row-control");
    assert!(!holds_the_wheel(cx, ButtonId::Back));

    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.select(MouseControlId::Button(ButtonId::MiddleClick), window, cx);
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_none(),
        "only Back and Forward can hold the wheel"
    );
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn a_mouse_without_raw_wheel_offers_no_wheel_hold(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let (view, cx) = wheel_hold_view(cx, false, Config::ephemeral(), ButtonId::Forward);
    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_none(),
        "no switch where holding cannot scroll"
    );
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn a_carried_over_wheel_hold_can_still_be_turned_off(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let mut config = Config::ephemeral();
    config.set_wheel_hold(
        crate::state::tests::KNOWN_MOUSE_KEY,
        ButtonId::Forward,
        true,
    );
    let (view, cx) = wheel_hold_view(cx, false, config, ButtonId::Forward);
    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_some(),
        "a setting from another mouse stays visible so it can be turned off"
    );

    click_picker_control(cx, "inspector-wheel-hold-row-control");

    assert!(!holds_the_wheel(cx, ButtonId::Forward));
    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_none(),
        "once off, it cannot be turned back on here"
    );
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn an_app_profile_explains_that_an_action_replaces_wheel_hold(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let mut config = Config::ephemeral();
    config.set_wheel_hold(crate::state::tests::KNOWN_MOUSE_KEY, ButtonId::Back, true);
    let (view, cx) = wheel_hold_view(cx, true, config, ButtonId::Back);
    assert!(cx.debug_bounds("inspector-wheel-hold-app-note").is_none());

    cx.update(|_, cx| {
        AppState::apply(cx, |state| {
            state.set_editing_app(Some("com.apple.Safari".into()))
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));

    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_none(),
        "the switch is device-wide, so an app profile does not show it"
    );
    assert!(
        cx.debug_bounds("inspector-wheel-hold-app-note").is_some(),
        "the app profile says what picking an action does"
    );
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.select(MouseControlId::Button(ButtonId::Forward), window, cx);
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("inspector-wheel-hold-app-note").is_none(),
        "a button that does not hold the wheel has nothing to explain"
    );
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.select(MouseControlId::Button(ButtonId::Back), window, cx);
        });
    });
    cx.update(|_, cx| {
        AppState::apply(cx, |state| {
            state.commit_binding(ButtonId::Back, Action::Copy)
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("inspector-wheel-hold-app-note").is_none(),
        "once the app overrides the button, its own action already applies"
    );
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

fn in_gesture_mode(cx: &mut VisualTestContext, button: ButtonId) -> bool {
    cx.update(|_, cx| {
        AppState::try_read(cx)
            .expect("state is installed")
            .gesture_bindings()
            .contains_key(&button)
    })
}

#[gpui::test]
fn a_gesture_button_offers_wheel_hold_and_leaves_gestures_for_it(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let mut config = Config::ephemeral();
    config.set_gesture_mode(crate::state::tests::KNOWN_MOUSE_KEY, ButtonId::Back, true);
    let (view, cx) = wheel_hold_view(cx, true, config, ButtonId::Back);
    view.update(cx, |view, _| {
        view.set_gesture_selected_dir(Some(GestureDirection::Up));
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(in_gesture_mode(cx, ButtonId::Back));
    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_some(),
        "the gesture inspector offers the switch too"
    );

    click_picker_control(cx, "inspector-wheel-hold-row-control");

    assert!(holds_the_wheel(cx, ButtonId::Back));
    assert!(
        !in_gesture_mode(cx, ButtonId::Back),
        "holding the wheel leaves gesture mode"
    );
    view.read_with(cx, |view, _| assert_eq!(view.gesture_active_dir, None));
    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_some(),
        "the single-action inspector shows the switch, now on"
    );
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

#[gpui::test]
fn a_gesture_button_without_raw_wheel_offers_no_wheel_hold(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let mut config = Config::ephemeral();
    config.set_gesture_mode(
        crate::state::tests::KNOWN_MOUSE_KEY,
        ButtonId::Forward,
        true,
    );
    let (view, cx) = wheel_hold_view(cx, false, config, ButtonId::Forward);
    assert!(in_gesture_mode(cx, ButtonId::Forward));
    assert!(
        cx.debug_bounds("inspector-wheel-hold-row").is_none(),
        "no switch where holding cannot scroll"
    );
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

/// An app profile replaces a gesture button's gestures with a button binding,
/// which gets a release, so its picker offers the held actions too.
#[gpui::test]
fn an_inherited_gesture_button_offers_the_held_actions(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let mut config = Config::ephemeral();
    config.set_gesture_mode(crate::state::tests::KNOWN_MOUSE_KEY, ButtonId::Back, true);
    let (view, cx) = wheel_hold_view(cx, true, config, ButtonId::Back);
    cx.update(|_, cx| {
        AppState::apply(cx, |state| {
            state.set_editing_app(Some("com.apple.Safari".into()))
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    view.update(cx, |view, _| view.action_picker_open = true);
    cx.update(|window, cx| window.draw(cx).clear(cx));

    assert!(cx.debug_bounds("inspector-gesture-override-Copy").is_some());
    assert!(
        cx.debug_bounds("inspector-gesture-override-DpiShift")
            .is_some()
    );
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

/// The smallest window with cards shrinks a G502's fallback model by its
/// width to about 315 px, short of what its nine cards on one side need.
/// They reach past the model into the tab's height, closing their gaps,
/// instead of running off the canvas.
#[test]
fn medium_window_uses_two_readable_card_columns() {
    let layout = model_layout(
        None,
        GamingLayout::for_model_key("0407f"),
        1280.,
        850.,
        false,
    );
    assert_eq!(layout.labels.len(), 9);
    assert!(
        layout
            .labels
            .iter()
            .any(|label| label.side == super::super::leader_lines::Side::Right)
    );
    assert!(layout.label_w >= 220.);
    assert!(layout.canvas_h <= 850. - MODEL_VERTICAL_RESERVE);
    assert!(layout.mouse_w >= 190.);
}

#[test]
fn a_crowded_side_keeps_every_card_on_the_canvas() {
    let viewport_h = 680.;
    let g502 = GamingLayout::for_model_key("0407f");
    let layout = model_layout(None, g502, 960., viewport_h, false);
    assert_eq!(layout.labels.len(), 9);
    assert!(layout.mouse_h < 9. * LABEL_H, "model {}", layout.mouse_h);
    assert!(layout.canvas_h <= (viewport_h - MODEL_VERTICAL_RESERVE).max(9. * LABEL_H) + 0.01);
    let mut ys: Vec<f32> = layout.labels.iter().map(|label| label.y).collect();
    ys.sort_by(f32::total_cmp);
    for y in &ys {
        let top = layout.mouse_top + y - LABEL_H / 2.;
        assert!(
            top >= -0.01 && top + LABEL_H <= layout.canvas_h + 0.01,
            "card off the canvas: {ys:?} in {}",
            layout.canvas_h
        );
    }
    for pair in ys.windows(2) {
        assert!(pair[1] - pair[0] >= LABEL_H - 0.01, "cards overlap: {ys:?}");
    }
}

/// The fallback silhouette shrinks on a small window, but its cards keep
/// their height, so they must be placed at the scaled size, not scaled after.
#[test]
fn a_shrunk_fallback_model_keeps_its_cards_apart() {
    let target_h = MOUSE_MODEL_SIZE.1 * 0.776;
    let (_, h, _, labels) = scaled_model(
        None,
        None,
        target_h,
        target_h,
        1000.,
        false,
        LabelDistribution::LeftOnly,
    );
    assert!((h - target_h).abs() < 0.01);
    let mut ys: Vec<f32> = labels.iter().map(|label| label.y).collect();
    ys.sort_by(f32::total_cmp);
    for pair in ys.windows(2) {
        assert!(
            pair[1] - pair[0] >= super::super::geometry::LABEL_H,
            "cards overlap: {ys:?}"
        );
    }
}

#[gpui::test]
fn gshift_keeps_preview_geometry_fixed_and_primary_editors_in_the_inspector(
    cx: &mut TestAppContext,
) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    cx.update(|cx| {
        let resolver = AssetResolver::new();
        let (commands, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut inventory = crate::state::tests::direct_inventory([0xa3, 0x93, 0xca, 0xe0]);
        inventory.paired[0].model_info.as_mut().unwrap().model_ids = [0x409f, 0, 0];
        inventory.paired[0]
            .model_info
            .as_mut()
            .unwrap()
            .extended_model_id = 1;
        inventory.paired[0].capabilities =
            Some(openlogi_core::device::Capabilities::from_feature_ids(&[
                0x8100, 0x8110,
            ]));
        let state = cx.new(|_| {
            AppState::new(Sources {
                inventories: &[inventory],
                ..Sources::in_memory(Config::ephemeral(), &resolver, commands)
            })
        });
        AppState::set_global(state, cx);
    });
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    for (index, (width, height)) in [(1280., 548.), (1734., 800.), (1000., 600.)]
        .into_iter()
        .enumerate()
    {
        cx.simulate_resize(size(px(width), px(height)));
        cx.update(|_, cx| {
            AppState::apply(cx, |state| state.set_gshift_latched(false));
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let normal_scroll = cx.debug_bounds("mouse-model-scroll").unwrap();
        let normal_switch = cx.debug_bounds("gshift-switch-slot").unwrap();
        if index == 0 {
            assert_g4_is_visible_and_clickable(&view, cx);
        }
        cx.update(|_, cx| {
            AppState::apply(cx, |state| state.set_gshift_latched(true));
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            normal_scroll,
            cx.debug_bounds("mouse-model-scroll").unwrap()
        );
        assert_eq!(
            normal_switch,
            cx.debug_bounds("gshift-switch-slot").unwrap()
        );
        let bar = cx.debug_bounds("gshift-layer-bar").unwrap();
        if index < 2 {
            assert_eq!(normal_scroll.bottom(), bar.bottom());
        } else {
            assert!(normal_scroll.bottom() <= bar.top());
        }
        let apply = cx.debug_bounds("apply-gshift-editing").unwrap();
        assert!(apply.left() >= normal_switch.right());
        assert!(
            apply.right() <= bar.right(),
            "apply toggle must fit the preview pane"
        );
        cx.update(|_, cx| {
            let state = AppState::try_read(cx).unwrap();
            assert!(!state.apply_gshift_while_editing());
        });
        click_picker_control(cx, "apply-gshift-switch");
        cx.update(|_, cx| assert!(AppState::try_read(cx).unwrap().apply_gshift_while_editing()));
        click_picker_control(cx, "apply-gshift-switch");
        cx.update(|_, cx| {
            let state = AppState::try_read(cx).unwrap();
            assert!(!state.apply_gshift_while_editing());
            assert!(state.gshift_latched());
        });
        let primary = cx.debug_bounds("primary-gshift-LeftClick").unwrap();
        let inspector = cx.debug_bounds("button-inspector").unwrap();
        assert!(inspector.contains(&primary.origin) && inspector.contains(&primary.bottom_right()));
        click_picker_control(cx, "primary-gshift-LeftClick");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.selected,
                Some(MouseControlId::Button(ButtonId::LeftClick))
            );
            assert!(view.action_picker_open);
        });
    }
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
}

fn assert_g4_is_visible_and_clickable(view: &Entity<MouseModelView>, cx: &mut VisualTestContext) {
    let back = cx.debug_bounds("label-card-Button(Back)").unwrap();
    let scroll = cx.debug_bounds("mouse-model-scroll").unwrap();
    assert!(
        back.bottom() <= scroll.bottom(),
        "the G4 card must be visible above the G-Shift area's empty gutter: {back:?}, {scroll:?}"
    );
    cx.simulate_click(
        gpui::point(back.center().x, back.bottom() - px(8.)),
        Modifiers::default(),
    );
    cx.update(|window, cx| window.draw(cx).clear(cx));
    view.read_with(cx, |view, _| {
        assert_eq!(view.selected, Some(MouseControlId::Button(ButtonId::Back)));
        assert!(view.action_picker_open);
    });
}

#[gpui::test]
fn macro_recording_and_complete_text_clipboard_stay_editable(cx: &mut TestAppContext) {
    use openlogi_core::binding::MacroStep;
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    install_app_state(cx);
    let (view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(1200.), px(1400.)));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "label-card-Button(Forward)");
    click_picker_control(cx, "inspector-action-macro-record");
    view.read_with(cx, |view, _| assert!(view.macro_draft.recording.is_some()));
    cx.simulate_keystrokes("a");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: Keystroke::parse("a").unwrap(),
    });
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, cx| {
        assert!(view.macro_draft.recording.is_none());
        let steps =
            MacroStep::parse_sequence(&view.macro_draft.on_press_input.read(cx).value()).unwrap();
        let edges: Vec<_> = steps
            .into_iter()
            .filter(|step| !matches!(step, MacroStep::Delay { .. }))
            .collect();
        let a = "A".parse().unwrap();
        assert_eq!(edges, [MacroStep::KeyDown(a), MacroStep::KeyUp(a)]);
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "inspector-action-macro-record");
    click_picker_control(cx, "inspector-action-macro-record");
    view.read_with(cx, |view, _| assert!(view.macro_draft.recording.is_none()));
    click_picker_control(cx, "inspector-action-macro-record");
    cx.simulate_keystrokes("b");
    click_picker_control(cx, "inspector-action-macro-name-input");
    view.read_with(cx, |view, cx| {
        assert!(view.macro_draft.recording.is_none());
        let macro_def = view.macro_draft.build_macro(cx).unwrap();
        assert_eq!(
            macro_def.on_press.last(),
            Some(&MacroStep::KeyUp("B".parse().unwrap()))
        );
    });
    click_picker_control(cx, "inspector-action-macro-record");
    click_picker_control(cx, "inspector-action-macro-phase-on-release");
    view.read_with(cx, |view, _| {
        assert!(view.macro_draft.recording.is_none());
        assert_eq!(
            view.macro_draft.active_phase,
            super::super::macro_editor::MacroPhase::OnRelease
        );
    });
    click_picker_control(cx, "inspector-action-macro-copy");
    let text = cx.update(|_, cx| cx.read_from_clipboard().unwrap().text().unwrap());
    let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
    json["name"] = "Pasted macro".into();
    json["on_release"] = "B".into();
    json["repeat_while_held"] = true.into();
    cx.update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(json.to_string())));
    click_picker_control(cx, "inspector-action-macro-paste");
    click_picker_control(cx, "inspector-action-macro-apply-text");
    view.read_with(cx, |view, cx| {
        let macro_def = view.macro_draft.build_macro(cx).unwrap();
        assert_eq!(macro_def.name, "Pasted macro");
        assert!(macro_def.repeat_while_held);
        assert_eq!(
            macro_def.on_release,
            MacroStep::parse_sequence("B").unwrap()
        );
    });
    cx.update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string("broken".into())));
    click_picker_control(cx, "inspector-action-macro-paste");
    click_picker_control(cx, "inspector-action-macro-apply-text");
    view.read_with(cx, |view, cx| {
        assert!(view.macro_draft.text.error.is_some());
        assert_eq!(
            view.macro_draft.build_macro(cx).unwrap().name,
            "Pasted macro"
        );
    });
    click_picker_control(cx, "inspector-action-macro-save-text");
    view.read_with(cx, |view, _| assert!(view.action_picker_open));
    cx.update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(json.to_string())));
    click_picker_control(cx, "inspector-action-macro-paste");
    click_picker_control(cx, "inspector-action-macro-save-text");
    view.read_with(cx, |view, _| assert!(!view.action_picker_open));
    cx.update(|_, cx| {
        let Action::Macro(macro_def) =
            &AppState::try_read(cx).unwrap().button_bindings()[&ButtonId::Forward]
        else {
            panic!("a macro was saved");
        };
        assert_eq!(macro_def.name, "Pasted macro");
        assert!(macro_def.repeat_while_held);
        assert_eq!(
            macro_def.on_release,
            MacroStep::parse_sequence("B").unwrap()
        );
    });
}

#[gpui::test]
fn inspector_wheel_reverses_after_a_fast_burst_at_the_top(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    let cx = open_macro_editor(cx, "C");
    cx.simulate_resize(size(px(1000.), px(600.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let before = cx.debug_bounds("inspector-action-search-input").unwrap();
    let inspector = cx.debug_bounds("button-inspector").unwrap();
    // Dispatch a burst inside one frame, as high-resolution wheels can do.
    // Extra upward input at the top must not become a debt that consumes the
    // first downward turn.
    cx.update(|window, cx| {
        use gpui::InputEvent as _;
        for delta in [60., 60., 60., 60., -60.] {
            window.dispatch_event(
                gpui::ScrollWheelEvent {
                    position: gpui::point(inspector.left() + px(8.), inspector.center().y),
                    delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(delta))),
                    modifiers: Modifiers::default(),
                    touch_phase: gpui::TouchPhase::Moved,
                }
                .to_platform_input(),
                cx,
            );
        }
        window.draw(cx).clear(cx);
    });
    let after = cx.debug_bounds("inspector-action-search-input").unwrap();
    assert_eq!(after.top(), before.top() - px(60.));
}

#[gpui::test]
fn macro_step_list_passes_wheel_to_the_sidebar_only_at_its_boundary(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    let cx = open_macro_editor(cx, &["+B, 50ms, -B"; 10].join(", "));
    let list = cx
        .debug_bounds("inspector-action-macro-steps-scroll")
        .unwrap();
    let search = cx.debug_bounds("inspector-action-search-input").unwrap();
    let wheel = |dy| gpui::ScrollWheelEvent {
        position: list.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(dy))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    };
    cx.simulate_event(wheel(-10_000.));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        cx.debug_bounds("inspector-action-search-input").unwrap(),
        search
    );
    let first = cx
        .debug_bounds("inspector-action-macro-on-press-step-0")
        .unwrap();
    cx.simulate_event(wheel(-60.));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        cx.debug_bounds("inspector-action-search-input")
            .unwrap()
            .top(),
        search.top() - px(60.)
    );
    let list_after = cx
        .debug_bounds("inspector-action-macro-steps-scroll")
        .unwrap();
    let first_after = cx
        .debug_bounds("inspector-action-macro-on-press-step-0")
        .unwrap();
    assert_eq!(
        first.top() - list.top(),
        first_after.top() - list_after.top()
    );
}

#[gpui::test]
fn long_macro_steps_scroll_without_overlapping_the_next_editor(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    let cx = open_macro_editor(cx, &["+B, 50ms, -B"; 10].join(", "));
    let scroll = cx
        .debug_bounds("inspector-action-macro-steps-scroll")
        .unwrap();
    let first = cx
        .debug_bounds("inspector-action-macro-on-press-step-0")
        .unwrap();
    let shortcut = cx
        .debug_bounds("inspector-action-custom-shortcut-input")
        .unwrap();
    assert_eq!(scroll.size.height, px(STEP_LIST_MAX_H));
    assert!(first.size.height >= px(20.));
    assert!(scroll.bottom() <= shortcut.top());
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: scroll.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-200.))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let after = cx
        .debug_bounds("inspector-action-macro-on-press-step-0")
        .unwrap();
    assert!(after.top() < first.top());
    assert_eq!(
        scroll,
        cx.debug_bounds("inspector-action-macro-steps-scroll")
            .unwrap()
    );
    assert_eq!(
        shortcut,
        cx.debug_bounds("inspector-action-custom-shortcut-input")
            .unwrap()
    );
    cx.simulate_resize(size(px(1000.), px(600.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let scroll = cx
        .debug_bounds("inspector-action-macro-steps-scroll")
        .unwrap();
    let shortcut = cx
        .debug_bounds("inspector-action-custom-shortcut-input")
        .unwrap();
    assert_eq!(scroll.size.height, px(STEP_LIST_MAX_H));
    assert!(scroll.bottom() <= shortcut.top());
}

/// Bind Forward to a macro whose press phase is `on_press`, and open its editor.
fn open_macro_editor<'a>(cx: &'a mut TestAppContext, on_press: &str) -> &'a mut VisualTestContext {
    rust_i18n::set_locale("en");
    cx.update(gpui_component::init);
    install_app_state(cx);
    cx.update(|cx| {
        AppState::apply(cx, |state| {
            state.commit_binding(
                ButtonId::Forward,
                Action::Macro(Box::new(openlogi_core::binding::Macro {
                    name: "Macro".into(),
                    on_press: openlogi_core::binding::MacroStep::parse_sequence(on_press).unwrap(),
                    while_held: Vec::new(),
                    on_release: Vec::new(),
                    repeat_while_held: false,
                })),
            )
        });
    });
    let (_view, cx) = cx.add_window_view(MouseModelView::new);
    cx.simulate_resize(size(px(1200.), px(1400.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "label-card-Button(Forward)");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx
}

#[gpui::test]
fn short_macro_steps_take_only_their_rows_height(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    let cx = open_macro_editor(cx, "+LeftCtrl, 50ms, +C, 50ms, -C, 50ms, -LeftCtrl");
    let scroll = cx
        .debug_bounds("inspector-action-macro-steps-scroll")
        .unwrap();
    let last = cx
        .debug_bounds("inspector-action-macro-on-press-step-6")
        .unwrap();
    assert!(scroll.size.height < px(STEP_LIST_MAX_H));
    assert!(last.bottom() <= scroll.bottom());
}

#[gpui::test]
fn changing_action_preserves_a_dirty_macro_until_explicit_discard(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    let cx = open_macro_editor(cx, "C");
    click_picker_control(cx, "inspector-action-macro-name-input");
    cx.simulate_input(" edited");
    click_picker_control(cx, "inspector-action-search-input");
    cx.simulate_input("Copy");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "inspector-action-Copy");
    assert!(cx.debug_bounds("inspector-action-macro-unsaved").is_some());
    cx.update(|_, cx| {
        assert!(matches!(
            AppState::try_read(cx).unwrap().button_bindings()[&ButtonId::Forward],
            Action::Macro(_)
        ));
    });
    click_picker_control(cx, "inspector-action-macro-keep-editing");
    assert!(cx.debug_bounds("inspector-action-macro-unsaved").is_none());
    click_picker_control(cx, "inspector-action-Copy");
    click_picker_control(cx, "inspector-action-macro-discard-replace");
    assert_forward_binding(cx, &Action::Copy);
}

#[gpui::test]
fn macro_text_mode_hides_alternative_actions_at_narrow_width(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    let cx = open_macro_editor(cx, "C");
    click_picker_control(cx, "inspector-action-macro-edit-text");
    cx.simulate_resize(size(px(760.), px(1000.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("inspector-action-custom-shortcut-input")
            .is_none()
    );
    assert!(
        cx.debug_bounds("inspector-action-custom-application-input")
            .is_none()
    );
    assert!(
        cx.debug_bounds("inspector-action-custom-command-input")
            .is_none()
    );
    let inspector = cx.debug_bounds("button-inspector").unwrap();
    for selector in [
        "inspector-action-macro-edit-text",
        "inspector-action-macro-guide",
        "inspector-action-macro-save-text",
        "inspector-action-macro-apply-text",
    ] {
        let bounds = cx.debug_bounds(selector).expect(selector);
        assert!(
            bounds.left() >= inspector.left() && bounds.right() <= inspector.right(),
            "{selector} must fit the inspector"
        );
    }
}

#[gpui::test]
fn saving_macro_accepts_the_draft_as_its_new_baseline(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    let cx = open_macro_editor(cx, "C");
    click_picker_control(cx, "inspector-action-macro-name-input");
    cx.simulate_input(" edited");
    click_picker_control(cx, "inspector-action-macro-save");
    click_picker_control(cx, "label-card-Button(Forward)");
    click_picker_control(cx, "inspector-action-search-input");
    cx.simulate_input("Copy");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "inspector-action-Copy");
    assert!(cx.debug_bounds("inspector-action-macro-unsaved").is_none());
    assert_forward_binding(cx, &Action::Copy);
}

#[gpui::test]
fn changing_to_gestures_protects_an_unsaved_macro(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let mut config = Config::ephemeral();
    config.set_binding(
        crate::state::tests::KNOWN_MOUSE_KEY,
        ButtonId::Forward,
        openlogi_core::binding::Binding::Single(Action::Macro(Box::new(
            openlogi_core::binding::Macro {
                name: "Saved macro".into(),
                on_press: openlogi_core::binding::MacroStep::parse_sequence("C").unwrap(),
                while_held: Vec::new(),
                on_release: Vec::new(),
                repeat_while_held: false,
            },
        ))),
    );
    let (_, cx) = wheel_hold_view(cx, true, config, ButtonId::Forward);
    cx.simulate_resize(size(px(1200.), px(1400.)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "inspector-action-macro-name-input");
    cx.simulate_input(" edited");
    click_picker_control(cx, "inspector-use-gestures");
    assert!(cx.debug_bounds("inspector-action-macro-unsaved").is_some());
    assert!(!in_gesture_mode(cx, ButtonId::Forward));
    click_picker_control(cx, "inspector-action-macro-keep-editing");
    assert!(!in_gesture_mode(cx, ButtonId::Forward));
    click_picker_control(cx, "inspector-use-gestures");
    click_picker_control(cx, "inspector-action-macro-discard-replace");
    assert!(in_gesture_mode(cx, ButtonId::Forward));
}

#[gpui::test]
fn macro_f1_opens_one_separate_guide_and_preserves_the_draft(cx: &mut TestAppContext) {
    let _locale = LOCALE_LOCK.lock().unwrap();
    cx.update(crate::app::menu::install);
    let cx = open_macro_editor(cx, "C");
    click_picker_control(cx, "inspector-action-macro-name-input");
    cx.simulate_input(" edited");
    cx.simulate_keystrokes("f1");
    let guide = cx.update(|_, cx| {
        cx.default_global::<crate::windows::WindowRegistry>()
            .macro_guide
            .expect("F1 opens the guide")
    });
    cx.update(|_, cx| super::super::macro_guide::open(cx));
    cx.update(|_, cx| {
        assert_eq!(
            cx.default_global::<crate::windows::WindowRegistry>()
                .macro_guide,
            Some(guide)
        );
    });
    guide
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear(cx);
    });
    click_picker_control(cx, "inspector-action-search-input");
    cx.simulate_input("Copy");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    click_picker_control(cx, "inspector-action-Copy");
    assert!(
        cx.debug_bounds("inspector-action-macro-unsaved").is_some(),
        "opening the guide must preserve the edited draft"
    );
}
