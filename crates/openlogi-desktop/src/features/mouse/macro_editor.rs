//! Multi-phase macro editor for the mouse button inspector.

use crate::state::AppState;
use crate::ui::components::DetailedTooltip as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    scroll::ScrollbarMode,
    v_flex,
};
use openlogi_core::binding::{Action, ButtonId, GestureDirection, KeyboardUsage, Macro, MacroStep};

use super::view::MouseModelView;
use crate::features::binding_editor::{PickFn, editor_section};
use crate::ui::components::{Toggle, control_button, control_input, localize_placeholder};
use crate::ui::section::section_label;
use crate::ui::theme::{Palette, Typography as _};

/// Which execution phase of a [`Macro`] the step-builder controls target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum MacroPhase {
    #[default]
    OnPress,
    WhileHeld,
    OnRelease,
}

impl MacroPhase {
    pub(super) const ALL: [Self; 3] = [Self::OnPress, Self::WhileHeld, Self::OnRelease];

    pub(super) const fn id(self) -> &'static str {
        match self {
            Self::OnPress => "on-press",
            Self::WhileHeld => "while-held",
            Self::OnRelease => "on-release",
        }
    }

    pub(super) fn label(self) -> gpui::SharedString {
        match self {
            Self::OnPress => tr!("actions.macro_on_press"),
            Self::WhileHeld => tr!("actions.macro_while_holding"),
            Self::OnRelease => tr!("actions.macro_on_release"),
        }
    }
}

/// Editable draft state for the mouse inspector's macro editor.
pub(super) struct MacroDraft {
    pub(super) name_input: Entity<InputState>,
    pub(super) on_press_input: Entity<InputState>,
    pub(super) while_held_input: Entity<InputState>,
    pub(super) on_release_input: Entity<InputState>,
    pub(super) key_input: Entity<InputState>,
    pub(super) delay_input: Entity<InputState>,
    pub(super) active_phase: MacroPhase,
    pub(super) repeat_while_held: bool,
    pub(super) invalid: bool,
    pub(super) recording: Option<super::macro_recorder::MacroRecording>,
    pub(super) record_focus: gpui::FocusHandle,
    /// Scroll position of the selected phase's step list.
    pub(super) steps_scroll: gpui::ScrollHandle,
    pub(super) text: super::macro_text::MacroTextDraft,
    baseline: Option<Macro>,
    pub(super) pending_action: Option<DraftChange>,
}

#[derive(Clone)]
pub(super) enum DraftChange {
    Assign(Action),
    LoadExample(Macro),
    UseDefault(ButtonId),
    GestureMode(ButtonId, bool),
}

impl MacroDraft {
    pub(super) fn new(window: &mut Window, cx: &mut Context<MouseModelView>) -> Self {
        let name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("actions.macro_name_placeholder"))
        });
        let on_press_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("actions.macro_sequence_placeholder"))
        });
        let while_held_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("actions.macro_sequence_placeholder"))
        });
        let on_release_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("actions.macro_sequence_placeholder"))
        });
        let key_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("actions.macro_key_placeholder"))
        });
        let delay_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("actions.macro_delay_placeholder"))
        });

        for input in [
            &name_input,
            &on_press_input,
            &while_held_input,
            &on_release_input,
            &key_input,
            &delay_input,
        ] {
            cx.subscribe(input, |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.macro_draft.invalid = false;
                    cx.notify();
                }
            })
            .detach();
        }

        let text = super::macro_text::MacroTextDraft::new(window, cx);
        Self {
            baseline: None,
            pending_action: None,
            text,
            name_input,
            on_press_input,
            while_held_input,
            on_release_input,
            key_input,
            delay_input,
            active_phase: MacroPhase::OnPress,
            repeat_while_held: false,
            invalid: false,
            recording: None,
            record_focus: cx.focus_handle(),
            steps_scroll: gpui::ScrollHandle::new(),
        }
    }

    pub(super) fn localize_placeholders(&self, window: &mut Window, cx: &mut App) {
        localize_placeholder(
            &self.name_input,
            tr!("actions.macro_name_placeholder"),
            window,
            cx,
        );
        for input in [
            &self.on_press_input,
            &self.while_held_input,
            &self.on_release_input,
        ] {
            localize_placeholder(input, tr!("actions.macro_sequence_placeholder"), window, cx);
        }
        localize_placeholder(
            &self.key_input,
            tr!("actions.macro_key_placeholder"),
            window,
            cx,
        );
        localize_placeholder(
            &self.delay_input,
            tr!("actions.macro_delay_placeholder"),
            window,
            cx,
        );
    }

    pub(super) fn clear(&mut self, window: &mut Window, cx: &mut App) {
        self.baseline = None;
        self.pending_action = None;
        self.recording = None;
        self.text.open = false;
        self.text.error = None;
        for input in [
            &self.name_input,
            &self.on_press_input,
            &self.while_held_input,
            &self.on_release_input,
            &self.key_input,
            &self.delay_input,
        ] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.active_phase = MacroPhase::OnPress;
        self.repeat_while_held = false;
        self.invalid = false;
    }

    pub(super) fn seed(&mut self, macro_def: &Macro, window: &mut Window, cx: &mut App) {
        self.baseline = Some(macro_def.clone());
        self.pending_action = None;
        self.recording = None;
        self.text.open = false;
        self.text.error = None;
        let name = macro_def.name.clone();
        let on_press = MacroStep::format_sequence(&macro_def.on_press);
        let while_held = MacroStep::format_sequence(&macro_def.while_held);
        let on_release = MacroStep::format_sequence(&macro_def.on_release);
        self.name_input
            .update(cx, |input, cx| input.set_value(name, window, cx));
        self.on_press_input
            .update(cx, |input, cx| input.set_value(on_press, window, cx));
        self.while_held_input
            .update(cx, |input, cx| input.set_value(while_held, window, cx));
        self.on_release_input
            .update(cx, |input, cx| input.set_value(on_release, window, cx));
        self.key_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.delay_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.active_phase = MacroPhase::OnPress;
        self.repeat_while_held = macro_def.repeat_while_held;
        self.invalid = false;
    }

    /// Load edited steps while retaining the saved assignment for dirty checks.
    pub(super) fn load_draft(&mut self, macro_def: &Macro, window: &mut Window, cx: &mut App) {
        let baseline = self.baseline.clone();
        self.seed(macro_def, window, cx);
        self.baseline = baseline;
    }

    pub(super) const fn phase_input(&self, phase: MacroPhase) -> &Entity<InputState> {
        match phase {
            MacroPhase::OnPress => &self.on_press_input,
            MacroPhase::WhileHeld => &self.while_held_input,
            MacroPhase::OnRelease => &self.on_release_input,
        }
    }

    pub(super) fn append_step(
        &mut self,
        phase: MacroPhase,
        step: MacroStep,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.stop_recording(window, cx);
        let input = self.phase_input(phase).clone();
        let current = input.read(cx).value().to_string();
        let Ok(mut steps) = MacroStep::parse_sequence(&current) else {
            self.invalid = true;
            return;
        };
        steps.push(step);
        let formatted = MacroStep::format_sequence(&steps);
        input.update(cx, |input, cx| input.set_value(formatted, window, cx));
        self.invalid = false;
    }

    pub(super) fn append_key_step(&mut self, down: bool, window: &mut Window, cx: &mut App) {
        let raw = self.key_input.read(cx).value().to_string();
        let Ok(key) = raw.trim().parse::<KeyboardUsage>() else {
            self.invalid = true;
            return;
        };
        let step = if down {
            MacroStep::KeyDown(key)
        } else {
            MacroStep::KeyUp(key)
        };
        self.append_step(self.active_phase, step, window, cx);
    }

    pub(super) fn append_delay_step(&mut self, window: &mut Window, cx: &mut App) {
        let raw = self.delay_input.read(cx).value().to_string();
        let Some(millis) = parse_delay_input(&raw) else {
            self.invalid = true;
            return;
        };
        self.append_step(self.active_phase, MacroStep::Delay { millis }, window, cx);
    }

    pub(super) fn remove_step(
        &mut self,
        phase: MacroPhase,
        index: usize,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.stop_recording(window, cx);
        let input = self.phase_input(phase).clone();
        let current = input.read(cx).value().to_string();
        let Ok(mut steps) = MacroStep::parse_sequence(&current) else {
            return;
        };
        if index < steps.len() {
            steps.remove(index);
            let formatted = MacroStep::format_sequence(&steps);
            input.update(cx, |input, cx| input.set_value(formatted, window, cx));
            self.invalid = false;
        }
    }

    pub(super) fn move_step(
        &mut self,
        phase: MacroPhase,
        index: usize,
        delta: isize,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.stop_recording(window, cx);
        let input = self.phase_input(phase).clone();
        let current = input.read(cx).value().to_string();
        let Ok(mut steps) = MacroStep::parse_sequence(&current) else {
            return;
        };
        let Some(target) = index.checked_add_signed(delta) else {
            return;
        };
        if index < steps.len() && target < steps.len() {
            steps.swap(index, target);
            let formatted = MacroStep::format_sequence(&steps);
            input.update(cx, |input, cx| input.set_value(formatted, window, cx));
            self.invalid = false;
        }
    }

    pub(super) fn build_macro(&self, cx: &App) -> Option<Macro> {
        self.parse_macro(cx)
            .filter(|macro_def| !macro_def.is_empty())
    }

    pub(super) fn parse_macro(&self, cx: &App) -> Option<Macro> {
        self.validate_macro(cx).ok()
    }

    pub(super) fn has_unsaved_changes(&self, cx: &App) -> bool {
        let parsed = if self.text.open {
            super::macro_text::parse_macro(&self.text.input.read(cx).value()).ok()
        } else {
            self.parse_macro(cx)
        };
        match (self.baseline.as_ref(), parsed.as_ref()) {
            (Some(baseline), Some(current)) => baseline != current,
            (None, Some(current)) => !current.is_empty() || !current.name.is_empty(),
            _ => true,
        }
    }

    pub(super) fn validate_macro(&self, cx: &App) -> Result<Macro, String> {
        let name = self.name_input.read(cx).value().trim().to_string();
        let parse = |phase: MacroPhase| {
            super::macro_validation::parse_phase(
                &phase.label(),
                &self.phase_input(phase).read(cx).value(),
            )
        };
        let on_press = parse(MacroPhase::OnPress)?;
        let while_held = parse(MacroPhase::WhileHeld)?;
        let on_release = parse(MacroPhase::OnRelease)?;
        let macro_def = Macro {
            name,
            on_press,
            while_held,
            on_release,
            repeat_while_held: self.repeat_while_held,
        };
        Ok(macro_def)
    }
}

fn parse_delay_input(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(millis) = trimmed.parse::<u64>() {
        return Some(millis);
    }
    match trimmed.parse::<MacroStep>() {
        Ok(MacroStep::Delay { millis }) => Some(millis),
        _ => None,
    }
}

/// Width of the phase selector beside each phase's step input, so the three
/// inputs line up.
const PHASE_LABEL_REM: f32 = 7.;

/// The tallest the selected phase's step list grows (about ten rows) before
/// it scrolls; shorter lists take only the height their rows need.
pub(super) const STEP_LIST_MAX_H: f32 = 268.;

/// The bordered surface the macro editor's regions sit on.
fn macro_card(pal: Palette) -> gpui::Div {
    v_flex()
        .flex_shrink_0()
        .rounded(pal.card_radius)
        .border_1()
        .border_color(pal.border)
        .bg(pal.panel)
        .overflow_hidden()
}

/// One logical region of the macro card, ruled off from the region above.
pub(super) fn card_region(pal: Palette) -> gpui::Div {
    v_flex()
        .gap_2()
        .px_3()
        .py_2p5()
        .border_t_1()
        .border_color(pal.border)
}

/// A captioned [`card_region`].
fn titled_region(title: SharedString, pal: Palette) -> gpui::Div {
    card_region(pal).child(section_label(title, pal))
}

/// The card's closing region: the error, if any, above right-aligned actions.
pub(super) fn card_footer(
    pal: Palette,
    error: Option<gpui::Div>,
    actions: impl IntoIterator<Item = AnyElement>,
) -> gpui::Div {
    card_region(pal)
        .bg(pal.page)
        .children(error)
        .child(h_flex().justify_end().flex_wrap().gap_1().children(actions))
}

fn example_picker(id: &'static str, view: &Entity<MouseModelView>) -> impl IntoElement {
    let view = view.clone();
    control_button(format!("{id}-macro-examples"))
        .outline()
        .label(tr!("actions.macro_examples"))
        .detailed_tooltip(tr!("actions.macro_examples_tooltip"))
        .dropdown_menu_with_anchor(gpui::Anchor::TopLeft, move |mut menu, _, _| {
            for preset in super::macro_presets::MacroPreset::ALL {
                let view = view.clone();
                menu = menu.item(PopupMenuItem::new(preset.label()).on_click(
                    move |_, window, cx| {
                        view.update(cx, |view, cx| {
                            match preset.build() {
                                Ok(example) if view.macro_draft.has_unsaved_changes(cx) => {
                                    view.macro_draft.pending_action =
                                        Some(DraftChange::LoadExample(example));
                                }
                                Ok(example) => {
                                    view.macro_draft.load_draft(&example, window, cx);
                                }
                                Err(_) => view.macro_draft.invalid = true,
                            }
                            cx.notify();
                        });
                    },
                ));
            }
            menu
        })
}

pub(super) fn replacement_prompt(
    id: &'static str,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    on_pick: &PickFn,
    pal: Palette,
) -> Option<gpui::Div> {
    let pending = draft.pending_action.clone()?;
    let keep = view.clone();
    let discard = view.clone();
    let pick = on_pick.clone();
    Some(
        v_flex()
            .debug_selector(move || format!("{id}-macro-unsaved"))
            .flex_shrink_0()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(pal.border)
            .rounded(pal.card_radius)
            .child(div().text_body().child(tr!("actions.macro_unsaved_title")))
            .child(
                div()
                    .text_caption()
                    .text_color(pal.text_muted)
                    .child(tr!("actions.macro_unsaved_help")),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        control_button(format!("{id}-macro-keep-editing"))
                            .debug_selector(move || format!("{id}-macro-keep-editing"))
                            .outline()
                            .label(tr!("actions.macro_keep_editing"))
                            .detailed_tooltip(tr!("actions.macro_keep_editing_tooltip"))
                            .on_click(move |_, _, cx| {
                                keep.update(cx, |view, cx| {
                                    view.macro_draft.pending_action = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        control_button(format!("{id}-macro-discard-replace"))
                            .debug_selector(move || format!("{id}-macro-discard-replace"))
                            .outline()
                            .label(tr!("actions.macro_discard_continue"))
                            .detailed_tooltip(tr!("actions.macro_discard_continue_tooltip"))
                            .on_click(move |_, window, cx| {
                                discard.update(cx, |view, cx| {
                                    view.macro_draft.pending_action = None;
                                    cx.notify();
                                });
                                apply_draft_change(&pending, &discard, &pick, window, cx);
                            }),
                    )
                    .child(if draft.text.open {
                        super::macro_text::save_text_button("macro-pending", view, on_pick)
                            .into_any_element()
                    } else {
                        save_button("macro-pending", view, on_pick).into_any_element()
                    }),
            ),
    )
}

/// Request a different action or button mode without silently losing a draft.
pub(super) fn request_draft_change(
    change: DraftChange,
    view: &Entity<MouseModelView>,
    on_pick: &PickFn,
    window: &mut Window,
    cx: &mut App,
) {
    if view.read(cx).macro_draft.has_unsaved_changes(cx) {
        view.update(cx, |view, cx| {
            view.macro_draft.pending_action = Some(change);
            view.show_action_picker();
            cx.notify();
        });
    } else {
        apply_draft_change(&change, view, on_pick, window, cx);
    }
}

fn apply_draft_change(
    change: &DraftChange,
    view: &Entity<MouseModelView>,
    on_pick: &PickFn,
    window: &mut Window,
    cx: &mut App,
) {
    match change {
        DraftChange::Assign(action) => {
            view.update(cx, |view, cx| view.clear_custom_action_drafts(window, cx));
            (on_pick)(action.clone(), window, cx);
        }
        DraftChange::LoadExample(example) => view.update(cx, |view, cx| {
            view.macro_draft.load_draft(example, window, cx);
            cx.notify();
        }),
        DraftChange::UseDefault(button) => {
            AppState::apply(cx, |state| state.clear_app_binding(*button));
            view.update(cx, |view, cx| {
                view.clear_custom_action_drafts(window, cx);
                view.close_action_picker();
                cx.notify();
            });
        }
        DraftChange::GestureMode(button, enabled) => {
            AppState::apply(cx, |state| state.commit_gesture_mode(*button, *enabled));
            view.update(cx, |view, cx| {
                view.clear_custom_action_drafts(window, cx);
                view.set_gesture_selected_dir(enabled.then_some(GestureDirection::Click));
                cx.notify();
            });
        }
    }
}

/// A danger-tinted message explaining why the macro cannot be used yet.
pub(super) fn error_callout(
    id_prefix: &'static str,
    suffix: &'static str,
    message: impl Into<SharedString>,
    pal: Palette,
    cx: &App,
) -> gpui::Div {
    let danger = cx.theme().danger;
    h_flex()
        .debug_selector(move || format!("{id_prefix}-{suffix}"))
        .items_start()
        .gap_2()
        .px_2p5()
        .py_1p5()
        .rounded(pal.control_radius)
        .border_1()
        .border_color(danger.opacity(0.4))
        .bg(danger.opacity(0.08))
        .child(
            Icon::new(IconName::TriangleAlert)
                .small()
                .flex_shrink_0()
                .text_color(danger),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_caption()
                .text_color(danger)
                .child(message.into()),
        )
}

/// Render the multi-phase macro editor section in the action picker.
pub(super) fn macro_editor(
    id_prefix: &'static str,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    on_pick: &PickFn,
    pal: Palette,
    cx: &App,
    target: &str,
) -> impl IntoElement {
    let card = macro_card(pal).child(super::macro_text::text_toolbar(id_prefix, draft, view, pal));
    let card = card
        .key_context("MacroEditor")
        .on_action(|_: &super::macro_guide::OpenMacroGuide, _, cx| super::macro_guide::open(cx));
    let card = if draft.text.open {
        card.children(super::macro_text::text_editor(
            id_prefix, draft, view, pal, on_pick, cx, target,
        ))
    } else {
        let error = draft.invalid.then(|| {
            error_callout(
                id_prefix,
                "macro-error",
                draft
                    .validate_macro(cx)
                    .err()
                    .unwrap_or_else(|| tr!("actions.macro_invalid").to_string()),
                pal,
                cx,
            )
        });
        card.child(identity_region(id_prefix, draft, view, pal))
            .child(step_builder_region(id_prefix, draft, view, pal))
            .child(phases_region(id_prefix, draft, view, pal, cx))
            .children(draft.parse_macro(cx).map(|macro_def| {
                card_region(pal).child(
                    div()
                        .text_caption()
                        .text_color(pal.text_muted)
                        .child(super::macro_validation::summary(&macro_def, target)),
                )
            }))
            .child(card_footer(
                pal,
                error,
                [save_button(id_prefix, view, on_pick).into_any_element()],
            ))
    };
    v_flex()
        .flex_shrink_0()
        .gap_1()
        .child(editor_section(tr!("actions.macro_heading"), pal))
        .child(card)
}

/// What the macro is called and how it behaves while held.
fn identity_region(
    id_prefix: &'static str,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    pal: Palette,
) -> gpui::Div {
    let repeat_view = view.clone();
    card_region(pal)
        .child(example_picker(id_prefix, view))
        .child(
            div()
                .min_w_0()
                .debug_selector(move || format!("{id_prefix}-macro-name-input"))
                .child(control_input(&draft.name_input).cleanable(true)),
        )
        .child(
            h_flex()
                .debug_selector(move || format!("{id_prefix}-macro-repeat-row"))
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .text_caption()
                        .text_color(pal.text_primary)
                        .child(tr!("actions.macro_repeat_while_held")),
                )
                .child(
                    div()
                        .id(format!("{id_prefix}-macro-repeat-tooltip"))
                        .tooltip(|window, cx| {
                            crate::ui::components::detailed_tooltip(
                                tr!("actions.macro_repeat_tooltip"),
                                window,
                                cx,
                            )
                        })
                        .debug_selector(move || format!("{id_prefix}-macro-repeat-toggle"))
                        .child(
                            Toggle::new(format!("{id_prefix}-macro-repeat-switch"))
                                .selected(draft.repeat_while_held)
                                .on_change(move |enabled, _window, cx| {
                                    repeat_view.update(cx, |view, cx| {
                                        view.macro_draft.repeat_while_held = *enabled;
                                        cx.notify();
                                    });
                                }),
                        ),
                ),
        )
}

/// Every way to append a step to the selected phase: recording, or one key
/// edge or delay at a time.
fn step_builder_region(
    id_prefix: &'static str,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    pal: Palette,
) -> gpui::Div {
    let key_down_view = view.clone();
    let key_up_view = view.clone();
    let delay_view = view.clone();
    titled_region(tr!("actions.macro_steps_heading"), pal)
        .child(recorder(id_prefix, draft, view, pal))
        .child(
            h_flex()
                .flex_wrap()
                .gap_1()
                .child(
                    div()
                        .w_full()
                        .flex_shrink_0()
                        .min_w_0()
                        .debug_selector(move || format!("{id_prefix}-macro-key-input"))
                        .child(control_input(&draft.key_input).cleanable(true)),
                )
                .child(
                    control_button(format!("{id_prefix}-macro-add-key-down"))
                        .debug_selector(move || format!("{id_prefix}-macro-add-key-down"))
                        .outline()
                        .label(tr!("actions.macro_step_key_down"))
                        .detailed_tooltip(tr!("actions.macro_key_down_tooltip"))
                        .on_click(move |_, window, cx| {
                            key_down_view.update(cx, |view, cx| {
                                view.macro_draft.append_key_step(true, window, cx);
                                cx.notify();
                            });
                        }),
                )
                .child(
                    control_button(format!("{id_prefix}-macro-add-key-up"))
                        .debug_selector(move || format!("{id_prefix}-macro-add-key-up"))
                        .outline()
                        .label(tr!("actions.macro_step_key_up"))
                        .detailed_tooltip(tr!("actions.macro_key_up_tooltip"))
                        .on_click(move |_, window, cx| {
                            key_up_view.update(cx, |view, cx| {
                                view.macro_draft.append_key_step(false, window, cx);
                                cx.notify();
                            });
                        }),
                ),
        )
        .child(
            h_flex()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .debug_selector(move || format!("{id_prefix}-macro-delay-input"))
                        .child(control_input(&draft.delay_input).cleanable(true)),
                )
                .child(
                    control_button(format!("{id_prefix}-macro-add-delay"))
                        .debug_selector(move || format!("{id_prefix}-macro-add-delay"))
                        .outline()
                        .label(tr!("actions.macro_step_delay"))
                        .detailed_tooltip(tr!("actions.macro_delay_tooltip"))
                        .on_click(move |_, window, cx| {
                            delay_view.update(cx, |view, cx| {
                                view.macro_draft.append_delay_step(window, cx);
                                cx.notify();
                            });
                        }),
                ),
        )
}

/// The record toggle, which owns keyboard focus while it captures keys.
fn recorder(
    id_prefix: &'static str,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    pal: Palette,
) -> gpui::Div {
    let record_view = view.clone();
    let recording = draft.recording.is_some();
    v_flex()
        .gap_1()
        .child(
            div()
                .track_focus(&draft.record_focus)
                .on_key_up({
                    let view = view.clone();
                    move |event, window, cx| {
                        view.update(cx, |view, cx| {
                            view.macro_draft.record_key_up(&event.keystroke, window, cx);
                            cx.notify();
                        });
                    }
                })
                .on_modifiers_changed({
                    let view = view.clone();
                    move |event, window, cx| {
                        view.update(cx, |view, cx| {
                            view.macro_draft
                                .record_modifiers(event.modifiers, window, cx);
                            cx.notify();
                        });
                    }
                })
                .child(
                    control_button(format!("{id_prefix}-macro-record"))
                        .debug_selector(move || format!("{id_prefix}-macro-record"))
                        .w_full()
                        .outline()
                        .selected(recording)
                        .detailed_tooltip(tr!("actions.macro_record_tooltip"))
                        .label(if recording {
                            tr!("actions.macro_record_stop")
                        } else {
                            tr!("actions.macro_record")
                        })
                        .on_click(move |_, window, cx| {
                            record_view.update(cx, |view, cx| {
                                super::macro_recorder::toggle_recording(
                                    &mut view.macro_draft,
                                    window,
                                    cx,
                                );
                                cx.notify();
                            });
                        }),
                ),
        )
        .child(
            div()
                .text_caption()
                .text_color(pal.text_muted)
                .child(tr!("actions.macro_record_help")),
        )
}

/// The three phases' step text, and the selected phase's steps as rows.
fn phases_region(
    id_prefix: &'static str,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    pal: Palette,
    cx: &App,
) -> gpui::Div {
    let active_phase = draft.active_phase;
    titled_region(tr!("actions.macro_phases_heading"), pal)
        .children(
            MacroPhase::ALL
                .into_iter()
                .map(|phase| phase_row(id_prefix, phase, active_phase == phase, draft, view, cx)),
        )
        .children(active_phase_steps(
            id_prefix,
            active_phase,
            draft,
            view,
            pal,
            cx,
        ))
}

fn save_button(
    id_prefix: &'static str,
    view: &Entity<MouseModelView>,
    on_pick: &PickFn,
) -> impl IntoElement {
    let save_view = view.clone();
    let on_pick = on_pick.clone();
    control_button(format!("{id_prefix}-macro-save"))
        .debug_selector(move || format!("{id_prefix}-macro-save"))
        .primary()
        .label(tr!("actions.macro_save"))
        .detailed_tooltip(tr!("actions.macro_save_tooltip"))
        .on_click(move |_, window, cx| {
            save_view.update(cx, |view, cx| view.macro_draft.stop_recording(window, cx));
            let built = save_view.read(cx).macro_draft.build_macro(cx);
            match built {
                Some(macro_def) => {
                    save_macro(macro_def, &save_view, &on_pick, window, cx);
                }
                None => save_view.update(cx, |view, cx| {
                    view.macro_draft.invalid = true;
                    cx.notify();
                }),
            }
        })
}

pub(super) fn save_macro(
    macro_def: Macro,
    view: &Entity<MouseModelView>,
    on_pick: &PickFn,
    window: &mut Window,
    cx: &mut App,
) {
    view.update(cx, |view, cx| view.macro_draft.seed(&macro_def, window, cx));
    (on_pick)(Action::Macro(Box::new(macro_def)), window, cx);
}

fn phase_row(
    id_prefix: &'static str,
    phase: MacroPhase,
    selected: bool,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    cx: &App,
) -> gpui::Div {
    let phase_id = phase.id();
    let accent = cx.theme().primary;
    let input = draft.phase_input(phase);
    let select_view = view.clone();

    h_flex()
        .flex_shrink_0()
        .items_center()
        .gap_1()
        .child(
            control_button(format!("{id_prefix}-macro-phase-{phase_id}"))
                .debug_selector(move || format!("{id_prefix}-macro-phase-{phase_id}"))
                .w(gpui::rems(PHASE_LABEL_REM))
                .flex_shrink_0()
                .outline()
                .selected(selected)
                .when(selected, |button| {
                    button.border_color(accent).text_color(accent)
                })
                .label(phase.label())
                .detailed_tooltip(match phase {
                    MacroPhase::OnPress => tr!("actions.macro_press_tooltip"),
                    MacroPhase::WhileHeld => tr!("actions.macro_held_tooltip"),
                    MacroPhase::OnRelease => tr!("actions.macro_release_tooltip"),
                })
                .on_click(move |_, window, cx| {
                    select_view.update(cx, |view, cx| {
                        view.macro_draft.stop_recording(window, cx);
                        view.macro_draft.active_phase = phase;
                        cx.notify();
                    });
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .debug_selector(move || format!("{id_prefix}-macro-{phase_id}-input"))
                .child(control_input(input).cleanable(true)),
        )
}

fn active_phase_steps(
    id_prefix: &'static str,
    phase: MacroPhase,
    draft: &MacroDraft,
    view: &Entity<MouseModelView>,
    pal: Palette,
    cx: &App,
) -> Option<gpui::Stateful<gpui::Div>> {
    let phase_id = phase.id();
    let raw = draft.phase_input(phase).read(cx).value().to_string();
    let parsed_steps = MacroStep::parse_sequence(&raw).unwrap_or_default();
    let step_count = parsed_steps.len();
    if parsed_steps.is_empty() {
        return None;
    }

    // The list takes its rows' height up to the cap, then scrolls; the wheel
    // scrolls this list first, then the page when it reaches a boundary.
    Some(
        div()
            .id(format!("{id_prefix}-macro-{phase_id}-steps-frame"))
            .debug_selector(move || format!("{id_prefix}-macro-steps-scroll"))
            .relative()
            .flex_none()
            .child(
                crate::ui::scroll::SmoothScroll::new(format!("{id_prefix}-macro-{phase_id}-steps"))
                    .max_h(px(STEP_LIST_MAX_H))
                    .with_handle(&draft.steps_scroll)
                    .scrollbar_mode(ScrollbarMode::Always)
                    .child(v_flex().gap_0p5().pr_2p5().children(
                        parsed_steps.into_iter().enumerate().map(|(idx, step)| {
                            phase_step_row(id_prefix, phase, idx, step_count, &step, view, pal)
                        }),
                    )),
            ),
    )
}

fn phase_step_row(
    id_prefix: &'static str,
    phase: MacroPhase,
    idx: usize,
    step_count: usize,
    step: &MacroStep,
    view: &Entity<MouseModelView>,
    pal: Palette,
) -> gpui::Div {
    let phase_id = phase.id();
    let step_text = step.to_string();
    let up_view = view.clone();
    let down_view = view.clone();
    let remove_view = view.clone();
    h_flex()
        .debug_selector(move || format!("{id_prefix}-macro-{phase_id}-step-{idx}"))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .gap_1()
        .px_2()
        .py_0p5()
        .rounded(pal.control_radius)
        .border_1()
        .border_color(pal.border)
        .bg(pal.control)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_caption()
                .text_color(pal.text_primary)
                .child(step_text),
        )
        .child(
            h_flex()
                .items_center()
                .gap_0p5()
                .child(
                    Button::new(format!("{id_prefix}-macro-{phase_id}-step-up-{idx}"))
                        .debug_selector(move || {
                            format!("{id_prefix}-macro-{phase_id}-step-up-{idx}")
                        })
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronUp)
                        .accessibility_label(tr!("actions.macro_move_up"))
                        .detailed_tooltip(tr!("actions.macro_move_up_tooltip"))
                        .disabled(idx == 0)
                        .on_click(move |_, window, cx| {
                            up_view.update(cx, |view, cx| {
                                view.macro_draft.move_step(phase, idx, -1, window, cx);
                                cx.notify();
                            });
                        }),
                )
                .child(
                    Button::new(format!("{id_prefix}-macro-{phase_id}-step-down-{idx}"))
                        .debug_selector(move || {
                            format!("{id_prefix}-macro-{phase_id}-step-down-{idx}")
                        })
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronDown)
                        .accessibility_label(tr!("actions.macro_move_down"))
                        .detailed_tooltip(tr!("actions.macro_move_down_tooltip"))
                        .disabled(idx + 1 >= step_count)
                        .on_click(move |_, window, cx| {
                            down_view.update(cx, |view, cx| {
                                view.macro_draft.move_step(phase, idx, 1, window, cx);
                                cx.notify();
                            });
                        }),
                )
                .child(
                    Button::new(format!("{id_prefix}-macro-{phase_id}-step-remove-{idx}"))
                        .debug_selector(move || {
                            format!("{id_prefix}-macro-{phase_id}-step-remove-{idx}")
                        })
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .accessibility_label(tr!("actions.macro_remove_step"))
                        .detailed_tooltip(tr!("actions.macro_remove_step_tooltip"))
                        .on_click(move |_, window, cx| {
                            remove_view.update(cx, |view, cx| {
                                view.macro_draft.remove_step(phase, idx, window, cx);
                                cx.notify();
                            });
                        }),
                ),
        )
}
