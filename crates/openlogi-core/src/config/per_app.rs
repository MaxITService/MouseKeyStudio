//! Per-application profiles: the button overrides a device stores for one
//! application, and how the application in front is matched to them.
//!
//! An override replaces a whole button with a single action. Editing addresses
//! a profile by its exact key; matching goes through [`overlay_for`], which
//! also falls back from a Windows executable path to that executable's
//! `exe:<filename>` key, and then to the `app:<alias>` key of the
//! [`Config::apps`] alias that lists the application.

use std::collections::BTreeMap;

use super::{AppDeviceSettings, Config};
use crate::app::{overlay_for, profile_key_for};
use crate::binding::{Action, Binding, ButtonId};
use crate::hid::{Dpi, ReportRate};

impl Config {
    /// Resolve automatic or pinned profile selection. Empty locks select Default.
    #[must_use]
    pub fn profile_app<'a>(&'a self, key: &str, app: Option<&'a str>) -> Option<&'a str> {
        self.devices
            .get(key)
            .and_then(|d| d.profile_lock.as_deref())
            .or(app)
            .filter(|app| !app.is_empty())
    }

    /// Sensor and shifted settings effective for the selected application.
    #[must_use]
    pub fn app_device_settings(&self, key: &str, app: Option<&str>) -> Option<&AppDeviceSettings> {
        overlay_for(&self.devices.get(key)?.per_app_settings, &self.apps, app?)
    }

    /// Mutable settings of an exact profile key, creating them when needed.
    pub fn edit_app_device_settings(&mut self, key: &str, app: &str) -> &mut AppDeviceSettings {
        self.devices
            .entry(key.into())
            .or_default()
            .per_app_settings
            .entry(app.into())
            .or_default()
    }

    /// G-Shift bindings for the selected application, inheriting unmodified buttons.
    #[must_use]
    pub fn effective_gshift_bindings(
        &self,
        key: &str,
        app: Option<&str>,
    ) -> BTreeMap<ButtonId, Action> {
        let mut bindings = self.gshift_overrides(key).cloned().unwrap_or_default();
        if let Some(settings) = self.app_device_settings(key, app) {
            bindings.extend(settings.gshift_bindings.clone());
        }
        bindings
    }

    /// Sensor DPI for the selected application, otherwise the route/device default.
    #[must_use]
    pub fn profile_dpi(&self, key: &str, route: &str, app: Option<&str>) -> Option<Dpi> {
        self.app_device_settings(key, self.profile_app(key, app))
            .and_then(|settings| settings.dpi)
            .or_else(|| self.devices.get(key)?.effective_dpi(route))
    }

    /// DPI stages of the selected application, otherwise device stages.
    #[must_use]
    pub fn profile_dpi_presets(&self, key: &str, app: Option<&str>) -> Vec<Dpi> {
        self.app_device_settings(key, self.profile_app(key, app))
            .and_then(|settings| settings.dpi_presets.clone())
            .unwrap_or_else(|| self.dpi_presets(key))
    }

    /// Polling rate of the selected application, otherwise the device default.
    #[must_use]
    pub fn profile_report_rate(&self, key: &str, app: Option<&str>) -> Option<ReportRate> {
        self.app_device_settings(key, self.profile_app(key, app))
            .and_then(|settings| settings.report_rate)
            .or_else(|| self.report_rate(key))
    }

    /// Resolve the effective binding map for `device_key`, overlaying the
    /// per-app entry for `bundle_id` (if any) on top of the global per-device
    /// `bindings`. A per-app override replaces the whole button with a
    /// [`Binding::Single`]; everything else falls through.
    ///
    /// Returns an empty map when the device has no recorded bindings yet.
    /// Callers (the GUI / hook) layer their own defaults on top.
    #[must_use]
    pub fn effective_bindings(
        &self,
        device_key: &str,
        bundle_id: Option<&str>,
    ) -> BTreeMap<ButtonId, Binding> {
        let Some(device) = self.devices.get(device_key) else {
            return BTreeMap::new();
        };
        let mut out = device.bindings.clone();
        if let Some(bid) = bundle_id
            && let Some(overlay) = overlay_for(&device.per_app_bindings, &self.apps, bid)
        {
            for (k, v) in overlay {
                out.insert(*k, Binding::Single(v.clone()));
            }
        }
        out
    }

    /// Records a per-app override. Creates the device + app entries as
    /// needed; passing an action of `None` removes the override and prunes
    /// the empty app map.
    pub fn set_per_app_binding(
        &mut self,
        device_key: &str,
        bundle_id: &str,
        button: ButtonId,
        action: Option<Action>,
    ) {
        let entry = self
            .devices
            .entry(device_key.to_string())
            .or_default()
            .per_app_bindings
            .entry(bundle_id.to_string())
            .or_default();
        match action {
            Some(a) => {
                entry.insert(button, a);
            }
            None => {
                entry.remove(&button);
            }
        }
        if let Some(d) = self.devices.get_mut(device_key) {
            d.per_app_bindings.retain(|_, m| !m.is_empty());
        }
    }

    /// The overrides `device_key` stores for the application key `app`,
    /// or `None` when it has no profile for it.
    ///
    /// Exact key, deliberately: this answers "what did the user author under
    /// this key", which is what an editor needs to show and to clear. The
    /// question [`Self::has_app_override`] answers — "will the app in front hit
    /// a profile" — is the matcher's, and goes through the same `exe:` and
    /// alias fallbacks the matcher does. The two look interchangeable and are
    /// not.
    #[must_use]
    pub fn per_app_overrides(
        &self,
        device_key: &str,
        app: &str,
    ) -> Option<&BTreeMap<ButtonId, Action>> {
        self.devices
            .get(device_key)?
            .per_app_bindings
            .get(app)
            .filter(|overrides| !overrides.is_empty())
    }

    /// Every application key `device_key` has a profile for, in key order.
    pub fn app_profiles(&self, device_key: &str) -> impl Iterator<Item = &str> {
        self.devices.get(device_key).into_iter().flat_map(|device| {
            device
                .per_app_bindings
                .keys()
                .chain(device.per_app_settings.keys())
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>()
        })
    }

    /// Drop `device_key`'s whole profile for `app`. Nothing happens when there
    /// is none.
    pub fn remove_app_profile(&mut self, device_key: &str, app: &str) {
        if let Some(device) = self.devices.get_mut(device_key) {
            device.per_app_bindings.remove(app);
            device.per_app_settings.remove(app);
            if device.profile_lock.as_deref() == Some(app) {
                device.profile_lock = None;
            }
        }
    }

    /// Whether `device_key` has a profile for the foreground app `app`.
    /// Includes profiles that change only sensor settings. Drives the
    /// menu-bar popover's "override active" badge.
    #[must_use]
    pub fn has_app_override(&self, device_key: &str, app: &str) -> bool {
        self.app_profile_for(device_key, app).is_some()
    }

    /// The key of the profile `device_key` runs the foreground application
    /// `app` under, matched as [`Self::effective_bindings`] matches it — the
    /// exact key, an `exe:<filename>` key, or an `app:<alias>` key — or `None`
    /// when the global bindings apply.
    #[must_use]
    pub fn app_profile_for(&self, device_key: &str, app: &str) -> Option<&str> {
        let device = self.devices.get(device_key)?;
        let app = self.profile_app(device_key, Some(app))?;
        profile_key_for(&device.per_app_bindings, &self.apps, app)
            .or_else(|| profile_key_for(&device.per_app_settings, &self.apps, app))
    }

    /// The first per-app profile, by device and then key, whose `app:` key
    /// names an alias [`Self::apps`] does not define, as `(device, key)`.
    /// Such a profile could never apply, so loading rejects it.
    #[cfg(feature = "fs")]
    pub(super) fn undefined_app_alias(&self) -> Option<(&str, &str)> {
        self.devices.iter().find_map(|(device_key, device)| {
            device
                .per_app_bindings
                .keys()
                .chain(device.per_app_settings.keys())
                .chain(device.action_ring.per_app.keys())
                .chain(device.profile_lock.iter())
                .find(|key| self.apps.is_undefined_reference(key))
                .map(|key| (device_key.as_str(), key.as_str()))
        })
    }
}
