//! Searchable profile selector and feature-independent controls.

use crate::ui::components::DetailedTooltip as _;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, img, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    Disableable as _, IconName, Sizable as _, WindowExt as _,
    button::ButtonVariants as _,
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    spinner::Spinner,
};

use super::catalog::{AppCatalogPicker, ApplicationIconState, ProfileIconCache};
use super::picker::add_app_popover;
use super::{ProfileChoice, ProfileScopeActions, ProfileScopeModel};
use crate::ui::components::control_button;
use crate::ui::theme::{self, Palette, Typography as _};

/// Feature-independent profile switcher. Feature adapters provide the
/// profiles and own selection/removal behavior through [`ProfileScopeActions`].
#[derive(IntoElement)]
pub(crate) struct ProfileScopeShell {
    id_base: &'static str,
    model: ProfileScopeModel,
    catalog: Entity<AppCatalogPicker>,
    icons: ProfileIconCache,
    actions: ProfileScopeActions,
}

impl ProfileScopeShell {
    pub(super) fn new(
        id_base: &'static str,
        model: ProfileScopeModel,
        catalog: Entity<AppCatalogPicker>,
        icons: ProfileIconCache,
        actions: ProfileScopeActions,
    ) -> Self {
        Self {
            id_base,
            model,
            catalog,
            icons,
            actions,
        }
    }
}

impl RenderOnce for ProfileScopeShell {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let pal = theme::palette(cx);
        profile_scope_content(self, pal, window, cx)
    }
}

fn profile_scope_content(
    shell: ProfileScopeShell,
    pal: Palette,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let selected_profile = shell
        .model
        .editing_app
        .as_deref()
        .and_then(|app| {
            shell
                .model
                .profiles
                .iter()
                .find(|profile| profile.app == app)
        })
        .cloned();
    let selector =
        window.use_keyed_state(format!("{}:selector", shell.id_base), cx, |window, cx| {
            super::selector::ProfileSelector::new(shell.actions.clone(), window, cx)
        });
    selector.update(cx, |selector, cx| {
        selector.show(&shell.model, shell.actions.clone(), window, cx);
    });
    let select = selector.read(cx).select.clone();

    h_flex()
        .debug_selector(|| "profile-scope-bar".into())
        .flex_shrink_0()
        .w_full()
        .min_w_0()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(pal.border)
        .bg(pal.panel)
        .px_4()
        .py_2()
        .child(
            div()
                .flex_none()
                .text_body()
                .text_color(pal.text_muted)
                .child(tr!("profiles.profile")),
        )
        .child(
            div()
                .id(format!("{}:profile-help", shell.id_base))
                .debug_selector(|| "profile-selector".into())
                .flex_1()
                .min_w_0()
                .max_w(px(560.))
                .tooltip(|window, cx| crate::ui::components::detailed_tooltip(tr!("profiles.select_profile_tooltip"), window, cx))
                .child(
                    crate::ui::components::control_select(&select)
                        .id(format!("{}:profile-select", shell.id_base))
                        .accessibility_label(tr!("profiles.profile"))
                        .search_placeholder(tr!("profiles.search_applications"))
                        .menu_max_h(px(360.))
                        .w_full(),
                ),
        )
        .child(add_app_popover(
            shell.id_base,
            shell.model.choices,
            shell.catalog,
            shell.icons,
            shell.actions.clone(),
            pal,
        ))
        .child(duplicate_button(shell.id_base, selected_profile.clone(), shell.actions.clone()))
        // The Default profile has no applications, so both per-profile
        // controls appear only once an application profile is selected.
        .when_some(selected_profile, |row, profile| {
            row.child(applications_button(shell.id_base, profile.app.clone()))
                .child(profile_options_menu(shell.id_base, profile, shell.actions))
        })
}

fn duplicate_button(
    id_base: &'static str,
    profile: Option<ProfileChoice>,
    actions: ProfileScopeActions,
) -> impl IntoElement {
    control_button(format!("{id_base}:duplicate"))
        .debug_selector(move || format!("{id_base}:duplicate"))
        .outline()
        .disabled(profile.as_ref().is_some_and(|profile| !profile.persisted))
        .icon(IconName::Copy)
        .detailed_tooltip(tr!("profiles.duplicate_profile_tooltip"))
        .accessibility_label(tr!("profiles.duplicate_profile"))
        .on_click(move |_, window, cx| {
            let source = profile.as_ref().map(|profile| profile.app.as_str());
            let name = profile.as_ref().map_or_else(
                || tr!("common.default").to_string(),
                |profile| profile.name.clone(),
            );
            let name = tr!("profiles.profile_copy_name", name => name).to_string();
            let result = crate::state::AppState::update(cx, |state, cx| {
                let (result, events) = state.duplicate_profile(source, &name);
                events.emit(cx);
                result
            });
            match result {
                Ok(key) => actions.select(Some(key), cx),
                Err(error) => window.open_alert_dialog(cx, move |dialog, _, _| {
                    dialog
                        .title(tr!("profiles.duplicate_failed"))
                        .description(error.clone())
                }),
            }
        })
}

pub(super) fn application_mark(
    icon: ApplicationIconState,
    name: &str,
    edge: f32,
    pal: Palette,
) -> gpui::Div {
    let slot = h_flex()
        .size(px(edge))
        .flex_none()
        .items_center()
        .justify_center();
    match icon {
        ApplicationIconState::Ready(icon) => slot.child(img(icon).size(px(edge)).flex_none()),
        ApplicationIconState::Loading => slot.child(
            Spinner::new()
                .with_size(px(edge * 0.6))
                .color(pal.text_muted),
        ),
        ApplicationIconState::Missing => {
            let initial = name
                .chars()
                .find(|character| !character.is_whitespace())
                .map_or_else(|| "?".to_string(), |character| character.to_string());
            slot.rounded(px(edge / 4.))
                .bg(pal.muted)
                .map(|tile| {
                    if edge < 24. {
                        tile.text_caption()
                    } else {
                        tile.text_body()
                    }
                })
                .text_color(pal.text_muted)
                .child(initial)
        }
    }
}

/// Opens the application list of the selected profile, keyed `profile`.
fn applications_button(id_base: &'static str, profile: String) -> impl IntoElement {
    control_button(format!("{id_base}:apps"))
        .debug_selector(move || format!("{id_base}:apps"))
        .outline()
        .label(tr!("profiles.apps"))
        .detailed_tooltip(tr!("profiles.apps_tooltip"))
        .on_click(move |_, window, cx| {
            super::applications::open(profile.clone(), window, cx);
        })
}

fn profile_options_menu(
    id_base: &'static str,
    profile: ProfileChoice,
    actions: ProfileScopeActions,
) -> impl IntoElement {
    control_button(format!("{id_base}:profile-options"))
        .ghost()
        .label(tr!("profiles.profile_options"))
        .detailed_tooltip(tr!("profiles.profile_options_tooltip"))
        .icon(IconName::ChevronDown)
        .dropdown_menu_with_anchor(gpui::Anchor::TopRight, move |menu, _, _| {
            let reset_profile = profile.clone();
            let reset_actions = actions.clone();
            let all_profile = profile.clone();
            let all_actions = actions.clone();
            let profile = profile.clone();
            let actions = actions.clone();
            menu.label(profile.name.clone())
                .item(
                    PopupMenuItem::new(tr!("profiles.reset_profile_dialog"))
                        .disabled(!profile.persisted)
                        .on_click(move |_, window, cx| {
                            let profile = reset_profile.clone();
                            let actions = reset_actions.clone();
                            // Open after the menu restores focus to its trigger.
                            window.defer(cx, move |window, cx| actions.reset(profile, window, cx));
                        }),
                )
                .separator()
                .item(
                    PopupMenuItem::new(if profile.persisted {
                        tr!("profiles.remove_profile_dialog")
                    } else {
                        tr!("profiles.remove_profile")
                    })
                    .on_click(move |_event, window, cx| {
                        let profile = profile.clone();
                        let actions = actions.clone();
                        window.defer(cx, move |window, cx| actions.remove(profile, window, cx));
                    }),
                )
                .separator()
                .item(
                    PopupMenuItem::new(tr!("profiles.remove_all_profiles_dialog")).on_click(
                        move |_, window, cx| {
                            let profile = all_profile.clone();
                            let actions = all_actions.clone();
                            window.defer(cx, move |window, cx| {
                                actions.remove_all(profile, window, cx);
                            });
                        },
                    ),
                )
        })
}
