//! One owner for updater eligibility, consumed by Settings and the source.

use crate::platform::installation::{Installation, InstallationSource};
use gpui::App;
use gpui_updater::{Release, StaticManifestSource, UpdateSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Availability {
    Ready,
    Detecting,
    Portable,
    Unsupported,
    MissingKey,
}

impl Availability {
    pub(crate) const fn reason_key(self) -> &'static str {
        match self {
            Self::Ready => "updates.owned_feed_ready",
            Self::Detecting => "updates.installation_detecting",
            Self::Portable => "updates.portable_manual",
            Self::Unsupported => "updates.owned_msi_only",
            Self::MissingKey => "updates.missing_verification_key",
        }
    }
}

fn decide(installation: Installation, has_key: bool) -> Availability {
    match installation {
        Installation::Detecting => Availability::Detecting,
        Installation::Detected(InstallationSource::WindowsPortable) => Availability::Portable,
        Installation::Detected(InstallationSource::WindowsMsi) if has_key => Availability::Ready,
        Installation::Detected(InstallationSource::WindowsMsi) => Availability::MissingKey,
        Installation::Detected(_) => Availability::Unsupported,
    }
}

pub(crate) fn availability(cx: &App) -> Availability {
    decide(
        cx.try_global::<Installation>()
            .copied()
            .unwrap_or(Installation::Detecting),
        super::minisign_public_key().is_some(),
    )
}

pub(super) struct OwnedMsiSource(StaticManifestSource);
impl OwnedMsiSource {
    pub(super) fn new(source: StaticManifestSource) -> Self {
        Self(source)
    }
}

fn guarded_fetch(
    availability: Availability,
    fetch: impl FnOnce() -> gpui_updater::Result<Release>,
) -> gpui_updater::Result<Release> {
    if availability != Availability::Ready {
        return Err(gpui_updater::Error::VerificationRequired(format!(
            "automatic updates unavailable ({})",
            availability.reason_key()
        )));
    }
    let release = fetch()?;
    // The manifest is untrusted until the MSI signature is checked. The
    // engine joins this name to its download directory and writes it first,
    // so accept only this distribution's literal installer filenames.
    if !matches!(
        release.asset.name.as_str(),
        "MouseKeyStudio-windows-x64.msi" | "MouseKeyStudio-windows-arm64.msi"
    ) {
        return Err(gpui_updater::Error::VerificationRequired(
            "unexpected MSI asset name in the update manifest".into(),
        ));
    }
    Ok(release)
}

impl UpdateSource for OwnedMsiSource {
    fn fetch_latest(&self) -> gpui_updater::Result<Release> {
        let status = decide(
            Installation::Detected(InstallationSource::detect()),
            super::minisign_public_key().is_some(),
        );
        guarded_fetch(status, || self.0.fetch_latest())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn release(name: &str) -> Release {
        Release::new(
            gpui_updater::Version::parse("1.0.0").unwrap(),
            gpui_updater::Asset::new(name, "https://invalid.invalid/unused", 1),
        )
        .with_sha256("a".repeat(64))
        .with_minisign_signature("nonempty-metadata-only-test")
    }

    #[test]
    fn untrusted_manifest_cannot_choose_a_download_path_or_an_executable() {
        for name in [
            "../../config.toml",
            r"..\..\config.toml",
            r"C:\Users\Example\config.toml",
            r"\config.toml",
            r"C:config.toml",
            "MouseKeyStudio-windows-x64.msi:payload",
            "MouseKeyStudio.exe",
            "",
        ] {
            let error = guarded_fetch(Availability::Ready, || Ok(release(name))).unwrap_err();
            assert!(
                matches!(error, gpui_updater::Error::VerificationRequired(_)),
                "reject untrusted asset name {name:?} before download"
            );
        }
        for name in [
            "MouseKeyStudio-windows-x64.msi",
            "MouseKeyStudio-windows-arm64.msi",
        ] {
            assert_eq!(
                guarded_fetch(Availability::Ready, || Ok(release(name)))
                    .unwrap()
                    .asset
                    .name,
                name
            );
        }
    }

    #[test]
    fn portable_unknown_and_unsigned_builds_never_reach_the_network() {
        for has_key in [false, true] {
            for source in [
                InstallationSource::WindowsPortable,
                InstallationSource::Unknown,
                InstallationSource::MacAppBundle,
                InstallationSource::Nix,
            ] {
                let calls = Cell::new(0);
                let result = guarded_fetch(decide(Installation::Detected(source), has_key), || {
                    calls.set(calls.get() + 1);
                    Ok(release("MouseKeyStudio-windows-x64.msi"))
                });
                result.unwrap_err();
                assert_eq!(calls.get(), 0);
            }
        }
        assert_eq!(
            decide(Installation::Detecting, true),
            Availability::Detecting
        );
        assert_eq!(
            decide(
                Installation::Detected(InstallationSource::WindowsMsi),
                false
            ),
            Availability::MissingKey
        );
        assert_eq!(
            guarded_fetch(Availability::MissingKey, || {
                Ok(release("MouseKeyStudio-windows-x64.msi"))
            })
            .unwrap_err()
            .to_string(),
            "verification policy not satisfied: automatic updates unavailable (updates.missing_verification_key)"
        );
    }

    #[test]
    fn only_an_owned_msi_with_our_key_can_fetch() {
        let status = decide(Installation::Detected(InstallationSource::WindowsMsi), true);
        assert_eq!(status, Availability::Ready);
        assert_eq!(
            guarded_fetch(status, || Ok(release("MouseKeyStudio-windows-x64.msi")))
                .unwrap()
                .version,
            gpui_updater::Version::parse("1.0.0").unwrap()
        );
        assert_eq!(
            super::super::manifest_source().url(),
            "https://github.com/MaxITService/MouseKeyStudio/releases/latest/download/latest.json"
        );
        assert_eq!(
            super::super::followed_channel(None),
            openlogi_core::update_feed::UpdateChannel::Stable
        );
    }
}
