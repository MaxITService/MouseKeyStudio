//! Native executable selection and file-manager actions for application profiles.

use crate::ui::components::DetailedTooltip as _;
use gpui::{ElementId, Styled as _};
use gpui_component::{
    IconName,
    button::{Button, ButtonVariants as _},
};

use crate::ui::components::control_button;

pub(super) fn directory_button(id: impl Into<ElementId>, identifier: &str) -> Option<Button> {
    // Alias names, bundle IDs and exe: selectors do not identify a directory.
    let path = std::path::Path::new(identifier);
    if !path.is_absolute() {
        return None;
    }
    let path = path.to_path_buf();
    let label = tr!("profiles.open_executable_directory", app => identifier);
    Some(
        control_button(id)
            .ghost()
            .flex_none()
            .icon(IconName::FolderOpen)
            .accessibility_label(label.clone())
            .detailed_tooltip(label)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                cx.reveal_path(&path);
            }),
    )
}

#[cfg(target_os = "windows")]
pub(super) fn choose(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    complete: impl FnOnce(Result<Option<String>, gpui::SharedString>, &mut gpui::Window, &mut gpui::App)
    + 'static,
) {
    let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(tr!("profiles.choose_executable")),
    });
    window
        .spawn(cx, async move |cx| {
            let result = match prompt.await {
                Ok(Ok(Some(paths))) => paths.first().map_or(Ok(None), |path| {
                    if path
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
                    {
                        Ok(Some(openlogi_core::app::windows_executable_id(path)))
                    } else {
                        Err(tr!("profiles.executable_required"))
                    }
                }),
                Ok(Ok(None)) => Ok(None),
                Ok(Err(_)) | Err(_) => Err(tr!("profiles.executable_picker_failed")),
            };
            let _ = cx.update(move |window, cx| complete(result, window, cx));
        })
        .detach();
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use gpui::{Context, IntoElement, Render, TestAppContext, Window, div};
    use std::{cell::RefCell, path::PathBuf, rc::Rc};

    struct PickerHost;
    impl Render for PickerHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
    fn executable_picker_handles_selection_cancellation_and_non_executables(
        cx: &mut TestAppContext,
    ) {
        let (_, visual) = cx.add_window_view(|_, _| PickerHost);
        for (selection, expected) in [
            (None, Ok(None)),
            (Some(r"C:\Tools\Readme.txt"), Err(())),
            (
                Some(r"C:\Tools\Editor.EXE"),
                Ok(Some(r"c:\tools\editor.exe")),
            ),
        ] {
            let result = Rc::new(RefCell::new(None));
            visual.update(|window, cx| {
                let result = result.clone();
                super::choose(window, cx, move |chosen, _, _| {
                    *result.borrow_mut() = Some(chosen);
                });
            });
            assert!(visual.did_prompt_for_paths());
            visual.simulate_path_prompt_response(|options| {
                assert!(options.files);
                assert!(!options.directories);
                assert!(!options.multiple);
                selection.map(|path| vec![PathBuf::from(path)])
            });
            visual.run_until_parked();
            let chosen = result
                .borrow_mut()
                .take()
                .expect("picker callback completes");
            assert_eq!(
                chosen.map_err(|_| ()),
                expected.map(|path| path.map(str::to_string))
            );
        }
    }
}
