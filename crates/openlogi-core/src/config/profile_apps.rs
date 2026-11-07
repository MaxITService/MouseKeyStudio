//! Transactional application assignment, including migration of direct profile keys.

use std::collections::BTreeMap;

use thiserror::Error;

use super::Config;
use crate::app::{AppAliasError, profile_key_for};

/// Why an application could not be attached to a profile.
#[derive(Debug, Error)]
pub enum ProfileAssignmentError {
    /// The device has no saved configuration to copy.
    #[error("device {0:?} has no saved configuration")]
    MissingDevice(String),
    /// The identifier already selects another profile, or a more specific
    /// selector would hide the new assignment.
    #[error("application {identifier:?} is already assigned to profile {profile:?}")]
    Conflict {
        /// The application identifier being assigned.
        identifier: String,
        /// The profile currently selected for it.
        profile: String,
    },
    /// The profile has no saved settings and is not an alias, so an alias
    /// created for it would select nothing.
    #[error("profile {0:?} has no saved settings yet")]
    UnsavedProfile(String),
    /// The alias table would become invalid.
    #[error(transparent)]
    Alias(#[from] AppAliasError),
}

impl Config {
    /// Copy this device's profile overrides and ring layout into a new named
    /// profile, leaving application assignments and runtime locks untouched.
    /// Button and sensor overrides retain their inheritance from Default;
    /// the ring layout is copied as displayed. `None` copies Default.
    /// An error leaves the configuration unchanged.
    pub fn duplicate_profile(
        &mut self,
        device_key: &str,
        source: Option<&str>,
        display_name: &str,
    ) -> Result<String, ProfileAssignmentError> {
        let device = self
            .devices
            .get(device_key)
            .ok_or_else(|| ProfileAssignmentError::MissingDevice(device_key.into()))?;
        if let Some(source) = source
            && !profile_keys(device).contains_key(source)
        {
            return Err(ProfileAssignmentError::UnsavedProfile(source.into()));
        }
        let bindings = source
            .and_then(|key| device.per_app_bindings.get(key))
            .cloned()
            .unwrap_or_default();
        let settings = source
            .and_then(|key| device.per_app_settings.get(key))
            .cloned()
            .unwrap_or_default();
        let ring = device.action_ring.profile_layout(source);
        let key = self.apps.insert_unassigned(display_name)?;
        let device = self.devices.entry(device_key.into()).or_default();
        device.per_app_bindings.insert(key.clone(), bindings);
        device.per_app_settings.insert(key.clone(), settings);
        device.action_ring.per_app.insert(key.clone(), ring);
        Ok(key)
    }

    /// Whether applications can be added to `profile`: some device saves
    /// settings, a ring layout or a lock under it, or it names an alias.
    #[must_use]
    pub fn can_assign_profile_applications(&self, profile: &str) -> bool {
        self.apps.for_profile_key(profile).is_some()
            || self
                .devices
                .values()
                .any(|device| profile_keys(device).contains_key(profile))
    }

    /// Add an application to a profile on the current platform. Direct profile
    /// keys become aliases; all devices' bindings, sensor settings, ring layouts
    /// and profile locks migrate together. An error leaves the config unchanged.
    pub fn assign_profile_application(
        &mut self,
        profile: &str,
        display_name: &str,
        identifier: &str,
    ) -> Result<String, ProfileAssignmentError> {
        if profile == identifier
            && !self.apps.is_undefined_reference(profile)
            && self.apps.for_profile_key(profile).is_none()
        {
            return Ok(profile.to_string());
        }
        if self.apps.is_undefined_reference(profile) {
            return Err(AppAliasError::UndefinedProfile(profile.to_string()).into());
        }
        if !self.can_assign_profile_applications(profile) {
            return Err(ProfileAssignmentError::UnsavedProfile(profile.to_string()));
        }
        for device in self.devices.values() {
            let keys = profile_keys(device);
            if let Some(owner) = profile_key_for(&keys, &self.apps, identifier)
                && owner != profile
            {
                return Err(conflict(identifier, owner));
            }
        }
        let mut candidate = self.clone();
        let new_key = candidate
            .apps
            .assign_identifier(profile, display_name, identifier)?;
        if new_key != profile {
            for device in candidate.devices.values_mut() {
                rekey(&mut device.per_app_bindings, profile, &new_key);
                rekey(&mut device.per_app_settings, profile, &new_key);
                rekey(&mut device.action_ring.per_app, profile, &new_key);
                if device.profile_lock.as_deref() == Some(profile) {
                    device.profile_lock = Some(new_key.clone());
                }
            }
        }
        // Converting an exact selector to an alias must not let another exact
        // or filename selector take over either of the assigned applications.
        for device in candidate.devices.values() {
            let keys = profile_keys(device);
            if keys.contains_key(&new_key) {
                for app in [profile, identifier] {
                    if let Some(owner) = profile_key_for(&keys, &candidate.apps, app)
                        && owner != new_key
                    {
                        return Err(conflict(app, owner));
                    }
                }
            }
        }
        *self = candidate;
        Ok(new_key)
    }
}

fn profile_keys(device: &super::DeviceConfig) -> BTreeMap<String, &String> {
    device
        .per_app_bindings
        .keys()
        .chain(device.per_app_settings.keys())
        .chain(device.action_ring.per_app.keys())
        .chain(device.profile_lock.iter().filter(|key| !key.is_empty()))
        .map(|key| (key.clone(), key))
        .collect()
}

fn rekey<T>(map: &mut BTreeMap<String, T>, old: &str, new: &str) {
    if let Some(value) = map.remove(old) {
        map.insert(new.to_string(), value);
    }
}

fn conflict(identifier: &str, profile: &str) -> ProfileAssignmentError {
    ProfileAssignmentError::Conflict {
        identifier: identifier.to_string(),
        profile: profile.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::{Action, ActionRingLayout, ButtonId};

    const FIRST: &str = "first-app";
    const SECOND: &str = "second-app";

    #[test]
    fn duplicate_profile_copies_settings_without_application_assignments() {
        let mut config = Config::default();
        config.set_per_app_binding("mouse", FIRST, ButtonId::Back, Some(Action::Undo));
        let source = config
            .assign_profile_application(FIRST, "Editor", SECOND)
            .unwrap();
        config
            .edit_app_device_settings("mouse", &source)
            .gshift_bindings
            .insert(ButtonId::Forward, Action::Copy);
        config.edit_app_device_settings("mouse", &source).dpi = Some(crate::hid::Dpi::new(1200));
        config
            .edit_app_device_settings("mouse", &source)
            .dpi_presets = Some(vec![crate::hid::Dpi::new(800), crate::hid::Dpi::new(1600)]);
        config
            .edit_app_device_settings("mouse", &source)
            .report_rate = Some(crate::hid::ReportRate::new(1000));
        let device = config.devices.get_mut("mouse").unwrap();
        device.profile_lock = Some(source.clone());
        device
            .action_ring
            .per_app
            .insert(source.clone(), ActionRingLayout::default());
        let original = device.clone();
        let copy = config
            .duplicate_profile("mouse", Some(&source), "Editor (copy)")
            .unwrap();
        let device = &config.devices["mouse"];
        assert_eq!(
            device.per_app_bindings[&copy],
            original.per_app_bindings[&source]
        );
        assert_eq!(
            device.per_app_settings[&copy],
            original.per_app_settings[&source]
        );
        assert_eq!(
            device.action_ring.per_app[&copy],
            original.action_ring.per_app[&source]
        );
        assert_eq!(device.profile_lock, original.profile_lock);
        assert_eq!(
            device.per_app_bindings[&source],
            original.per_app_bindings[&source]
        );
        let alias = config.apps.for_profile_key(&copy).unwrap().1;
        assert_eq!(alias.display_name(), Some("Editor (copy)"));
        let serialized_alias = toml::Value::try_from(alias).unwrap();
        for platform in crate::app::AppPlatform::ALL {
            assert!(serialized_alias.get(platform.field()).is_none());
        }
        assert_eq!(
            profile_key_for(&device.per_app_bindings, &config.apps, FIRST),
            Some(source.as_str())
        );
        assert_eq!(
            profile_key_for(&device.per_app_bindings, &config.apps, SECOND),
            Some(source.as_str())
        );
        let another = config
            .duplicate_profile("mouse", Some(&copy), "Editor (copy)")
            .unwrap();
        assert_ne!(copy, another);
        assert_eq!(
            config
                .apps
                .for_profile_key(&another)
                .unwrap()
                .1
                .display_name(),
            Some("Editor (copy) 2")
        );
        config
            .assign_profile_application(&copy, "Editor (copy)", "third-app")
            .unwrap();
        assert_eq!(
            profile_key_for(
                &config.devices["mouse"].per_app_bindings,
                &config.apps,
                "third-app"
            ),
            Some(copy.as_str())
        );
        let saved = toml::to_string(&config).unwrap();
        let loaded: Config = toml::from_str(&saved).unwrap();
        assert_eq!(loaded.apps, config.apps);
        assert_eq!(loaded.devices, config.devices);
    }

    #[test]
    fn duplicate_default_inherits_base_and_invalid_sources_leave_config_unchanged() {
        let mut config = Config::default();
        config
            .devices
            .insert("mouse".into(), super::super::DeviceConfig::default());
        let copy = config
            .duplicate_profile("mouse", None, "Default (copy)")
            .unwrap();
        assert!(config.app_profiles("mouse").any(|key| key == copy));
        assert!(config.devices["mouse"].per_app_bindings[&copy].is_empty());
        let before = toml::to_string(&config).unwrap();
        config
            .duplicate_profile("missing", None, "Copy")
            .unwrap_err();
        config
            .duplicate_profile("mouse", Some("missing"), "Copy")
            .unwrap_err();
        assert_eq!(toml::to_string(&config).unwrap(), before);
    }

    #[test]
    fn assigning_another_application_preserves_all_device_profiles_and_locks() {
        let mut config = Config::default();
        for device in ["mouse-a", "mouse-b"] {
            config.set_per_app_binding(device, FIRST, ButtonId::Back, Some(Action::Undo));
            let settings = config.edit_app_device_settings(device, FIRST).clone();
            let device = config.devices.get_mut(device).unwrap();
            device.profile_lock = Some(FIRST.into());
            device
                .action_ring
                .per_app
                .insert(FIRST.into(), ActionRingLayout::default());
            assert_eq!(device.per_app_settings[FIRST], settings);
        }
        let key = config
            .assign_profile_application(FIRST, "Editor", SECOND)
            .unwrap();
        for device in ["mouse-a", "mouse-b"] {
            let device = &config.devices[device];
            for app in [FIRST, SECOND] {
                assert_eq!(
                    profile_key_for(&device.per_app_bindings, &config.apps, app),
                    Some(key.as_str())
                );
                assert_eq!(
                    profile_key_for(&device.action_ring.per_app, &config.apps, app),
                    Some(key.as_str())
                );
                assert_eq!(
                    profile_key_for(&device.per_app_settings, &config.apps, app),
                    Some(key.as_str())
                );
            }
            assert_eq!(device.per_app_bindings[&key][&ButtonId::Back], Action::Undo);
            assert!(device.per_app_settings.contains_key(&key));
            assert!(device.action_ring.per_app.contains_key(&key));
            assert_eq!(device.profile_lock.as_deref(), Some(key.as_str()));
            assert!(!device.per_app_bindings.contains_key(FIRST));
        }
        let same = config
            .assign_profile_application(&key, "Editor", SECOND)
            .unwrap();
        assert_eq!(same, key);
        assert_eq!(
            config
                .apps
                .for_profile_key(&key)
                .unwrap()
                .1
                .identifiers_here(),
            [FIRST, SECOND]
        );
        let saved = toml::to_string(&config).unwrap();
        let loaded: Config = toml::from_str(&saved).unwrap();
        assert_eq!(loaded.apps, config.apps);
    }

    #[test]
    fn conflicts_leave_the_original_configuration_unchanged() {
        let mut config = Config::default();
        config.set_per_app_binding("mouse", FIRST, ButtonId::Back, Some(Action::Undo));
        config.set_per_app_binding("another-mouse", SECOND, ButtonId::Back, Some(Action::Copy));
        let before = toml::to_string(&config).unwrap();
        assert!(matches!(
            config.assign_profile_application(FIRST, "Editor", SECOND),
            Err(ProfileAssignmentError::Conflict { .. })
        ));
        assert_eq!(toml::to_string(&config).unwrap(), before);
        assert!(!config.can_assign_profile_applications("draft"));
        assert!(matches!(
            config.assign_profile_application("draft", "Draft", "other"),
            Err(ProfileAssignmentError::UnsavedProfile(_))
        ));
        assert_eq!(toml::to_string(&config).unwrap(), before);
        assert!(matches!(
            config.assign_profile_application("app:missing", "Editor", "other"),
            Err(ProfileAssignmentError::Alias(
                AppAliasError::UndefinedProfile(_)
            ))
        ));
        assert_eq!(toml::to_string(&config).unwrap(), before);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn converting_an_exact_path_refuses_to_let_a_filename_profile_take_over() {
        let mut config = Config::default();
        let exact = r"c:\tools\editor.exe";
        config.set_per_app_binding("mouse", exact, ButtonId::Back, Some(Action::Undo));
        config.set_per_app_binding(
            "mouse",
            "exe:editor.exe",
            ButtonId::Back,
            Some(Action::Copy),
        );
        let before = toml::to_string(&config).unwrap();
        assert!(matches!(
            config.assign_profile_application(exact, "Editor", r"c:\tools\second.exe"),
            Err(ProfileAssignmentError::Conflict { .. })
        ));
        assert_eq!(toml::to_string(&config).unwrap(), before);
    }

    #[test]
    fn aliases_keep_foreign_platform_identifiers_and_reject_duplicate_ownership() {
        let mut config: Config = toml::from_str(
            r#"
            schema_version = 12
            [apps.editor]
            name = "Editor"
            macos = "editor"
            linux = "editor"
            windows = "editor"
            [apps.other]
            macos = "taken"
            linux = "taken"
            windows = "taken"
        "#,
        )
        .unwrap();
        config.set_per_app_binding("mouse", "app:editor", ButtonId::Back, Some(Action::Undo));
        let key = config
            .assign_profile_application("app:editor", "Editor", "second")
            .unwrap();
        assert_eq!(key, "app:editor");
        assert_eq!(
            config
                .apps
                .for_profile_key(&key)
                .unwrap()
                .1
                .identifiers_here(),
            ["editor", "second"]
        );
        let table = toml::Value::try_from(&config.apps).unwrap();
        for platform in crate::app::AppPlatform::ALL {
            if platform != crate::app::AppPlatform::CURRENT {
                assert_eq!(table["editor"][platform.field()].as_str(), Some("editor"));
            }
        }
        let before = toml::to_string(&config).unwrap();
        assert!(matches!(
            config.assign_profile_application(&key, "Editor", "taken"),
            Err(ProfileAssignmentError::Alias(
                AppAliasError::DuplicateIdentifier { .. }
            ))
        ));
        assert_eq!(toml::to_string(&config).unwrap(), before);
    }
}
