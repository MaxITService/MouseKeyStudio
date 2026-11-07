//! Display helpers for the shared action vocabulary.

use gpui::SharedString;
use openlogi_core::binding::{Action, ButtonId, GamingLayout};

/// Localized physical button name, with its G-series identifier where useful.
pub(crate) fn localized_button_label(
    button: ButtonId,
    layout: Option<&GamingLayout>,
) -> SharedString {
    let name = tr!(button.translation_key());
    if matches!(button, ButtonId::Back | ButtonId::Forward)
        && let Some(slot) = layout.and_then(|layout| layout.slot_of(button))
    {
        format!("{name} G{}", slot + 1).into()
    } else {
        name
    }
}

/// Localized label for an action, including its dynamic payload.
#[expect(
    clippy::expect_used,
    reason = "the preceding match arms handle every action without a static key"
)]
pub(crate) fn localized_action_label(action: &Action) -> SharedString {
    match action {
        Action::SetDpiPreset(index) => {
            tr!("pointer.dpi_preset", index => (index + 1).to_string())
        }
        Action::CustomShortcut(combo) => combo.rendered_label().into(),
        Action::HoldShortcut(combo) => {
            tr!("actions.hold_shortcut", chord => combo.rendered_label())
        }
        Action::TapKeyHoldingModifiers(combo) => match combo.rendered_modifiers() {
            Some(modifiers) => tr!(
                "actions.hold_modifiers_tap_key",
                modifiers => modifiers,
                key => combo.rendered_key()
            ),
            None => tr!("actions.tap_key", key => combo.rendered_key()),
        },
        Action::TypeText(text) => tr!("actions.type_text_action", text => text.clone()),
        Action::RunAppleScript(_) => tr!("actions.run_applescript_heading"),
        Action::RunShellCommand(_) => tr!("actions.run_shell_command_heading"),
        Action::RunPowerShellCommand(_) => tr!("actions.shell_powershell_action"),
        Action::Workflow(steps) if steps.len() == 1 => {
            tr!("actions.workflow_step_count_singular")
        }
        Action::Workflow(steps) => {
            tr!("actions.workflow_step_count_plural", count => steps.len().to_string())
        }
        Action::OpenApplication(target) => {
            tr!("actions.open_named_target", name => target.display_name())
        }
        Action::Macro(sequence) => match sequence.display_name() {
            Some(name) => tr!("actions.macro_named_action", name => name),
            None if sequence.step_count() == 1 => tr!("actions.macro_step_count_singular"),
            None => tr!(
                "actions.macro_step_count_plural",
                count => sequence.step_count().to_string()
            ),
        },
        _ => tr!(action
            .translation_key()
            .expect("every payload-free action has a translation key")),
    }
}
