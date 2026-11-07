//! UI-owned authorization transaction: save only after Windows accepts the task.

use super::{AppState, StateEvent};
use crate::platform::admin_autostart::{self, Outcome, Preferences};
use gpui::AppContext as _;

#[derive(Default, PartialEq, Eq)]
pub(crate) enum SetupStatus {
    #[default]
    Idle,
    Pending,
    Failed,
}

impl AppState {
    /// State of the explicit Windows autostart setup, shared by both switches.
    pub(crate) fn autostart_setup_status(&self) -> &SetupStatus {
        &self.autostart_setup
    }

    /// Change the administrator preference only after the user's UAC decision.
    pub(crate) fn request_autostart_as_admin(enabled: bool, cx: &mut gpui::App) {
        let at_login = Self::try_read(cx).is_some_and(|state| state.app_settings().launch_at_login);
        Self::configure_autostart(
            Preferences {
                administrator: enabled,
                at_login,
            },
            cx,
        );
    }

    /// Keep the scheduled task and normal login switch in the same transaction.
    pub(crate) fn request_launch_at_login(enabled: bool, cx: &mut gpui::App) {
        if Self::try_read(cx).is_some_and(|state| state.app_settings().autostart_as_admin) {
            Self::configure_autostart(
                Preferences {
                    administrator: true,
                    at_login: enabled,
                },
                cx,
            );
        } else {
            Self::apply(cx, |state| state.commit_launch_at_login(enabled));
        }
    }

    fn configure_autostart(preferences: Preferences, cx: &mut gpui::App) {
        let Some(exe) = crate::services::ipc::agent_binary_path() else {
            Self::update(cx, |state, cx| {
                state.autostart_setup = SetupStatus::Failed;
                cx.emit(StateEvent::SettingsChanged);
            });
            return;
        };
        Self::configure_autostart_with(preferences, exe, admin_autostart::configure, cx);
    }

    fn configure_autostart_with(
        preferences: Preferences,
        exe: std::path::PathBuf,
        configure: impl Fn(&std::path::Path, Preferences) -> Result<Outcome, String>
        + Clone
        + Send
        + 'static,
        cx: &mut gpui::App,
    ) {
        Self::update(cx, |state, cx| {
            if state.autostart_setup == SetupStatus::Pending || state.config_issue().is_some() {
                return;
            }
            state.autostart_setup = SetupStatus::Pending;
            cx.emit(StateEvent::SettingsChanged);
            let previous = Preferences {
                administrator: state.app_settings().autostart_as_admin,
                at_login: state.app_settings().launch_at_login,
            };
            let setup_exe = exe.clone();
            let apply = configure.clone();
            let setup = cx.background_spawn(async move { apply(&setup_exe, preferences) });
            cx.spawn(async move |state, cx| {
                let result = setup.await;
                let applied = matches!(&result, Ok(Outcome::Applied));
                let rollback = state.update(cx, |state, cx| {
                    state.autostart_setup = SetupStatus::Idle;
                    let rollback = match result {
                        Ok(Outcome::Applied) => {
                            !state.commit_autostart_preferences(preferences)
                        }
                        Ok(Outcome::Cancelled) => false,
                        Err(error) => {
                            tracing::warn!(%error, "administrator autostart setup failed");
                            state.autostart_setup = SetupStatus::Failed;
                            false
                        }
                    };
                    if rollback {
                        // Keep both switches disabled until compensation finishes.
                        state.autostart_setup = SetupStatus::Pending;
                    }
                    cx.emit(StateEvent::SettingsChanged);
                    rollback
                });
                if rollback.unwrap_or(applied) {
                    let restored = cx.background_spawn(async move {
                        configure(&exe, previous)
                    }).await;
                    let _ = state.update(cx, |state, cx| {
                        if !matches!(restored, Ok(Outcome::Applied)) {
                            tracing::error!("could not restore administrator autostart after a failed config save; Windows task may differ from preferences");
                        }
                        state.autostart_setup = SetupStatus::Failed;
                        cx.emit(StateEvent::SettingsChanged);
                    });
                }
            })
            .detach();
        });
    }

    /// A failed save restores the config; the caller must restore the OS task
    /// before ending the authorization transaction as well.
    fn commit_autostart_preferences(&mut self, preferences: Preferences) -> bool {
        self.config.edit(|config| {
            config.app_settings.autostart_as_admin = preferences.administrator;
            config.app_settings.launch_at_login = preferences.at_login;
        });
        self.persist_and_reload("Windows autostart setup")
    }
}

#[cfg(test)]
mod tests;
