//! Editable examples; Helldivers sequences preserve the user's recorded timings.

use gpui::SharedString;
use openlogi_core::binding::{Macro, MacroStep, MacroStepParseError};

#[derive(Clone, Copy)]
pub(super) enum MacroPreset {
    Copy,
    Paste,
    DoubleTap,
    Eagle,
    Resupply,
}

impl MacroPreset {
    pub(super) const ALL: [Self; 5] = [
        Self::Copy,
        Self::Paste,
        Self::DoubleTap,
        Self::Eagle,
        Self::Resupply,
    ];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Paste => "paste",
            Self::DoubleTap => "double-tap",
            Self::Eagle => "eagle",
            Self::Resupply => "resupply",
        }
    }

    pub(super) fn label(self) -> SharedString {
        match self {
            Self::Copy => tr!("actions.macro_example_copy"),
            Self::Paste => tr!("actions.macro_example_paste"),
            Self::DoubleTap => tr!("actions.macro_example_double_tap"),
            Self::Eagle => "Helldivers: EAGLE Airstrike".into(),
            Self::Resupply => "Helldivers: Resupply".into(),
        }
    }

    pub(super) fn build(self) -> Result<Macro, MacroStepParseError> {
        let sequence = match self {
            Self::Copy => "+LeftCtrl, 50ms, +C, 50ms, -C, 50ms, -LeftCtrl",
            Self::Paste => "+LeftCtrl, 50ms, +V, 50ms, -LeftCtrl, 50ms, -V",
            Self::DoubleTap => "Space, 100ms, Space",
            Self::Eagle => {
                "+B, 50ms, +Up, 50ms, -Up, 50ms, +Right, 50ms, -Right, 50ms, +Down, 50ms, -Down, 50ms, +Right, 50ms, -Right, 50ms, -B"
            }
            Self::Resupply => {
                "+B, 50ms, +Down, 50ms, -Down, 50ms, +Down, 50ms, -Down, 50ms, +Up, 50ms, -Up, 50ms, +Right, 50ms, -Right, 50ms, -B"
            }
        };
        Ok(Macro {
            name: self.label().to_string(),
            on_press: MacroStep::parse_sequence(sequence)?,
            while_held: Vec::new(),
            on_release: Vec::new(),
            repeat_while_held: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_examples_parse_and_balance_every_held_key() {
        for preset in MacroPreset::ALL {
            let definition = preset.build().unwrap();
            let mut held = std::collections::HashSet::new();
            for step in &definition.on_press {
                match step {
                    MacroStep::KeyDown(key) => assert!(held.insert(*key)),
                    MacroStep::KeyUp(key) => assert!(held.remove(key)),
                    MacroStep::Delay { .. } => {}
                }
            }
            assert!(held.is_empty(), "{} leaves a key held", preset.id());
            assert_eq!(
                MacroStep::parse_sequence(&MacroStep::format_sequence(&definition.on_press))
                    .unwrap(),
                definition.on_press
            );
        }
    }
}
