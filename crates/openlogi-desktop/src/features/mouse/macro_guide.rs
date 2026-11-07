//! Macro notation reference, kept beside the editor and its parser adapter.

use crate::ui::components::DetailedTooltip as _;
use gpui::{
    App, ClipboardItem, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Size, Styled as _, Subscription, Window, actions,
    div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    Disableable as _, button::ButtonVariants as _, h_flex, text::TextView, v_flex,
};
use openlogi_core::binding::KeyboardUsage;
use std::path::PathBuf;

use crate::app::menu::{CloseWindow, Minimize, Zoom};
use crate::state::{AppState, StateEvent};
use crate::ui::components::control_button;
use crate::ui::theme::{self, Palette, Typography as _};
use crate::windows::{self, AuxWindow};

actions!(macro_guide, [OpenMacroGuide]);

const AGENT_GUIDE_FILE: &str = "MACRO-AGENT-GUIDE.md";

fn agent_guide_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|dir| dir.join(AGENT_GUIDE_FILE))
}

pub(crate) fn window_title() -> SharedString {
    tr!("actions.macro_guide_title")
}

pub(crate) fn open(cx: &mut App) {
    windows::open_or_focus(
        |registry| &mut registry.macro_guide,
        window_title(),
        Size::new(px(740.), px(680.)),
        MacroGuide::new,
        cx,
    );
}

pub(super) struct MacroGuide {
    focus: FocusHandle,
    appearance: Option<Subscription>,
    _language: Option<Subscription>,
    pub(super) scroll: gpui::ScrollHandle,
}

impl MacroGuide {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            scroll: gpui::ScrollHandle::new(),
            focus,
            appearance: None,
            _language: AppState::try_global(cx).map(|_| {
                AppState::repaint_on(cx, |event| matches!(event, StateEvent::LanguageChanged))
            }),
        }
    }
}

impl AuxWindow for MacroGuide {
    fn set_appearance_obs(&mut self, sub: Subscription) {
        self.appearance = Some(sub);
    }
}

fn reference() -> SharedString {
    // The validated usage type owns which keys exist and how they are spelled.
    let keys = (u8::MIN..=u8::MAX)
        .filter_map(|code| KeyboardUsage::try_from(code).ok())
        .map(|key| format!("`` {} ``", key.label()))
        .collect::<Vec<_>>()
        .join(" · ");
    tr!("actions.macro_guide_body", keys => keys)
}

impl Render for MacroGuide {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        theme::apply_ui_scale(window, cx);
        let pal = theme::palette(cx);
        v_flex()
            .size_full()
            .bg(pal.page)
            .text_color(pal.text_primary)
            .track_focus(&self.focus)
            .key_context("MacroGuide")
            .on_action(|_: &CloseWindow, window, _| window.remove_window())
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Zoom, window, _| window.zoom_window())
            .when(windows::needs_client_titlebar(window), |view| {
                view.child(windows::aux_title_bar(window_title(), cx))
            })
            .child(
                v_flex()
                    .flex_shrink_0()
                    .p_5()
                    .gap_1()
                    .child(div().text_heading().child(window_title()))
                    .child(
                        div()
                            .text_body()
                            .text_color(pal.text_muted)
                            .child(tr!("actions.macro_guide_intro")),
                    ),
            )
            .child(
                crate::ui::scroll::SmoothScroll::new("macro-guide-scroll")
                    .flex_1()
                    .min_h_0()
                    .with_handle(&self.scroll)
                    .child(
                        v_flex()
                            .flex_shrink_0()
                            .w_full()
                            .px_5()
                            .pb_5()
                            .gap_5()
                            .child(agent_instructions(pal))
                            .child(
                                TextView::markdown("macro-guide-reference", reference())
                                    .flex_shrink_0()
                                    .selectable(true),
                            )
                            .children(copyable_examples()),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .justify_end()
                    .p_3()
                    .border_t_1()
                    .border_color(pal.border)
                    .child(
                        control_button("macro-guide-close")
                            .outline()
                            .label(tr!("common.done"))
                            .detailed_tooltip(tr!("actions.macro_guide_close_tooltip"))
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            )
    }
}

fn agent_instructions(pal: Palette) -> impl IntoElement {
    let guide = agent_guide_path();
    let exists = guide.as_ref().is_some_and(|path| path.is_file());
    let path = guide
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let open_path = guide.and_then(|path| url::Url::from_file_path(path).ok());
    v_flex()
        .flex_shrink_0()
        .gap_2()
        .p_3()
        .border_1()
        .border_color(pal.border)
        .rounded(pal.card_radius)
        .child(div().text_body().child(tr!("actions.macro_agent_title")))
        .child(
            div()
                .text_caption()
                .text_color(pal.text_muted)
                .child(tr!("actions.macro_agent_help")),
        )
        .child(div().text_caption().truncate().child(path.clone()))
        .when(!exists, |view| {
            view.child(
                div()
                    .text_caption()
                    .text_color(pal.text_muted)
                    .child(tr!("actions.macro_agent_missing")),
            )
        })
        .child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .child(
                    control_button("macro-agent-open")
                        .debug_selector(|| "macro-agent-open".into())
                        .outline()
                        .disabled(!exists)
                        .label(tr!("actions.macro_agent_open"))
                        .detailed_tooltip(tr!("actions.macro_agent_open_tooltip"))
                        .on_click(move |_, _, cx| {
                            if let Some(url) = &open_path {
                                cx.open_url(url.as_str());
                            }
                        }),
                )
                .child(
                    control_button("macro-agent-copy-path")
                        .debug_selector(|| "macro-agent-copy-path".into())
                        .outline()
                        .disabled(path.is_empty())
                        .label(tr!("actions.macro_agent_copy_path"))
                        .detailed_tooltip(tr!("actions.macro_agent_copy_path_tooltip"))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                        }),
                ),
        )
}

fn copyable_examples() -> impl Iterator<Item = gpui::Div> {
    super::macro_presets::MacroPreset::ALL
        .into_iter()
        .filter_map(|preset| {
            let macro_def = preset.build().ok()?;
            let text = super::macro_text::format_macro(&macro_def);
            Some(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(div().text_body().child(preset.label()))
                            .child(
                                control_button(format!("guide-copy-{}", preset.id()))
                                    .ghost()
                                    .label(tr!("actions.macro_copy_example"))
                                    .detailed_tooltip(tr!("actions.macro_copy_example_tooltip"))
                                    .on_click({
                                        let text = text.clone();
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                text.clone(),
                                            ));
                                        }
                                    }),
                            ),
                    )
                    .child(
                        TextView::markdown(
                            format!("guide-example-{}", preset.id()),
                            format!("```json\n{text}\n```"),
                        )
                        .selectable(true),
                    ),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn agent_guide_path_follows_the_running_executable(cx: &mut gpui::TestAppContext) {
        let _locale = crate::services::i18n::LOCALE_LOCK.lock().unwrap();
        rust_i18n::set_locale("en");
        cx.update(gpui_component::init);
        let (_, visual) = cx.add_window_view(MacroGuide::new);
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let button = visual.debug_bounds("macro-agent-copy-path").unwrap();
        visual.simulate_click(button.center(), gpui::Modifiers::default());
        let copied = visual.update(|_, cx| cx.read_from_clipboard().unwrap().text().unwrap());
        let expected = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join(AGENT_GUIDE_FILE);
        assert_eq!(copied, expected.display().to_string());
    }
}
