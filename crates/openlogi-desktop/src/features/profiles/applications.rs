//! The applications assigned to the profile an editor is editing, with a
//! native executable picker that adds another one to it.

use crate::ui::components::DetailedTooltip as _;
use gpui::{
    App, AppContext as _, ClipboardItem, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div,
    prelude::FluentBuilder as _,
};
#[cfg(target_os = "windows")]
use gpui_component::Disableable as _;
use gpui_component::{
    ActiveTheme as _, IconName, WindowExt as _, button::ButtonVariants as _, h_flex, v_flex,
};
use openlogi_core::app::AppAliases;

use super::executable;
use crate::state::AppState;
use crate::ui::components::control_button;
use crate::ui::theme::{self, Typography as _};

/// Open the application list of the profile keyed `profile`: the one selected
/// in the editor whose profile bar hosts the trigger.
pub(super) fn open(profile: String, window: &mut Window, cx: &mut App) {
    let apps = cx.new(|cx| ProfileApps {
        profile,
        feedback: None,
        #[cfg(target_os = "windows")]
        selecting: false,
        _subscription: AppState::repaint_on(cx, |_| true),
    });
    window.open_dialog(cx, move |dialog, window, cx| {
        let viewport = window.viewport_size();
        // The dialog body is the list's only scroll owner. Capping the
        // surface keeps its title and footer in the window while the body
        // scrolls between them; uncapped, the surface grows past the window
        // edge and the body never overflows.
        let margin = viewport.height / 10.;
        let this = apps.read(cx);
        dialog
            .w(theme::ContentWidth::Medium
                .rems()
                .to_pixels(window.rem_size())
                .min(viewport.width - gpui::px(32.)))
            .margin_top(margin)
            .max_h(viewport.height - margin * 2.)
            .title(this.title(cx))
            .child(apps.clone())
            .footer(footer(&apps, cx))
    });
}

/// The outcome of the last command, shown beside the commands in the footer
/// because the row it concerns may be scrolled out of view.
enum Feedback {
    Copied,
    #[cfg(target_os = "windows")]
    Added(String),
    #[cfg(target_os = "windows")]
    AlreadyAssigned(String),
    #[cfg(target_os = "windows")]
    Failed(SharedString),
}

impl Feedback {
    fn message(&self) -> SharedString {
        match self {
            Self::Copied => tr!("profiles.path_copied"),
            #[cfg(target_os = "windows")]
            Self::Added(app) => tr!("profiles.application_added", app => app),
            #[cfg(target_os = "windows")]
            Self::AlreadyAssigned(app) => {
                tr!("profiles.application_already_assigned", app => app)
            }
            #[cfg(target_os = "windows")]
            Self::Failed(error) => error.clone(),
        }
    }

    fn is_error(&self) -> bool {
        #[cfg(target_os = "windows")]
        if matches!(self, Self::Failed(_)) {
            return true;
        }
        false
    }
}

struct ProfileApps {
    /// The profile key. Adding the first extra application turns a direct key
    /// into an alias key, so it follows the assignment's result.
    profile: String,
    feedback: Option<Feedback>,
    #[cfg(target_os = "windows")]
    selecting: bool,
    _subscription: Subscription,
}

impl ProfileApps {
    fn title(&self, cx: &App) -> SharedString {
        let name = AppState::try_read(cx).map_or_else(
            || openlogi_core::app::readable_app_name(&self.profile),
            |state| super::profile_name(state, &self.profile),
        );
        tr!("profiles.apps_title", profile => name)
    }

    #[cfg(target_os = "windows")]
    fn can_add(&self, cx: &App) -> bool {
        AppState::try_read(cx)
            .is_some_and(|state| state.can_assign_profile_applications(&self.profile))
    }

    #[cfg(target_os = "windows")]
    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            return;
        }
        self.selecting = true;
        self.feedback = None;
        cx.notify();
        let view = cx.entity().downgrade();
        executable::choose(window, cx, move |result, _, cx| {
            let _ = view.update(cx, |this, cx| {
                this.selecting = false;
                match result {
                    Ok(Some(identifier)) => this.assign(&identifier, cx),
                    // Cancelling the picker leaves the profile untouched.
                    Ok(None) => {}
                    Err(error) => this.feedback = Some(Feedback::Failed(error)),
                }
                cx.notify();
            });
        });
    }

    #[cfg(target_os = "windows")]
    fn assign(&mut self, identifier: &str, cx: &mut Context<Self>) {
        let Some(state) = AppState::try_read(cx) else {
            return;
        };
        let app = super::profile_name(state, identifier);
        if assigned_identifiers(state.app_aliases(), &self.profile)
            .iter()
            .any(|listed| listed == identifier)
        {
            self.feedback = Some(Feedback::AlreadyAssigned(app));
            return;
        }
        let name = super::profile_name(state, &self.profile);
        let profile = self.profile.clone();
        let result = AppState::update(cx, |state, cx| {
            let (result, events) = state.assign_profile_application(&profile, &name, identifier);
            events.emit(cx);
            result
        });
        self.feedback = Some(match result {
            Ok(key) => {
                self.profile = key;
                Feedback::Added(app)
            }
            Err(reason) => Feedback::Failed(tr!("profiles.assignment_failed", reason => reason)),
        });
    }

    fn identifier_row(
        state: &AppState,
        identifier: &str,
        first: bool,
        pal: theme::Palette,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let directory =
            executable::directory_button(format!("apps:folder:{identifier}"), identifier);
        let located = directory.is_some();
        let path_selector = format!("assigned-app-path:{identifier}");
        let copy_selector = format!("assigned-app-copy:{identifier}");
        let copy_label = tr!("profiles.copy_executable");
        let copied = identifier.to_string();
        h_flex()
            .w_full()
            .min_w_0()
            .items_start()
            .gap_2()
            .px_3()
            .py_2()
            .when(!first, |row| row.border_t_1().border_color(pal.border))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .whitespace_normal()
                            .text_body()
                            .child(super::profile_name(state, identifier)),
                    )
                    .child(
                        // The full identifier wraps rather than truncating:
                        // it is the information this list exists to show.
                        div()
                            .debug_selector(move || path_selector)
                            .w_full()
                            .min_w_0()
                            .whitespace_normal()
                            .text_caption()
                            .text_color(pal.text_muted)
                            .child(identifier.to_string()),
                    )
                    .when(!located, |column| {
                        column.child(
                            div()
                                .whitespace_normal()
                                .text_caption()
                                .text_color(pal.text_muted)
                                .child(tr!("profiles.executable_location_unknown")),
                        )
                    }),
            )
            .child(
                h_flex().flex_none().gap_1().children(directory).child(
                    control_button(format!("apps:copy:{identifier}"))
                        .debug_selector(move || copy_selector)
                        .ghost()
                        .icon(IconName::Copy)
                        .accessibility_label(copy_label.clone())
                        .detailed_tooltip(tr!("profiles.copy_identifier_tooltip"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()));
                            this.feedback = Some(Feedback::Copied);
                            cx.notify();
                        })),
                ),
            )
    }
}

/// The platform identifiers that select the profile keyed `profile`: its
/// alias's list for this platform, or the direct key itself.
fn assigned_identifiers(aliases: &AppAliases, profile: &str) -> Vec<String> {
    aliases.for_profile_key(profile).map_or_else(
        || vec![profile.to_string()],
        |(_, alias)| alias.identifiers_here().to_vec(),
    )
}

impl Render for ProfileApps {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = theme::palette(cx);
        let content = v_flex().w_full().min_w_0().gap_3().child(
            div()
                .whitespace_normal()
                .text_body()
                .text_color(pal.text_muted)
                .child(tr!("profiles.apps_description")),
        );
        let Some(state) = AppState::try_read(cx) else {
            return content;
        };
        let identifiers = assigned_identifiers(state.app_aliases(), &self.profile);
        #[cfg(target_os = "windows")]
        let content = content.when(!self.can_add(cx), |content| {
            content.child(
                div()
                    .whitespace_normal()
                    .text_caption()
                    .text_color(pal.text_muted)
                    .child(tr!("profiles.unsaved_profile_apps")),
            )
        });
        if identifiers.is_empty() {
            return content.child(
                div()
                    .py_4()
                    .whitespace_normal()
                    .text_body()
                    .text_color(pal.text_muted)
                    .child(tr!("profiles.no_platform_applications")),
            );
        }
        let rows = identifiers
            .iter()
            .enumerate()
            .map(|(index, identifier)| Self::identifier_row(state, identifier, index == 0, pal, cx))
            .collect::<Vec<_>>();
        content.child(
            v_flex()
                .debug_selector(|| "assigned-apps-list".into())
                .w_full()
                .min_w_0()
                .border_1()
                .border_color(pal.border)
                .rounded(pal.control_radius)
                .children(rows),
        )
    }
}

/// Commands that apply to the whole profile stay in the footer, outside the
/// scrolling body, with the last command's outcome beside them.
fn footer(apps: &Entity<ProfileApps>, cx: &App) -> impl IntoElement {
    let this = apps.read(cx);
    let pal = theme::palette(cx);
    let status = this.feedback.as_ref().map(|feedback| {
        div()
            .debug_selector(|| "assigned-apps-status".into())
            .flex_1()
            .min_w_0()
            .whitespace_normal()
            .text_caption()
            .text_color(if feedback.is_error() {
                cx.theme().danger
            } else {
                pal.text_muted
            })
            .child(feedback.message())
    });
    let row = h_flex().w_full().min_w_0().items_center().gap_2();
    #[cfg(target_os = "windows")]
    let row = {
        let apps = apps.clone();
        row.child(
            control_button("apps:add-executable")
                .debug_selector(|| "assigned-apps-add".into())
                .outline()
                .icon(IconName::Plus)
                .label(tr!("profiles.add_executable"))
                .detailed_tooltip(tr!("profiles.add_executable_tooltip"))
                .loading(this.selecting)
                .disabled(this.selecting || !this.can_add(cx))
                .on_click(move |_, window, cx| {
                    apps.update(cx, |this, cx| this.choose(window, cx));
                }),
        )
    };
    row.child(div().flex_1().min_w_0().children(status)).child(
        control_button("apps:done")
            .debug_selector(|| "assigned-apps-done".into())
            .label(tr!("common.done"))
            .detailed_tooltip(tr!("profiles.apps_done_tooltip"))
            .on_click(|_, window, cx| window.close_dialog(cx)),
    )
}

#[cfg(test)]
mod tests;
