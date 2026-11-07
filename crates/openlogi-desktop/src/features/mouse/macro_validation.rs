//! User-facing validation and summaries for both representations of a macro.

use gpui::SharedString;
use openlogi_core::binding::{Macro, MacroStep};
use std::error::Error as _;

pub(super) fn parse_phase(phase: &str, text: &str) -> Result<Vec<MacroStep>, String> {
    MacroStep::parse_sequence_detailed(text).map_err(|error| {
        let token = error.token();
        let key = token.trim_start_matches(['+', '-', '↓', '↑']).to_ascii_lowercase();
        let hint = match key.as_str() {
            "ctrl" | "control" | "ctlr" | "leftctlr" => tr!("actions.macro_key_suggestion", key => "LeftCtrl"),
            "shift" => tr!("actions.macro_key_suggestion", key => "LeftShift"),
            "alt" | "option" => tr!("actions.macro_key_suggestion", key => "LeftAlt"),
            "win" | "cmd" | "command" | "super" => tr!("actions.macro_key_suggestion", key => "LeftWin"),
            _ => tr!("actions.macro_error_guide_hint"),
        };
        let detail = error.source().map(ToString::to_string).unwrap_or_default();
        tr!("actions.macro_phase_error", phase => phase, step => error.step(), token => token, detail => detail, hint => hint).to_string()
    })
}

pub(super) fn summary(macro_def: &Macro, target: &str) -> SharedString {
    let phases = [
        &macro_def.on_press,
        &macro_def.while_held,
        &macro_def.on_release,
    ];
    let count: usize = phases.iter().map(|steps| steps.len()).sum();
    let delay = phases
        .into_iter()
        .flatten()
        .fold(0u64, |total, step| match step {
            MacroStep::Delay { millis } => total.saturating_add(*millis),
            _ => total,
        });
    tr!("actions.macro_summary", count => count, delay => delay, target => target,
        repeat => if macro_def.repeat_while_held { tr!("actions.macro_summary_repeat") } else { tr!("actions.macro_summary_once") })
}
