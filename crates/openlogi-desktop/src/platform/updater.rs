//! This distribution updates only its own MSI installation, from its own
//! signed stable feed. Portable ZIPs and unverified builds use manual downloads.
//! The source repeats the ownership gate on its worker before any HTTP request,
//! so menu actions cannot bypass the policy shown in Settings.

use gpui::{App, AppContext as _, Context, Entity, Global, Subscription};
use gpui_updater::{
    EngineConfig, StaticManifestSource, UpdateStatus, Updater, Verification, Version,
};
use openlogi_core::config::AppSettings;
use openlogi_core::update_feed::UpdateChannel;

use crate::state::AppState;

mod policy;
pub(crate) use policy::{Availability, availability};

const MANIFEST_URL: &str =
    "https://github.com/MaxITService/MouseKeyStudio/releases/latest/download/latest.json";
const MINISIGN_PUBLIC_KEY: Option<&str> = option_env!("MKS_UPDATE_MINISIGN_PUBLIC_KEY");

/// The independent distribution currently publishes one stable feed.
/// A channel saved by an upstream configuration cannot redirect this build.
pub(crate) fn followed_channel(_settings: Option<&AppSettings>) -> UpdateChannel {
    UpdateChannel::Stable
}

pub(crate) fn channel_in_use(_cx: &App) -> UpdateChannel {
    followed_channel(None)
}

#[derive(Clone)]
pub struct SharedUpdater(pub Entity<Updater>);
impl Global for SharedUpdater {}

struct UpdaterObserver(
    #[expect(
        dead_code,
        reason = "retains the updater subscription for the app lifetime"
    )]
    Subscription,
);
impl Global for UpdaterObserver {}

pub fn new_entity(cx: &mut App) -> Entity<Updater> {
    cx.new(engine_updater)
}

fn engine_updater(cx: &mut Context<Updater>) -> Updater {
    Updater::new(
        policy::OwnedMsiSource::new(manifest_source()),
        engine_config(minisign_public_key()),
        cx,
    )
}

fn engine_config(key: Option<&str>) -> EngineConfig {
    #[expect(
        clippy::expect_used,
        reason = "Cargo supplies a valid semver package version"
    )]
    let version = Version::parse(env!("CARGO_PKG_VERSION")).expect("embedded semver");
    let mut config = EngineConfig::new(version).verification(Verification::Strict);
    if let Some(key) = key {
        config = config.minisign_public_key(key);
    }
    config
}

fn manifest_source() -> StaticManifestSource {
    StaticManifestSource::new(MANIFEST_URL)
        .os("windows")
        .arch(match std::env::consts::ARCH {
            "aarch64" => "arm64",
            arch => arch,
        })
        .format("msi")
}

pub(super) fn minisign_public_key() -> Option<&'static str> {
    MINISIGN_PUBLIC_KEY
        .map(str::trim)
        .filter(|key| !key.is_empty())
}

pub fn install(cx: &mut App, settings: &AppSettings) {
    let updater = new_entity(cx);
    let observer = cx.observe(&updater, |updater, cx| {
        let auto_install =
            AppState::try_read(cx).is_some_and(|state| state.app_settings().auto_install_updates);
        if auto_install && matches!(updater.read(cx).status(), UpdateStatus::Available(_)) {
            updater.update(cx, Updater::download_and_install);
        }
    });
    cx.set_global(UpdaterObserver(observer));
    if settings.check_for_updates {
        // The worker-owned source gates this check, including early startup
        // before installation metadata has reached the UI.
        updater.update(cx, Updater::check);
    }
    cx.set_global(SharedUpdater(updater));
}

pub fn shared(cx: &App) -> Option<Entity<Updater>> {
    cx.try_global::<SharedUpdater>()
        .map(|global| global.0.clone())
}

pub(crate) fn set_auto_install_updates(enabled: bool, cx: &mut App) {
    AppState::apply(cx, |state| state.commit_auto_install_updates(enabled));
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;
    use openlogi_core::config::Config;

    use super::*;
    use crate::services::assets::AssetResolver;
    use crate::state::{ConfigPersistence, Sources};

    #[test]
    fn actual_engine_rejects_missing_trust_signature_or_checksum_before_download() {
        use gpui_updater::{Asset, Release, UpdateEngine, UpdateSource};

        struct Feed(Release);
        impl UpdateSource for Feed {
            fn fetch_latest(&self) -> gpui_updater::Result<Release> {
                Ok(self.0.clone())
            }
        }

        let release = Release::new(
            Version::parse("999.0.0").unwrap(),
            Asset::new(
                "MouseKeyStudio-windows-x64.msi",
                "https://invalid.invalid/unused",
                1,
            ),
        );
        let signed = release
            .clone()
            .with_minisign_signature("metadata-only-test");
        let complete = signed.clone().with_sha256("a".repeat(64));
        for (key, metadata) in [
            (None, complete.clone()),
            (
                Some("test-only-public-key"),
                release.with_sha256("a".repeat(64)),
            ),
            (Some("test-only-public-key"), signed),
        ] {
            let error = UpdateEngine::new(Feed(metadata), engine_config(key))
                .check()
                .unwrap_err();
            assert!(matches!(
                error,
                gpui_updater::Error::VerificationRequired(_)
            ));
        }
        // Presence passes the check. Authenticity is verified by the engine
        // after download; this test never downloads or installs an artifact.
        assert!(
            UpdateEngine::new(Feed(complete), engine_config(Some("test-only-public-key")))
                .check()
                .unwrap()
                .is_some()
        );
    }

    /// Construct the real updater without a check; preferences still persist.
    #[gpui::test]
    fn the_engine_runs_and_settings_still_commit(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let (commands, _) = tokio::sync::mpsc::unbounded_channel();
            let state = cx.new(|_| {
                AppState::new(Sources::in_memory(
                    Config::ephemeral(),
                    &AssetResolver::new(),
                    commands,
                ))
            });
            AppState::set_global(state, cx);
            let settings = AppState::try_read(cx).unwrap().app_settings().clone();
            assert!(!settings.check_for_updates, "no check may run in this test");
            install(cx, &settings);

            let updater = shared(cx).unwrap();
            assert!(!updater.read(cx).is_driver_backed());

            for enabled in [true, false] {
                set_auto_install_updates(enabled, cx);
                assert_eq!(
                    AppState::try_read(cx)
                        .unwrap()
                        .app_settings()
                        .auto_install_updates,
                    enabled
                );
            }
        });
    }

    /// A channel picked after launch is committed, but this launch's updater
    /// keeps the channel it started on.
    #[gpui::test]
    fn a_picked_channel_commits_and_applies_at_the_next_launch(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let (commands, _) = tokio::sync::mpsc::unbounded_channel();
            let state = cx.new(|_| {
                AppState::new(Sources::in_memory(
                    Config::ephemeral(),
                    &AssetResolver::new(),
                    commands,
                ))
            });
            AppState::set_global(state, cx);
            let settings = AppState::try_read(cx).unwrap().app_settings().clone();
            assert!(!settings.check_for_updates, "no check may run in this test");
            assert_eq!(settings.update_channel, None);
            let launched = followed_channel(None);
            assert_eq!(channel_in_use(cx), launched);
            install(cx, &settings);

            let picked = match launched {
                UpdateChannel::Stable => UpdateChannel::Beta,
                UpdateChannel::Beta => UpdateChannel::Stable,
            };
            AppState::apply(cx, |state| state.commit_update_channel(picked));
            let committed = AppState::try_read(cx).unwrap().app_settings().clone();
            assert_eq!(committed.update_channel, Some(picked));
            assert_eq!(followed_channel(Some(&committed)), UpdateChannel::Stable);
            assert_eq!(channel_in_use(cx), launched);
            assert_eq!(
                crate::services::diagnostics::collect(cx).app.update_channel,
                Some(launched),
                "the report names the channel the updater uses, not the pending pick"
            );
        });
    }

    /// A save the config refuses keeps the configured value; that retained
    /// value is what `set_auto_install_updates` mirrors into Sparkle afterwards.
    #[gpui::test]
    fn a_refused_save_keeps_the_configured_auto_install(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let (commands, _) = tokio::sync::mpsc::unbounded_channel();
            let resolver = AssetResolver::new();
            let state = cx.new(|_| {
                AppState::new(Sources {
                    persistence: ConfigPersistence::ReadOnly("read-only".into()),
                    ..Sources::in_memory(Config::ephemeral(), &resolver, commands)
                })
            });
            AppState::set_global(state, cx);
            let settings = AppState::try_read(cx).unwrap().app_settings().clone();
            assert!(!settings.check_for_updates, "no check may run in this test");
            install(cx, &settings);

            let configured = settings.auto_install_updates;
            set_auto_install_updates(!configured, cx);
            assert_eq!(
                AppState::try_read(cx)
                    .unwrap()
                    .app_settings()
                    .auto_install_updates,
                configured
            );
        });
    }
}
