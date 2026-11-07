//! Shell selection and examples shared by Windows command editors.

use gpui::{Context, Entity, IntoElement, Render, SharedString, Window};
use gpui_component::{input::InputState, v_flex};
use openlogi_core::binding::Action;

/// The shell is saved as a distinct action, so old CMD bindings keep their meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandShell {
    System,
    PowerShell,
}

impl CommandShell {
    fn example(self) -> &'static str {
        match self {
            Self::PowerShell => "Start-Process notepad.exe",
            Self::System if cfg!(target_os = "windows") => r#"start "" notepad.exe"#,
            Self::System => "echo hello",
        }
    }

    fn for_action(action: Option<&Action>) -> Self {
        match action {
            Some(Action::RunShellCommand(_)) => Self::System,
            Some(Action::RunPowerShellCommand(_)) => Self::PowerShell,
            _ if cfg!(target_os = "windows") => Self::PowerShell,
            _ => Self::System,
        }
    }
}

/// Retained shell choice for one command input. The parent owns the input's lifetime.
pub(crate) struct CommandDraft {
    shell: CommandShell,
    input: Entity<InputState>,
}

impl CommandDraft {
    pub(crate) fn default_placeholder() -> SharedString {
        CommandShell::for_action(None).example().into()
    }

    pub(crate) fn new(input: Entity<InputState>, action: Option<&Action>) -> Self {
        Self {
            shell: CommandShell::for_action(action),
            input,
        }
    }

    pub(crate) fn action(&self, text: String) -> Option<Action> {
        if text.trim().is_empty() {
            return None;
        }
        Some(match self.shell {
            CommandShell::System => Action::RunShellCommand(text),
            CommandShell::PowerShell => Action::RunPowerShellCommand(text),
        })
    }

    pub(crate) fn placeholder(&self) -> SharedString {
        self.shell.example().into()
    }

    pub(crate) fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_shell(CommandShell::for_action(None), window, cx);
    }

    fn set_shell(&mut self, shell: CommandShell, window: &mut Window, cx: &mut Context<Self>) {
        self.shell = shell;
        self.input.update(cx, |input, cx| {
            input.set_placeholder(shell.example(), window, cx);
        });
        cx.notify();
    }
}

impl Render for CommandDraft {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let help = v_flex();
        #[cfg(target_os = "windows")]
        let help = {
            use crate::ui::components::DetailedTooltip as _;
            use crate::ui::theme::Typography as _;
            use gpui::{InteractiveElement as _, ParentElement as _, Styled as _, div};
            use gpui_component::{Selectable as _, button::Button, h_flex};
            let pal = crate::ui::theme::palette(cx);
            let powershell = self.shell == CommandShell::PowerShell;
            let command = self.shell.example();
            let input = self.input.clone();
            help.gap_2()
                .px_2()
                .py_2()
                .text_caption()
                .text_color(pal.text_muted)
                .child(
                    h_flex().flex_wrap().gap_1().children(
                        [
                            (
                                CommandShell::PowerShell,
                                "PowerShell",
                                "shell-mode-powershell",
                                tr!("actions.shell_powershell_tooltip"),
                            ),
                            (
                                CommandShell::System,
                                "CMD",
                                "shell-mode-cmd",
                                tr!("actions.shell_cmd_tooltip"),
                            ),
                        ]
                        .into_iter()
                        .map(|(shell, label, id, tip)| {
                            Button::new(id)
                                .debug_selector(move || id.into())
                                .compact()
                                .label(label)
                                .selected(self.shell == shell)
                                .detailed_tooltip(tip)
                                .on_click(cx.listener(move |draft, _, window, cx| {
                                    draft.set_shell(shell, window, cx);
                                }))
                        }),
                    ),
                )
                .child(div().child(if powershell {
                    tr!("actions.shell_windows_powershell_help")
                } else {
                    tr!("actions.shell_windows_help")
                }))
                .child(
                    Button::new("shell-notepad-example")
                        .debug_selector(|| "shell-notepad-example".into())
                        .compact()
                        .label(tr!("actions.shell_notepad_example"))
                        .detailed_tooltip(tr!("actions.shell_example_tooltip", command => command))
                        .on_click(move |_, window, cx| {
                            input.update(cx, |input, cx| input.set_value(command, window, cx));
                        }),
                )
                .child(div().child(command))
                .child(div().child(if powershell {
                    tr!("actions.shell_windows_powershell_script_help")
                } else {
                    tr!("actions.shell_windows_script_help")
                }))
        };
        #[cfg(not(target_os = "windows"))]
        let _ = (&self.input, cx);
        help
    }
}

#[cfg(all(debug_assertions, feature = "ui-snapshots"))]
pub(crate) fn snapshot_scenes() -> Vec<crate::ui::snapshots::Scene> {
    use gpui::{AppContext as _, ParentElement as _, Styled as _, px, size};
    use std::rc::Rc;
    struct CommandScene {
        input: Entity<InputState>,
        draft: Entity<CommandDraft>,
    }
    impl Render for CommandScene {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            v_flex().p_3().child(super::command_editor(
                "command-snapshot",
                &self.input,
                &self.draft,
                false,
                Rc::new(|_| {}),
                &(Rc::new(|_, _: &mut Window, _: &mut gpui::App| {}) as super::PickFn),
                crate::ui::theme::palette(cx),
            ))
        }
    }
    fn build(window: &mut Window, cx: &mut gpui::App, cmd: bool) -> gpui::AnyView {
        cx.new(|cx| {
            let text = if cmd {
                r#"start "" notepad.exe"#
            } else {
                "Start-Process notepad.exe"
            };
            let input = cx.new(|cx| {
                let mut input = InputState::new(window, cx);
                input.set_value(text, window, cx);
                input
            });
            let action = if cmd {
                Action::RunShellCommand(text.into())
            } else {
                Action::RunPowerShellCommand(text.into())
            };
            let draft = cx.new(|_| CommandDraft::new(input.clone(), Some(&action)));
            CommandScene { input, draft }
        })
        .into()
    }
    vec![
        crate::ui::snapshots::Scene {
            name: "commands/powershell",
            size: size(px(440.), px(540.)),
            build: |w, cx| build(w, cx, false),
        },
        crate::ui::snapshots::Scene {
            name: "commands/powershell-narrow",
            size: size(px(300.), px(640.)),
            build: |w, cx| build(w, cx, false),
        },
        crate::ui::snapshots::Scene {
            name: "commands/cmd-narrow",
            size: size(px(300.), px(640.)),
            build: |w, cx| build(w, cx, true),
        },
    ]
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    use crate::features::binding_editor::{PickFn, command_editor};
    use gpui::{AppContext as _, Modifiers, px};
    use std::{cell::RefCell, rc::Rc};

    struct ShellEditor {
        input: Entity<InputState>,
        draft: Entity<CommandDraft>,
        pick: PickFn,
    }
    impl Render for ShellEditor {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            command_editor(
                "shell-test",
                &self.input,
                &self.draft,
                false,
                Rc::new(|_| {}),
                &self.pick,
                crate::ui::theme::palette(cx),
            )
        }
    }

    #[gpui::test]
    fn shell_examples_edit_the_draft_and_save_the_chosen_shell(cx: &mut gpui::TestAppContext) {
        let _locale = crate::services::i18n::LOCALE_LOCK.lock().unwrap();
        rust_i18n::set_locale("en");
        cx.update(gpui_component::init);
        let picked = Rc::new(RefCell::new(None));
        let result = Rc::clone(&picked);
        let (view, visual) = cx.add_window_view(move |window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx));
            let draft = cx.new(|_| CommandDraft::new(input.clone(), None));
            ShellEditor {
                input,
                draft,
                pick: Rc::new(move |action, _, _| *result.borrow_mut() = Some(action)),
            }
        });
        visual.simulate_resize(gpui::size(px(320.), px(700.)));
        visual.update(|window, cx| window.draw(cx).clear(cx));
        view.read_with(visual, |view, cx| {
            assert_eq!(view.draft.read(cx).action(String::new()), None);
            assert_eq!(view.draft.read(cx).action("  \t".into()), None);
            assert_eq!(
                view.draft.read(cx).action("Get-Date".into()),
                Some(Action::RunPowerShellCommand("Get-Date".into()))
            );
        });
        for (selector, expected) in [
            (
                "shell-mode-powershell",
                Action::RunPowerShellCommand("Start-Process notepad.exe".into()),
            ),
            (
                "shell-mode-cmd",
                Action::RunShellCommand(r#"start "" notepad.exe"#.into()),
            ),
        ] {
            let before = view.read_with(visual, |view, cx| view.input.read(cx).value());
            let mode = visual.debug_bounds(selector).expect("shell toggle");
            visual.simulate_click(mode.center(), Modifiers::default());
            visual.update(|window, cx| window.draw(cx).clear(cx));
            view.read_with(visual, |view, cx| {
                assert_eq!(
                    view.input.read(cx).value(),
                    before,
                    "switching shells keeps the draft"
                );
            });
            let example = visual.debug_bounds("shell-notepad-example").unwrap();
            visual.simulate_click(example.center(), Modifiers::default());
            assert!(
                picked.borrow().is_none(),
                "example must only edit the draft"
            );
            view.read_with(visual, |view, cx| {
                assert_eq!(
                    view.draft
                        .read(cx)
                        .action(view.input.read(cx).value().to_string()),
                    Some(expected.clone())
                );
            });
            let assign = visual
                .debug_bounds("shell-test-custom-command-add")
                .unwrap();
            visual.simulate_click(assign.center(), Modifiers::default());
            assert_eq!(picked.borrow_mut().take(), Some(expected));
        }
        visual.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.draft.update(cx, |draft, cx| draft.reset(window, cx));
            });
        });
        view.read_with(visual, |view, cx| {
            assert_eq!(
                view.draft.read(cx).placeholder(),
                CommandDraft::default_placeholder()
            );
        });
    }

    #[test]
    fn existing_cmd_and_powershell_actions_keep_their_shell() {
        assert_eq!(CommandShell::for_action(None), CommandShell::PowerShell);
        assert_eq!(
            CommandShell::for_action(Some(&Action::RunShellCommand("echo hi".into()))),
            CommandShell::System
        );
        assert_eq!(
            CommandShell::for_action(Some(&Action::RunPowerShellCommand("Get-Date".into()))),
            CommandShell::PowerShell
        );
    }
}
