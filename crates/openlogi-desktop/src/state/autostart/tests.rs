//! Exercise the real async completion path with an inert Windows-task backend.
use std::sync::{Arc, Mutex};

use openlogi_core::config::{Config, ConfigFile};

use super::*;
use crate::services::assets::AssetResolver;
use crate::state::{ConfigPersistence, Sources};

#[gpui::test]
fn cancelled_or_failed_authorization_never_saves_or_requests_compensation(
    cx: &mut gpui::TestAppContext,
) {
    for cancelled in [true, false] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&calls);
        let (commands, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        cx.update(|cx| {
            let resolver = AssetResolver::new();
            let state = cx.new(|_| {
                AppState::new(Sources::in_memory(Config::ephemeral(), &resolver, commands))
            });
            AppState::set_global(state, cx);
            AppState::configure_autostart_with(
                Preferences {
                    administrator: true,
                    at_login: true,
                },
                "fake-agent.exe".into(),
                move |_, preferences| {
                    recorded.lock().unwrap().push(preferences);
                    if cancelled {
                        Ok(Outcome::Cancelled)
                    } else {
                        Err("simulated task failure".into())
                    }
                },
                cx,
            );
        });
        while receiver.try_recv().is_ok() {}
        cx.run_until_parked();
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "no second authorization after cancellation or failure"
        );
        cx.read(|cx| {
            let state = AppState::try_read(cx).unwrap();
            assert!(!state.app_settings().autostart_as_admin);
            assert_eq!(state.autostart_setup == SetupStatus::Idle, cancelled);
            assert!(state.config_issue().is_none());
        });
        assert!(
            receiver.try_recv().is_err(),
            "no config reload after cancellation or failure"
        );
    }
}

#[gpui::test]
fn a_save_conflict_restores_the_previous_windows_autostart_preferences(
    cx: &mut gpui::TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(openlogi_core::paths::CONFIG_FILE);
    let (mut config, mut file) = ConfigFile::load_from_path(&path).unwrap();
    config.app_settings.autostart_as_admin = true;
    config.app_settings.launch_at_login = true;
    file.save(&config).unwrap();
    let previous = Preferences {
        administrator: true,
        at_login: true,
    };
    let requested = Preferences {
        administrator: false,
        at_login: true,
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&calls);
    let (commands, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    cx.update(|cx| {
        let resolver = AssetResolver::new();
        let state = cx.new(|_| {
            AppState::new(Sources {
                persistence: ConfigPersistence::UserFile(file),
                ..Sources::in_memory(config, &resolver, commands)
            })
        });
        AppState::set_global(state, cx);
        AppState::configure_autostart_with(
            requested,
            dir.path().join("fake-agent.exe"),
            move |_, preferences| {
                let mut calls = recorded.lock().unwrap();
                calls.push(preferences);
                if calls.len() == 1 {
                    // Another writer wins while the first authorization is open.
                    let (mut current, mut file) = ConfigFile::load_from_path(&path).unwrap();
                    current.app_settings.smooth_scroll = true;
                    file.save(&current).unwrap();
                }
                Ok(Outcome::Applied)
            },
            cx,
        );
    });
    while receiver.try_recv().is_ok() {}
    cx.run_until_parked();
    assert_eq!(*calls.lock().unwrap(), [requested, previous]);
    cx.read(|cx| {
        let state = AppState::try_read(cx).unwrap();
        assert!(state.app_settings().autostart_as_admin);
        assert!(state.autostart_setup == SetupStatus::Failed);
        assert!(state.config_issue().is_some());
    });
    assert!(
        receiver.try_recv().is_err(),
        "failed save never reloads the agent"
    );
}

#[test]
fn successful_autostart_preferences_are_persisted_before_reloading() {
    let resolver = AssetResolver::new();
    let (commands, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut state = AppState::new(Sources::in_memory(Config::ephemeral(), &resolver, commands));
    while receiver.try_recv().is_ok() {}
    assert!(state.commit_autostart_preferences(Preferences {
        administrator: true,
        at_login: false
    }));
    assert!(state.app_settings().autostart_as_admin);
    assert!(!state.app_settings().launch_at_login);
    assert!(matches!(
        receiver.try_recv(),
        Ok(crate::services::ipc::Command::ReloadConfig(_))
    ));
}
