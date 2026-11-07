//! What the tray menu offers, decided from the config the agent last adopted
//! and its live DPI state. Pure, so the decisions are tested without a window.

use openlogi_agent_core::DpiCycles;
use openlogi_core::app::{ForegroundApp, profile_display_name};
use openlogi_core::config::Config;
use openlogi_core::hid::Dpi;

/// The saved DPI stages of one online device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DpiChoice {
    /// Config key the preset is dispatched to.
    pub(super) key: String,
    pub(super) device: String,
    pub(super) presets: Vec<Dpi>,
    /// The stage the device is on now.
    pub(super) current: usize,
}

/// Online devices whose DPI stages the user saved, by device name. A device
/// that only has its model's stock stages is left out: picking from them
/// would change a setting the user never chose.
pub(super) fn dpi_choices(config: &Config, cycles: &DpiCycles) -> Vec<DpiChoice> {
    let mut choices: Vec<DpiChoice> = cycles
        .by_key
        .iter()
        .filter(|(_, state)| !state.stock_presets && !state.presets.is_empty())
        .map(|(key, state)| DpiChoice {
            key: key.clone(),
            device: device_name(config, key),
            presets: state.presets.clone(),
            current: state.index,
        })
        .collect();
    choices.sort_by(|a, b| a.device.cmp(&b.device).then_with(|| a.key.cmp(&b.key)));
    choices
}

/// What the user calls the device keyed `key`: their own name for it, else
/// the model name it was last seen with.
fn device_name(config: &Config, key: &str) -> String {
    config
        .device_custom_name(key)
        .or_else(|| {
            config
                .devices
                .get(key)
                .and_then(|device| device.identity.as_ref())
                .map(|identity| identity.display_name.as_str())
        })
        .unwrap_or(key)
        .to_string()
}

/// The "Lock profile" item: whether it is checked, which profile it names,
/// and the per-device locks choosing it writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProfileLock {
    pub(super) locked: bool,
    /// The profile key the item names; empty for the default profile.
    pub(super) profile: String,
    /// `(device key, lock)` for every online device.
    pub(super) toggle: Vec<(String, Option<String>)>,
}

/// Lock every online device to the profile it runs now, or release them all
/// when any is locked. `None` without an online device to lock.
///
/// The profile is the one of the application last in front before the tray
/// was clicked. The click moves the foreground to the shell and then to the
/// agent's menu, but neither enters `recent`: the hook reports no application
/// for a shell surface, and the recent list leaves OpenLogi out. A File
/// Explorer folder window is an application and locks its own profile.
pub(super) fn profile_lock(
    config: &Config,
    cycles: &DpiCycles,
    recent: &[ForegroundApp],
) -> Option<ProfileLock> {
    let mut online: Vec<&str> = cycles.by_key.keys().map(String::as_str).collect();
    if online.is_empty() {
        return None;
    }
    online.sort_unstable();
    let lock_of = |key: &str| {
        config
            .devices
            .get(key)
            .and_then(|device| device.profile_lock.clone())
    };
    let locks: Vec<String> = online.iter().filter_map(|key| lock_of(key)).collect();
    // Name an application profile over Default when devices disagree.
    if let Some(profile) = locks
        .iter()
        .find(|profile| !profile.is_empty())
        .or_else(|| locks.first())
        .cloned()
    {
        return Some(ProfileLock {
            locked: true,
            profile,
            toggle: online
                .iter()
                .map(|key| ((*key).to_string(), None))
                .collect(),
        });
    }
    let app = recent.first();
    let target = |key: &str| {
        app.and_then(|app| config.app_profile_for(key, &app.id))
            .unwrap_or_default()
            .to_string()
    };
    let toggle: Vec<(String, Option<String>)> = online
        .iter()
        .map(|key| ((*key).to_string(), Some(target(key))))
        .collect();
    let profile = toggle
        .iter()
        .filter_map(|(_, profile)| profile.clone())
        .find(|profile| !profile.is_empty())
        .unwrap_or_default();
    Some(ProfileLock {
        locked: false,
        profile,
        toggle,
    })
}

/// The name the menu shows for `profile`.
pub(super) fn profile_label(config: &Config, recent: &[ForegroundApp], profile: &str) -> String {
    if profile.is_empty() {
        rust_i18n::t!("common.default").into_owned()
    } else {
        profile_display_name(&config.apps, recent, profile)
    }
}

#[cfg(test)]
mod tests {
    use openlogi_agent_core::DpiCycleState;
    use openlogi_core::config::{AppDeviceSettings, DeviceConfig};

    use super::*;

    const MOUSE: &str = "mouse";
    const EDITOR: &str = r"c:\tools\editor.exe";

    fn cycles(states: &[(&str, &[u16], bool)]) -> DpiCycles {
        DpiCycles {
            selected: None,
            by_key: states
                .iter()
                .map(|(key, presets, stock)| {
                    let mut state = DpiCycleState::default();
                    state.presets = presets.iter().copied().map(Dpi::new).collect();
                    state.index = 1;
                    state.stock_presets = *stock;
                    ((*key).to_string(), state)
                })
                .collect(),
        }
    }

    fn app(id: &str) -> ForegroundApp {
        ForegroundApp::unnamed(id.to_string())
    }

    fn config_with_editor_profile() -> Config {
        let mut config = Config::default();
        let device = config
            .devices
            .entry(MOUSE.to_string())
            .or_insert_with(DeviceConfig::default);
        device
            .per_app_settings
            .insert(EDITOR.to_string(), AppDeviceSettings::default());
        config
    }

    #[test]
    fn only_saved_dpi_stages_are_offered() {
        let mut config = Config::default();
        config.set_device_custom_name("saved", Some("G502 X".into()));
        let choices = dpi_choices(
            &config,
            &cycles(&[
                ("saved", &[800, 1600], false),
                ("stock", &[400, 800], true),
                ("keyboard", &[], false),
            ]),
        );
        assert_eq!(
            choices,
            [DpiChoice {
                key: "saved".into(),
                device: "G502 X".into(),
                presets: vec![Dpi::new(800), Dpi::new(1600)],
                current: 1,
            }]
        );
    }

    #[test]
    fn locking_pins_the_profile_of_the_app_last_in_front() {
        let config = config_with_editor_profile();
        let lock = profile_lock(
            &config,
            &cycles(&[(MOUSE, &[], false)]),
            &[app(EDITOR), app(r"c:\tools\other.exe")],
        )
        .expect("an online device");
        assert!(!lock.locked);
        assert_eq!(lock.profile, EDITOR);
        assert_eq!(lock.toggle, [(MOUSE.to_string(), Some(EDITOR.to_string()))]);
        assert_eq!(profile_label(&config, &[], &lock.profile), "editor");
    }

    #[test]
    fn a_file_explorer_profile_is_locked_like_any_other() {
        const EXPLORER: &str = r"c:\windows\explorer.exe";
        let mut config = config_with_editor_profile();
        config
            .devices
            .get_mut(MOUSE)
            .unwrap()
            .per_app_settings
            .insert(EXPLORER.to_string(), AppDeviceSettings::default());
        let lock = profile_lock(
            &config,
            &cycles(&[(MOUSE, &[], false)]),
            &[app(EXPLORER), app(EDITOR)],
        )
        .expect("an online device");
        assert_eq!(lock.profile, EXPLORER);
        assert_eq!(
            lock.toggle,
            [(MOUSE.to_string(), Some(EXPLORER.to_string()))]
        );
    }

    #[test]
    fn an_app_without_a_profile_locks_the_default() {
        let config = config_with_editor_profile();
        let lock = profile_lock(
            &config,
            &cycles(&[(MOUSE, &[], false)]),
            &[app(r"c:\tools\other.exe")],
        )
        .expect("an online device");
        assert_eq!(lock.profile, "");
        assert_eq!(lock.toggle, [(MOUSE.to_string(), Some(String::new()))]);
    }

    #[test]
    fn any_lock_shows_checked_and_releases_every_device() {
        let mut config = config_with_editor_profile();
        config.devices.get_mut(MOUSE).unwrap().profile_lock = Some(EDITOR.into());
        let lock = profile_lock(
            &config,
            &cycles(&[(MOUSE, &[], false), ("keyboard", &[], false)]),
            &[],
        )
        .expect("online devices");
        assert!(lock.locked);
        assert_eq!(lock.profile, EDITOR);
        assert_eq!(
            lock.toggle,
            [("keyboard".to_string(), None), (MOUSE.to_string(), None)]
        );
    }

    #[test]
    fn a_locked_app_profile_is_named_over_a_device_locked_to_default() {
        let mut config = config_with_editor_profile();
        config.devices.get_mut(MOUSE).unwrap().profile_lock = Some(EDITOR.into());
        config
            .devices
            .entry("keyboard".into())
            .or_default()
            .profile_lock = Some(String::new());
        let lock = profile_lock(
            &config,
            &cycles(&[(MOUSE, &[], false), ("keyboard", &[], false)]),
            &[],
        )
        .expect("online devices");
        assert!(lock.locked);
        assert_eq!(
            lock.profile, EDITOR,
            "\"keyboard\" sorts first but locks Default"
        );
    }

    #[test]
    fn no_online_device_hides_the_lock() {
        assert_eq!(profile_lock(&Config::default(), &cycles(&[]), &[]), None);
    }
}
