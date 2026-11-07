//! The bootstrap preference is separate from device/profile settings.

use crate::ui::{
    components::{DetailedTooltip as _, control_button},
    theme,
};
use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div,
};
use gpui_component::{Disableable as _, WindowExt as _, dialog::DialogButtonProps};
use std::path::PathBuf;

mod pointer;

pub(super) struct ConfigLocationView {
    current: Result<PathBuf, String>,
    next: Result<PathBuf, String>,
    error: Option<String>,
    pending: bool,
    pointer: Entity<pointer::PointerView>,
}

impl ConfigLocationView {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            current: openlogi_core::paths::config_dir().map_err(|e| e.to_string()),
            next: openlogi_core::paths::next_config_dir().map_err(|e| e.to_string()),
            error: None,
            pending: false,
            pointer: cx.new(pointer::PointerView::new),
        }
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.pending = true;
        cx.notify();
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("config_location.choose")),
        });
        cx.spawn_in(window, async move |view, cx| {
            let selection = prompt.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.pending = false;
                let selected = match selection {
                    Ok(Ok(Some(paths))) => paths.into_iter().next(),
                    Ok(Ok(None)) => None,
                    _ => {
                        this.error = Some(tr!("config_location.picker_failed").to_string());
                        None
                    }
                };
                cx.notify();
                let Some(selected) = selected else {
                    return;
                };
                let view = cx.entity().downgrade();
                window.open_alert_dialog(cx, move |dialog, _, _| {
                    let selected = selected.clone();
                    let view = view.clone();
                    dialog
                        .title(tr!("config_location.confirm_title"))
                        .description(format!(
                            "{}\n\n{}",
                            tr!("config_location.warning"),
                            selected.display()
                        ))
                        .button_props(
                            DialogButtonProps::default()
                                .ok_text(tr!("config_location.use_after_restart"))
                                .cancel_text(tr!("common.cancel"))
                                .show_cancel(true),
                        )
                        .on_ok(move |_, _, cx| {
                            let selected = selected.clone();
                            let _ = view.update(cx, |this, cx| this.save(selected, cx));
                            true
                        })
                });
            });
        })
        .detach();
    }

    fn save(&mut self, selected: PathBuf, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.pending = true;
        self.error = None;
        cx.notify();
        let work = cx.background_spawn(async move {
            openlogi_core::paths::set_next_config_dir(&selected)
                .map(|()| selected)
                .map_err(|e| e.to_string())
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |this, cx| {
                this.pending = false;
                match result {
                    Ok(path) => this.next = Ok(path),
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for ConfigLocationView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self
            .current
            .as_ref()
            .map_or_else(Clone::clone, |p| p.display().to_string());
        let mut content = div()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .min_w_0()
            .child(
                div()
                    .text_sm()
                    .child(tr!("config_location.current", path => current)),
            )
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(theme::accent())
                    .text_sm()
                    .child(tr!("config_location.warning")),
            );
        if self.next != self.current {
            let next = self
                .next
                .as_ref()
                .map_or_else(Clone::clone, |p| p.display().to_string());
            content = content.child(
                div()
                    .text_sm()
                    .child(tr!("config_location.restart", path => next)),
            );
        }
        if let Some(error) = &self.error {
            content = content.child(div().text_sm().child(error.clone()));
        }
        content
            .child(
                control_button("choose-config-directory")
                    .label(tr!("config_location.choose"))
                    .outline()
                    .disabled(self.pending)
                    .detailed_tooltip(tr!("config_location.choose_tooltip"))
                    .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
            )
            .child(self.pointer.clone())
    }
}

#[cfg(all(debug_assertions, feature = "ui-snapshots"))]
pub(crate) fn snapshot_scenes() -> Vec<crate::ui::snapshots::Scene> {
    use crate::ui::snapshots::Scene;
    use gpui::{px, size};
    let mut scenes = vec![
        Scene {
            name: "settings/location",
            size: size(px(900.), px(2100.)),
            build: |window, cx| {
                cx.new(|cx| super::SettingsView::new(super::SettingsPage::General, window, cx))
                    .into()
            },
        },
        Scene {
            name: "settings/location-pending-narrow",
            size: size(px(520.), px(1250.)),
            build: |_, cx| {
                cx.new(|cx| ConfigLocationView {
                    current: Ok(PathBuf::from(r"C:\Users\Example\.config\openlogi")),
                    next: Ok(PathBuf::from(
                        r"D:\Portable Applications\Mouse & Key Studio\My profiles",
                    )),
                    error: None,
                    pending: false,
                    pointer: cx.new(|_| {
                        pointer::PointerView::example(
                            openlogi_core::paths::ConfigPointerLocation::PortableFolder,
                            None,
                        )
                    }),
                })
                .into()
            },
        },
        Scene {
            name: "settings/location-error",
            size: size(px(520.), px(1250.)),
            build: |_, cx| {
                cx.new(|cx| ConfigLocationView {
                    current: Ok(PathBuf::from(r"C:\Users\Example\.config\openlogi")),
                    next: Ok(PathBuf::from(r"C:\Users\Example\.config\openlogi")),
                    error: Some("Access denied. The previous folder is still selected.".into()),
                    pending: false,
                    pointer: cx.new(|_| {
                        pointer::PointerView::example(
                            openlogi_core::paths::ConfigPointerLocation::UserProfile,
                            Some("Access denied. The portable folder is read-only."),
                        )
                    }),
                })
                .into()
            },
        },
    ];
    scenes.extend(pointer::snapshot_scenes());
    scenes
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn cancelling_folder_picker_preserves_location(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (view, visual) = cx.add_window_view(|_, cx| ConfigLocationView::new(cx));
        let before = view.read_with(visual, |view, _| view.next.clone());
        visual.update(|window, cx| {
            view.update(cx, |view, cx| view.choose(window, cx));
        });
        assert!(visual.did_prompt_for_paths());
        visual.simulate_path_prompt_response(|options| {
            assert!(options.directories);
            assert!(!options.files);
            assert!(!options.multiple);
            None
        });
        visual.run_until_parked();
        view.read_with(visual, |view, _| {
            assert_eq!(view.next, before);
            assert!(!view.pending);
            assert!(view.error.is_none());
        });
    }
}
