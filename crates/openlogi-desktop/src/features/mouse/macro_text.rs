//! Complete, portable text representation of a macro, including every phase.

use crate::ui::components::DetailedTooltip as _;
use gpui::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Styled as _, Window, div,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _,
    button::ButtonVariants as _,
    h_flex,
    input::{InputEvent, Textarea, TextareaState},
};
use openlogi_core::binding::{Macro, MacroStep};
use serde::{Deserialize, Serialize};

use super::macro_editor::{MacroDraft, card_footer, card_region, error_callout};
use super::view::MouseModelView;
use crate::features::binding_editor::PickFn;
use crate::ui::components::control_button;
use crate::ui::theme::{Palette, Typography as _};

/// Tall enough to show a pretty-printed macro without scrolling.
const TEXT_EDITOR_REM: f32 = 18.;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MacroText {
    name: String,
    on_press: String,
    while_held: String,
    on_release: String,
    repeat_while_held: bool,
}

pub(super) fn format_macro(macro_def: &Macro) -> String {
    let text = MacroText {
        name: macro_def.name.clone(),
        on_press: MacroStep::format_sequence(&macro_def.on_press),
        while_held: MacroStep::format_sequence(&macro_def.while_held),
        on_release: MacroStep::format_sequence(&macro_def.on_release),
        repeat_while_held: macro_def.repeat_while_held,
    };
    // This DTO contains only strings and a boolean, which JSON can always encode.
    serde_json::to_string_pretty(&text).unwrap_or_default()
}

pub(super) fn parse_macro(text: &str) -> Result<Macro, String> {
    let text: MacroText = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let phase = |name: &str, text: &str| super::macro_validation::parse_phase(name, text);
    Ok(Macro {
        name: text.name,
        on_press: phase("on_press", &text.on_press)?,
        while_held: phase("while_held", &text.while_held)?,
        on_release: phase("on_release", &text.on_release)?,
        repeat_while_held: text.repeat_while_held,
    })
}

pub(super) struct MacroTextDraft {
    pub(super) open: bool,
    pub(super) error: Option<String>,
    pub(super) input: Entity<TextareaState>,
}

impl MacroTextDraft {
    pub(super) fn new(window: &mut Window, cx: &mut Context<MouseModelView>) -> Self {
        let input = cx.new(|cx| TextareaState::new(window, cx).rows(10));
        cx.subscribe(&input, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.macro_draft.text.error = None;
                cx.notify();
            }
        })
        .detach();
        Self {
            open: false,
            error: None,
            input,
        }
    }
}

impl MacroDraft {
    /// Replace the step editor with the complete JSON text of the current
    /// draft, or flag the draft invalid when its steps do not parse.
    pub(super) fn open_text_editor(&mut self, window: &mut Window, cx: &mut gpui::App) {
        self.stop_recording(window, cx);
        if let Some(macro_def) = self.parse_macro(cx) {
            self.text.input.update(cx, |input, cx| {
                input.set_value(format_macro(&macro_def), window, cx);
            });
            self.text.open = true;
            self.text.error = None;
        } else {
            self.invalid = true;
        }
    }
}

/// The strip across the top of the macro card: whole-macro actions that work
/// in both the step and the text editor.
pub(super) fn text_toolbar(
    id: &'static str,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    pal: Palette,
) -> impl IntoElement {
    let edit_view = view.clone();
    let copy_view = view.clone();
    let paste_view = view.clone();
    h_flex()
        .flex_wrap()
        .gap_1()
        .px_2()
        .py_1p5()
        .bg(pal.muted)
        .child(
            control_button(format!("{id}-macro-edit-text"))
                .debug_selector(move || format!("{id}-macro-edit-text"))
                .ghost()
                .icon(IconName::FileText)
                .selected(draft.text.open)
                .disabled(draft.text.open)
                .label(tr!("actions.macro_edit_text"))
                .detailed_tooltip(tr!("actions.macro_edit_text_tooltip"))
                .on_click(move |_, window, cx| {
                    edit_view.update(cx, |view, cx| {
                        if !view.macro_draft.text.open {
                            view.macro_draft.open_text_editor(window, cx);
                        }
                        cx.notify();
                    });
                }),
        )
        .child(
            control_button(format!("{id}-macro-copy"))
                .debug_selector(move || format!("{id}-macro-copy"))
                .ghost()
                .icon(IconName::Copy)
                .label(tr!("actions.macro_copy"))
                .detailed_tooltip(tr!("actions.macro_copy_tooltip"))
                .on_click(move |_, window, cx| {
                    copy_view.update(cx, |view, cx| {
                        let draft = &mut view.macro_draft;
                        draft.stop_recording(window, cx);
                        let text = if draft.text.open {
                            Some(draft.text.input.read(cx).value().to_string())
                        } else {
                            draft.parse_macro(cx).as_ref().map(format_macro)
                        };
                        if let Some(text) = text {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                        } else {
                            draft.invalid = true;
                        }
                        cx.notify();
                    });
                }),
        )
        .child(
            control_button(format!("{id}-macro-paste"))
                .debug_selector(move || format!("{id}-macro-paste"))
                .ghost()
                .icon(IconName::Inbox)
                .label(tr!("actions.macro_paste"))
                .detailed_tooltip(tr!("actions.macro_paste_tooltip"))
                .on_click(move |_, window, cx| {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        paste_view.update(cx, |view, cx| {
                            view.macro_draft.stop_recording(window, cx);
                            view.macro_draft
                                .text
                                .input
                                .update(cx, |input, cx| input.set_value(text, window, cx));
                            view.macro_draft.text.open = true;
                            view.macro_draft.text.error = None;
                            cx.notify();
                        });
                    }
                }),
        )
        .child(
            control_button(format!("{id}-macro-guide"))
                .debug_selector(move || format!("{id}-macro-guide"))
                .ghost()
                .icon(IconName::BookOpen)
                .label(tr!("actions.macro_guide"))
                .detailed_tooltip(tr!("actions.macro_guide_tooltip"))
                .on_click(|_, _, cx| super::macro_guide::open(cx)),
        )
}

/// The text-mode body of the macro card: the JSON editor region, then the
/// footer with its error and actions.
pub(super) fn text_editor(
    id: &'static str,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    pal: Palette,
    on_pick: &PickFn,
    cx: &App,
    target: &str,
) -> [AnyElement; 2] {
    let apply_view = view.clone();
    let cancel_view = view.clone();
    let editor = card_region(pal)
        .child(
            div()
                .text_caption()
                .text_color(pal.text_muted)
                .child(tr!("actions.macro_text_help")),
        )
        .child(
            div()
                .debug_selector(move || format!("{id}-macro-full-text"))
                .child(
                    Textarea::new(&draft.text.input)
                        .h(gpui::rems(TEXT_EDITOR_REM))
                        .font_family(cx.theme().mono_font_family.clone()),
                ),
        )
        .children(
            parse_macro(&draft.text.input.read(cx).value())
                .ok()
                .map(|macro_def| {
                    div()
                        .text_caption()
                        .text_color(pal.text_muted)
                        .child(super::macro_validation::summary(&macro_def, target))
                }),
        );
    let actions = [
        control_button(format!("{id}-macro-cancel-text"))
            .ghost()
            .label(tr!("actions.macro_discard_text"))
            .detailed_tooltip(tr!("actions.macro_discard_text_tooltip"))
            .on_click(move |_, _, cx| {
                cancel_view.update(cx, |view, cx| {
                    view.macro_draft.text.open = false;
                    view.macro_draft.text.error = None;
                    cx.notify();
                });
            })
            .into_any_element(),
        control_button(format!("{id}-macro-apply-text"))
            .debug_selector(move || format!("{id}-macro-apply-text"))
            .outline()
            .label(tr!("actions.macro_apply_text"))
            .detailed_tooltip(tr!("actions.macro_apply_text_tooltip"))
            .on_click(move |_, window, cx| {
                apply_view.update(cx, |view, cx| {
                    match parse_macro(&view.macro_draft.text.input.read(cx).value()) {
                        Ok(macro_def) => {
                            view.macro_draft.load_draft(&macro_def, window, cx);
                        }
                        Err(error) => view.macro_draft.text.error = Some(error),
                    }
                    cx.notify();
                });
            })
            .into_any_element(),
        save_text_button(id, view, on_pick).into_any_element(),
    ];
    let error = draft
        .text
        .error
        .clone()
        .map(|error| error_callout(id, "macro-text-error", error, pal, cx));
    [
        editor.into_any_element(),
        card_footer(pal, error, actions).into_any_element(),
    ]
}

pub(super) fn save_text_button(
    id: &'static str,
    view: &Entity<MouseModelView>,
    on_pick: &PickFn,
) -> impl IntoElement {
    let save_view = view.clone();
    let on_pick = on_pick.clone();
    control_button(format!("{id}-macro-save-text"))
        .debug_selector(move || format!("{id}-macro-save-text"))
        .primary()
        .label(tr!("actions.macro_save"))
        .detailed_tooltip(tr!("actions.macro_save_tooltip"))
        .on_click(move |_, window, cx| {
            let result = parse_macro(&save_view.read(cx).macro_draft.text.input.read(cx).value())
                .and_then(|macro_def| {
                    if macro_def.is_empty() {
                        Err(tr!("action_ring.custom_action_invalid_input").to_string())
                    } else {
                        Ok(macro_def)
                    }
                });
            match result {
                Ok(macro_def) => {
                    super::macro_editor::save_macro(macro_def, &save_view, &on_pick, window, cx);
                }
                Err(error) => save_view.update(cx, |view, cx| {
                    view.macro_draft.text.error = Some(error);
                    cx.notify();
                }),
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_guide_function_key_range_matches_the_macro_parser() {
        let guide = include_str!("../../../../../docs/MACRO-AGENT-GUIDE.md");
        let last: u8 = guide
            .split("F1–F")
            .nth(1)
            .unwrap()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap();
        for number in 1..=24 {
            assert_eq!(
                MacroStep::parse_sequence(&format!("F{number}")).is_ok(),
                number <= last,
                "the guide must describe the parser's F{number} support"
            );
        }
    }

    #[test]
    fn shipped_agent_guide_examples_are_valid_complete_macros() {
        let guide = include_str!("../../../../../docs/MACRO-AGENT-GUIDE.md");
        let examples: Vec<_> = guide
            .split("```json")
            .skip(1)
            .map(|block| block.split("```").next().unwrap())
            .collect();
        assert_eq!(examples.len(), 5);
        for example in examples {
            let macro_def = parse_macro(example).expect("shipped example must be pasteable");
            assert!(!macro_def.is_empty());
            assert_eq!(
                macro_def.is_held(),
                macro_def.name == "Hold B, enter sequence, release B",
                "only the final guide example follows physical release; the others finish after a click"
            );
            assert_eq!(parse_macro(&format_macro(&macro_def)).unwrap(), macro_def);
        }
    }

    #[test]
    fn macro_validation_names_phase_and_bad_step_with_a_correction() {
        let _locale = crate::services::i18n::LOCALE_LOCK.lock().unwrap();
        rust_i18n::set_locale("en");
        let mut json: serde_json::Value =
            serde_json::from_str(&format_macro(&Macro::default())).unwrap();
        json["on_release"] = "50ms, +Ctlr".into();
        let error = parse_macro(&json.to_string()).unwrap_err();
        assert!(error.contains("on_release"), "{error}");
        assert!(error.contains("step 2"), "{error}");
        assert!(error.contains("+Ctlr"), "{error}");
        assert!(error.contains("LeftCtrl"), "{error}");
    }

    #[test]
    fn complete_macro_text_roundtrips_every_phase_and_metadata() {
        let macro_def = Macro {
            name: "Copy \"this\"\n测试".into(),
            on_press: MacroStep::parse_sequence("+RightCtrl").unwrap(),
            while_held: MacroStep::parse_sequence("A, 50ms").unwrap(),
            on_release: MacroStep::parse_sequence("-RightCtrl").unwrap(),
            repeat_while_held: true,
        };
        assert_eq!(parse_macro(&format_macro(&macro_def)).unwrap(), macro_def);
        parse_macro(&format_macro(&macro_def).replace("50ms", "nonsense")).unwrap_err();
        parse_macro(&format_macro(&macro_def).replace("on_press", "on_pres")).unwrap_err();
        parse_macro("not json").unwrap_err();
    }
}
