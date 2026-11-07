use openlogi_core::binding::{
    Action, ActionRingIcon, ButtonId, GestureDirection, GridKey, KNOWN_CONTROLS, KeyCombo,
    KeyboardUsage, Macro, MacroStep,
};

use crate::features::mouse::thumbwheel::ThumbwheelPreset;
use crate::ui::action::localized_action_label;

#[test]
fn low_memory_control_label_and_explanation_resolve_in_every_locale() {
    for (locale, _) in openlogi_core::locale::SUPPORTED {
        for key in ["app.low_memory_mode", "app.low_memory_mode_description"] {
            let text = rust_i18n::t!(key, locale = locale);
            assert_ne!(text, key, "missing {key} in {locale}");
            assert!(!text.trim().is_empty());
        }
    }
    assert_eq!(
        rust_i18n::t!("app.low_memory_mode", locale = "en"),
        "Low memory mode"
    );
    assert_eq!(
        rust_i18n::t!("app.low_memory_mode", locale = "ru"),
        "Экономия памяти"
    );
}

/// Typed dynamic keys are the part catalog parity cannot inspect: a typo in
/// one of these methods compiles and renders the key at runtime. Resolve every
/// typed key against the English catalog, which also proves this binary loaded
/// the shared catalog at the expected relative path.
#[test]
fn typed_translation_keys_resolve() {
    let _locale = super::LOCALE_LOCK.lock();
    rust_i18n::set_locale("en");
    let covered = |key: &str| rust_i18n::t!(key) != key;
    assert!(covered("app.settings"), "desktop catalog is not wired up");

    for b in ButtonId::ALL
        .into_iter()
        .chain(KNOWN_CONTROLS.iter().map(|control| ButtonId::Control(control.id)))
        // A control with no catalog row resolves to the one generic key.
        .chain([ButtonId::control(0xffff)])
    {
        assert!(
            covered(b.translation_key()),
            "no catalog key for ButtonId::{b:?}"
        );
    }
    // Grid keys bind from config.toml before any GUI list exists; their
    // labels must still resolve the day a picker offers them.
    for key in GridKey::ALL {
        assert!(
            covered(key.translation_key()),
            "no catalog key for GridKey::{key:?}"
        );
    }
    for d in GestureDirection::ALL {
        assert!(
            covered(d.translation_key()),
            "no catalog key for GestureDirection::{d:?}"
        );
    }
    for a in Action::catalog() {
        let key = a
            .translation_key()
            .expect("every catalog action is payload-free");
        assert!(covered(key), "no catalog key for Action::{a:?}");
        assert!(
            covered(a.category().translation_key()),
            "no catalog key for {:?}",
            a.category()
        );
    }
    for icon in ActionRingIcon::ALL {
        assert!(
            covered(icon.translation_key()),
            "no catalog key for {icon:?}"
        );
    }
    for preset in ThumbwheelPreset::ALL {
        assert!(
            covered(preset.translation_key()),
            "no catalog key for {preset:?}"
        );
    }
}

/// Recorded chord actions carry a payload, so the typed-key sweep above cannot
/// reach their keys. Render the real labels in a translated locale instead: a
/// mistyped key or placeholder name would leave the key or a raw `%{…}` behind.
#[test]
fn chord_action_labels_resolve() {
    let _locale = super::LOCALE_LOCK.lock();
    let chord = |text: &str| -> KeyCombo { text.parse().expect("test shortcut must be valid") };
    let label = |action: Action| localized_action_label(&action).to_string();

    rust_i18n::set_locale("zh-CN");
    let hold = label(Action::HoldShortcut(chord("Cmd+Tab")));
    let switcher = label(Action::TapKeyHoldingModifiers(chord("Cmd+Tab")));
    let bare_key = label(Action::TapKeyHoldingModifiers(chord("F5")));
    rust_i18n::set_locale("en");

    assert_eq!(hold, "按住 Cmd+Tab");
    assert_eq!(switcher, "按住 Cmd，点按 Tab");
    assert_eq!(bare_key, "点按 F5");
}

#[test]
fn macro_action_labels_resolve() {
    let _locale = super::LOCALE_LOCK.lock();
    let key = |name: &str| -> KeyboardUsage { name.parse().expect("test key must be valid") };
    let label = |action: Action| localized_action_label(&action).to_string();

    rust_i18n::set_locale("en");
    let named = label(Action::Macro(Box::new(Macro {
        name: "Quick Crouch".into(),
        on_press: vec![MacroStep::KeyDown(key("LeftCtrl"))],
        while_held: Vec::new(),
        on_release: Vec::new(),
        repeat_while_held: false,
    })));
    let single = label(Action::Macro(Box::new(Macro {
        name: String::new(),
        on_press: vec![MacroStep::KeyDown(key("RightCtrl"))],
        while_held: Vec::new(),
        on_release: Vec::new(),
        repeat_while_held: false,
    })));
    let multi = label(Action::Macro(Box::new(Macro {
        name: "   ".into(),
        on_press: vec![
            MacroStep::KeyDown(key("RightCtrl")),
            MacroStep::Delay { millis: 50 },
        ],
        while_held: Vec::new(),
        on_release: vec![MacroStep::KeyUp(key("RightCtrl"))],
        repeat_while_held: false,
    })));

    assert_eq!(named, "Macro: Quick Crouch");
    assert_eq!(single, "Macro (1 step)");
    assert_eq!(multi, "Macro (3 steps)");
}

/// Czech must resolve in the actual desktop backend, including the pairing
/// entry point and the authentication instructions.
#[test]
fn czech_pairing_translations_resolve() {
    assert_eq!(
        rust_i18n::t!("pairing.add_device", locale = "cs"),
        "Přidat zařízení"
    );
    assert_eq!(
        rust_i18n::t!(
            "pairing.keyboard_pairing_passkey_instructions",
            locale = "cs"
        ),
        "Na nové klávesnici napište tento kód a stiskněte Enter."
    );
    assert_eq!(
        rust_i18n::t!("pairing.paired_receiver_slot", locale = "cs", slot = 2),
        "Spárováno na pozici 2."
    );
}
