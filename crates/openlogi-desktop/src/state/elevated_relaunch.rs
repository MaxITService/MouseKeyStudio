//! "Restart as administrator now": relaunch the GUI through UAC, then hand the
//! agent over so the elevated GUI starts an elevated one.

use super::{AppState, StateEvent, autostart::SetupStatus};
use crate::platform::elevation::{self, RelaunchError};
use gpui::AppContext as _;

impl AppState {
    /// State of the elevated relaunch the Settings button drives.
    pub(crate) fn elevated_relaunch_status(&self) -> &SetupStatus {
        &self.elevated_relaunch
    }

    /// Whether a relaunch with administrator rights can be offered: this
    /// process is not elevated yet and no relaunch is already under way.
    pub(crate) fn can_relaunch_elevated(&self) -> bool {
        !elevation::is_elevated() && self.elevated_relaunch != SetupStatus::Pending
    }

    /// Ask UAC for an elevated copy of this app. Declining leaves everything as
    /// it was; once Windows starts the copy, this process stops the agent for
    /// it and quits so the copy can take over.
    pub(crate) fn request_elevated_relaunch(cx: &mut gpui::App) {
        Self::update(cx, |state, cx| {
            if !state.can_relaunch_elevated() {
                return;
            }
            state.elevated_relaunch = SetupStatus::Pending;
            cx.emit(StateEvent::SettingsChanged);
            let relaunch = cx.background_spawn(async { elevation::relaunch_elevated() });
            cx.spawn(async move |state, cx| {
                // The status the button returns to when no elevated copy started.
                let unstarted = match relaunch.await {
                    Ok(()) => None,
                    Err(RelaunchError::Cancelled) => {
                        tracing::info!("elevated relaunch declined at the UAC prompt");
                        Some(SetupStatus::Idle)
                    }
                    Err(RelaunchError::Failed(error)) => {
                        tracing::warn!(%error, "could not relaunch with administrator rights");
                        Some(SetupStatus::Failed)
                    }
                };
                if let Some(status) = unstarted {
                    let _ = state.update(cx, |state, cx| {
                        state.elevated_relaunch = status;
                        cx.emit(StateEvent::SettingsChanged);
                    });
                    return;
                }
                tracing::info!("elevated copy started — handing the agent over");
                cx.background_spawn(async { crate::services::ipc::hand_off_agent() })
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    }
}
