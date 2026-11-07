//! The settings directory and its small pointer file are separate user choices.

use crate::ui::{
    components::{DetailedTooltip as _, control_button},
    theme,
};
use gpui::{
    AppContext as _, ClipboardItem, Context, InteractiveElement as _, IntoElement, ParentElement,
    Render, Styled, Window, div,
};
use gpui_component::Disableable as _;
use openlogi_core::paths::{ConfigPointerChoice, ConfigPointerLocation};

#[derive(Clone)]
struct Destination {
    location: ConfigPointerLocation,
    path: String,
    selected: bool,
    write_error: Option<String>,
}

impl From<ConfigPointerChoice> for Destination {
    fn from(choice: ConfigPointerChoice) -> Self {
        Self {
            location: choice.location(),
            path: choice.path().display().to_string(),
            selected: choice.is_selected(),
            write_error: choice.write_error().map(str::to_owned),
        }
    }
}

pub(super) struct PointerView {
    destinations: Option<Vec<Destination>>,
    busy: bool,
    error: Option<String>,
}

impl PointerView {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            destinations: None,
            busy: false,
            error: None,
        };
        view.refresh(cx);
        view
    }

    #[cfg(any(test, all(debug_assertions, feature = "ui-snapshots")))]
    pub(super) fn example(selected: ConfigPointerLocation, portable_error: Option<&str>) -> Self {
        Self {
            destinations: Some(vec![
                Destination {
                    location: ConfigPointerLocation::UserProfile,
                    path: format!(
                        r"C:\Users\Example\.config\mousekey-studio\portable-locations\{}.toml",
                        "12ab34cd56ef7890".repeat(4)
                    ),
                    selected: selected == ConfigPointerLocation::UserProfile,
                    write_error: None,
                },
                Destination {
                    location: ConfigPointerLocation::PortableFolder,
                    path:
                        r"D:\Portable Applications\Mouse & Key Studio\mousekey-studio-portable.toml"
                            .into(),
                    selected: selected == ConfigPointerLocation::PortableFolder,
                    write_error: portable_error.map(str::to_owned),
                },
            ]),
            busy: false,
            error: None,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        let work = cx.background_spawn(async {
            openlogi_core::paths::config_pointer_choices().map_err(|error| error.to_string())
        });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |this, cx| {
                this.busy = false;
                this.adopt(result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn adopt(&mut self, result: Result<Vec<ConfigPointerChoice>, String>) {
        match result {
            Ok(choices) => {
                self.destinations = Some(choices.into_iter().map(Destination::from).collect());
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn relocate(&mut self, location: ConfigPointerLocation, cx: &mut Context<Self>) {
        if self.busy
            || !self.destinations.as_ref().is_some_and(|destinations| {
                destinations.iter().any(|entry| {
                    entry.location == location && !entry.selected && entry.write_error.is_none()
                })
            })
        {
            return;
        }
        self.busy = true;
        self.error = None;
        let work = cx.background_spawn(async move {
            let moved = openlogi_core::paths::set_config_pointer_location(location)
                .map_err(|error| error.to_string());
            let refreshed =
                openlogi_core::paths::config_pointer_choices().map_err(|error| error.to_string());
            (moved, refreshed)
        });
        cx.spawn(async move |view, cx| {
            let (moved, refreshed) = work.await;
            let _ = view.update(cx, |this, cx| {
                this.busy = false;
                this.adopt(refreshed);
                if let Err(error) = moved {
                    this.error = Some(error);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn destination(&self, destination: &Destination, cx: &mut Context<Self>) -> impl IntoElement {
        let (id, copy_id, label) = match destination.location {
            ConfigPointerLocation::UserProfile => (
                "config-pointer-user",
                "copy-pointer-user",
                tr!("config_location.pointer_user"),
            ),
            ConfigPointerLocation::PortableFolder => (
                "config-pointer-portable",
                "copy-pointer-portable",
                tr!("config_location.pointer_portable"),
            ),
        };
        let location = destination.location;
        let path = destination.path.clone();
        let status = destination.write_error.as_ref().map_or_else(
            || tr!("config_location.pointer_writable").to_string(),
            |error| tr!("config_location.pointer_denied", error => error).to_string(),
        );
        let mut row = div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_sm().child(label))
            .child(
                div()
                    .text_sm()
                    .whitespace_normal()
                    .child(wrapping_path(&path)),
            )
            .child(
                div()
                    .text_xs()
                    .child(tr!("config_location.pointer_length", count => path.chars().count())),
            )
            .child(div().text_sm().child(status));
        if destination.selected {
            row = row.child(
                div()
                    .text_sm()
                    .text_color(theme::accent())
                    .child(tr!("config_location.pointer_selected")),
            );
        }
        row.child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    control_button(id)
                        .debug_selector(move || id.into())
                        .label(tr!("config_location.pointer_use"))
                        .outline()
                        .disabled(
                            self.busy || destination.selected || destination.write_error.is_some(),
                        )
                        .detailed_tooltip(tr!("config_location.pointer_use_tooltip"))
                        .on_click(cx.listener(move |this, _, _, cx| this.relocate(location, cx))),
                )
                .child(
                    control_button(copy_id)
                        .debug_selector(move || copy_id.into())
                        .label(tr!("config_location.pointer_copy"))
                        .outline()
                        .detailed_tooltip(tr!("config_location.pointer_copy_tooltip"))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                        }),
                ),
        )
    }
}

fn wrapping_path(path: &str) -> String {
    // Keep long paths readable at narrow widths; Copy path retains the original.
    let mut wrapped = String::with_capacity(path.len());
    for character in path.chars() {
        wrapped.push(character);
        if matches!(character, '\\' | '/' | '-' | '_' | '.') {
            wrapped.push('\u{200b}');
        }
    }
    wrapped
}

#[cfg(all(debug_assertions, feature = "ui-snapshots"))]
pub(super) fn snapshot_scenes() -> Vec<crate::ui::snapshots::Scene> {
    use crate::ui::snapshots::Scene;
    use gpui::{px, size};
    vec![
        Scene {
            name: "settings/pointer-portable",
            size: size(px(560.), px(1200.)),
            build: |_, cx| {
                rust_i18n::set_locale("en");
                cx.new(|_| PointerView::example(ConfigPointerLocation::PortableFolder, None))
                    .into()
            },
        },
        Scene {
            name: "settings/pointer-denied-narrow",
            size: size(px(520.), px(1300.)),
            build: |_, cx| {
                rust_i18n::set_locale("en");
                cx.new(|_| {
                    PointerView::example(
                        ConfigPointerLocation::UserProfile,
                        Some("Access denied: the folder or pointer is read-only."),
                    )
                })
                .into()
            },
        },
        Scene {
            name: "settings/pointer-ru-narrow",
            size: size(px(520.), px(1450.)),
            build: |_, cx| {
                rust_i18n::set_locale("ru");
                cx.new(|_| {
                    PointerView::example(
                        ConfigPointerLocation::UserProfile,
                        Some("Отказано в доступе: папка или указатель доступны только для чтения."),
                    )
                })
                .into()
            },
        },
        Scene {
            name: "settings/pointer-loading",
            size: size(px(520.), px(650.)),
            build: |_, cx| {
                rust_i18n::set_locale("en");
                cx.new(|_| PointerView {
                    destinations: None,
                    busy: true,
                    error: None,
                })
                .into()
            },
        },
        Scene {
            name: "settings/pointer-move-failed",
            size: size(px(520.), px(1400.)),
            build: |_, cx| {
                rust_i18n::set_locale("en");
                cx.new(|_| {
                    let mut view = PointerView::example(ConfigPointerLocation::UserProfile, None);
                    view.error = Some(
                        "The source pointer is in use by another process. It is still selected."
                            .into(),
                    );
                    view
                })
                .into()
            },
        },
    ]
}

impl Render for PointerView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_4()
            .pt_4()
            .border_t_1()
            .border_color(theme::palette(cx).border)
            .child(div().text_sm().child(tr!("config_location.pointer_title")))
            .child(
                div().text_sm().child(
                    if self
                        .destinations
                        .as_ref()
                        .is_some_and(|destinations| destinations.len() == 1)
                    {
                        tr!("config_location.pointer_installed_help")
                    } else {
                        tr!("config_location.pointer_help")
                    },
                ),
            );
        if let Some(destinations) = &self.destinations {
            for destination in destinations {
                content = content.child(self.destination(destination, cx));
            }
        }
        if self.busy {
            content = content.child(
                div()
                    .text_sm()
                    .child(tr!("config_location.pointer_checking")),
            );
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .text_sm()
                    .child(tr!("config_location.pointer_failed", error => error)),
            );
        }
        content.child(
            control_button("check-pointer-access")
                .debug_selector(|| "check-pointer-access".into())
                .label(tr!("config_location.pointer_check"))
                .outline()
                .disabled(self.busy)
                .detailed_tooltip(tr!("config_location.pointer_check_tooltip"))
                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
        )
    }
}

#[cfg(test)]
mod tests;
