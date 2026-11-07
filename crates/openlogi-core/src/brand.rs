//! Brand constants shared across the workspace: the project's public URLs,
//! the `mousekey-studio://` deep-link command vocabulary, the three processes' bundle
//! identifiers, and the app bundle's layout — where the helpers live and what
//! they are called on disk.
//!
//! All of it lives here, in the platform-free core crate, so the agent (which
//! *emits* tray deep links and finds the overlay), the GUI (which *parses* the
//! deep links and finds the agent), and the packaging tooling share a single
//! source of truth: the command names can't drift across the process boundary,
//! a helper is looked up at runtime by the name packaging gave it, and a rename
//! touches one file.

use std::path::Path;

/// This independent distribution’s GitHub repository.
pub const REPO_URL: &str = "https://github.com/MaxITService/MouseKeyStudio";
/// The README, used as the in-app "Help" link.
pub const HELP_URL: &str = "https://github.com/MaxITService/MouseKeyStudio#readme";
/// The "latest release" page.
pub const RELEASES_URL: &str = "https://github.com/MaxITService/MouseKeyStudio/releases/latest";

/// The application identifier: the Wayland xdg-toplevel `app_id` (and X11
/// `WM_CLASS`) the GUI advertises, the root of the macOS bundle-id family
/// (`io.github.maxitservice.mousekeystudio.agent`, `io.github.maxitservice.mousekeystudio-dev`), and the value the Linux
/// `.desktop` file pins as `StartupWMClass`. Defined once here so the window the
/// compositor sees, the launcher that groups it, and the frontmost backend that
/// self-identifies OpenLogi can never disagree. The `.desktop` file carries its
/// own literal copy (it can't reference Rust) — keep the two in sync.
pub const APP_ID: &str = "io.github.maxitservice.mousekeystudio";

/// The always-on agent's bundle identifier — the process that owns the hook and
/// holds the Accessibility grant, shipped as a nested login item.
pub const AGENT_ID: &str = "io.github.maxitservice.mousekeystudio.agent";

/// The Actions Ring overlay's bundle identifier, the second nested login item.
pub const OVERLAY_ID: &str = "io.github.maxitservice.mousekeystudio.overlay";

/// The agent's macOS launchd service label: the `Label` in the app bundle's
/// embedded `Contents/Library/LaunchAgents/<label>.plist`, what `SMAppService`
/// registers, and the name `launchctl` addresses (`gui/<uid>/<label>`). Dev
/// bundles use [`dev_id`] of this, so a dev registration can never collide
/// with the shipped one.
///
/// Keep this namespace separate from upstream registrations. Once shipped,
/// changing it would orphan Login Items approvals for this distribution.
pub const AGENT_SERVICE_LABEL: &str = "io.github.maxitservice.mousekeystudio.agent.service";

/// What a dev build appends to every identifier above, so a local build can
/// never claim a shipped TCC grant and System Settings shows which of the two
/// installed copies a row belongs to.
const DEV_SUFFIX: &str = "-dev";

/// `id`'s dev-channel counterpart.
///
/// Packaging (`cargo xtask macos`) stamps the result into every `Info.plist`;
/// the agent matches running GUI processes against it. Defined here so the
/// identity a dev bundle carries and the identity anything looks for cannot
/// diverge.
#[must_use]
pub fn dev_id(id: &str) -> String {
    format!("{id}{DEV_SUFFIX}")
}

/// The app's display name: the outer bundle's `CFBundleName` and
/// `CFBundleDisplayName`, and the root the helpers' names are formed from.
pub const APP_NAME: &str = "Mouse & Key Studio";

/// The GUI's executable, as cargo builds it and as the macOS bundle and the
/// Linux packages ship it. The helpers' executables are [`Helper::executable`].
pub const GUI_EXECUTABLE: &str = "mousekey-studio";

/// The CLI command, distinct from both Windows GUI filename spellings.
pub const CLI_EXECUTABLE: &str = "mks";

/// The GUI filename used in Windows archives and MSI packages.
pub const WINDOWS_GUI_EXECUTABLE: &str = "MouseKeyStudio.exe";

/// `name`'s dev-channel counterpart, the way [`dev_id`] is for identifiers:
/// what System Settings shows for a local build's bundle, and the directory a
/// dev helper lives in.
#[must_use]
pub fn dev_name(name: &str) -> String {
    format!("{name} Dev")
}

/// Where the app bundle nests its login-item helpers, relative to its root.
pub const LOGIN_ITEMS_DIR: &str = "Contents/Library/LoginItems";

/// Where the app bundle carries the agent's launchd service plist, relative
/// to its root.
pub const LAUNCH_AGENTS_DIR: &str = "Contents/Library/LaunchAgents";

/// The nested login-item helpers the app bundle embeds.
///
/// Each helper's directory is named exactly like its display name, because
/// macOS privacy panes fall back to a bundle's filename whenever its metadata
/// is stale — a spelling that differs from the display name, or a dev helper
/// named like the shipped one, renders as a row nobody can trust. Packaging
/// writes these names and both the app and the agent look the helpers up by
/// them at runtime, so they are defined once here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Helper {
    /// The always-on agent: the process that owns the hook and holds the
    /// Accessibility grant.
    Agent,
    /// The Actions Ring renderer.
    Overlay,
}

impl Helper {
    /// The helper's bundle identifier.
    #[must_use]
    pub const fn bundle_id(self) -> &'static str {
        match self {
            Self::Agent => AGENT_ID,
            Self::Overlay => OVERLAY_ID,
        }
    }

    /// The shipped display name — and so the shipped bundle directory's name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Agent => "MouseKeyStudio Agent",
            Self::Overlay => "MouseKeyStudio Overlay",
        }
    }

    /// The helper's executable: how cargo names the binary, and how the
    /// process shows up in a process list.
    #[must_use]
    pub const fn executable(self) -> &'static str {
        match self {
            Self::Agent => "mks-agent",
            Self::Overlay => "mks-overlay",
        }
    }

    /// The shipped helper's bundle, relative to the app bundle's root.
    #[must_use]
    pub fn bundle_dir(self) -> String {
        format!("{LOGIN_ITEMS_DIR}/{}.app", self.display_name())
    }

    /// A dev build's helper bundle, relative to the app bundle's root.
    #[must_use]
    pub fn dev_bundle_dir(self) -> String {
        format!("{LOGIN_ITEMS_DIR}/{}.app", dev_name(self.display_name()))
    }

    /// The helper's executable inside `bundle_dir`, one of [`Self::bundle_dir`]
    /// or [`Self::dev_bundle_dir`].
    #[must_use]
    pub fn executable_in(self, bundle_dir: &str) -> String {
        format!("{bundle_dir}/Contents/MacOS/{}", self.executable())
    }

    /// Paths shipped by this distribution. Never search upstream helper bundles.
    #[must_use]
    pub fn executable_candidates(self) -> [String; 2] {
        [
            self.executable_in(&self.dev_bundle_dir()),
            self.executable_in(&self.bundle_dir()),
        ]
    }
}

/// The `.app` root of a packaged helper binary — `…/Foo.app/Contents/MacOS/foo`
/// gives `…/Foo.app` — and `None` for a bare binary such as a cargo build
/// output.
#[must_use]
pub fn helper_bundle_root(executable: &Path) -> Option<&Path> {
    let bundle = executable.ancestors().nth(3)?;
    (bundle.extension()? == "app").then_some(bundle)
}

/// Whether `id` names a dev build — the inverse of [`dev_id`].
///
/// The profile split keys off this: a dev bundle gets its own config directory
/// and IPC socket, so a false negative points a dev build at the user's real
/// config. That asymmetry is why the legacy `.dev` spelling is still accepted —
/// a local bundle built before the rename must not silently claim production
/// state just because nobody rebuilt it.
#[must_use]
pub fn is_dev_id(id: &str) -> bool {
    strip_dev_suffix(id) != id
}

/// Whether `id` — a foreground-application identifier, as
/// [`ForegroundApp::id`](crate::app::ForegroundApp::id) defines one — names one
/// of OpenLogi's own three processes.
///
/// The frontmost-app reader sees the GUI whenever its window is in front, so
/// without this OpenLogi would offer itself as a target for a per-app profile.
/// Both identifier shapes are recognised: the bundle-id family above (macOS
/// bundle ids, and the `WM_CLASS` / `app_id` the GUI advertises on Linux), dev
/// builds included; and the Windows executable path, matched on its file name
/// — the installed names from `packaging/windows/MouseKeyStudio.wxs` (which carries
/// its own literal copy, since it can't reference Rust — keep the two in sync)
/// plus the cargo artifact name a dev build's GUI runs under.
#[must_use]
pub fn is_openlogi_foreground_id(id: &str) -> bool {
    const WINDOWS_SUFFIX: &str = ".exe";
    let executables = [
        CLI_EXECUTABLE,
        "MouseKeyStudio",
        GUI_EXECUTABLE,
        Helper::Agent.executable(),
        Helper::Overlay.executable(),
    ];

    let base = strip_dev_suffix(id);
    [APP_ID, AGENT_ID, OVERLAY_ID]
        .iter()
        .any(|own| base.eq_ignore_ascii_case(own))
        || id
            .rsplit(['\\', '/'])
            .next()
            .filter(|file| ends_with_ignore_ascii_case(file, WINDOWS_SUFFIX))
            .and_then(|file| file.get(..file.len() - WINDOWS_SUFFIX.len()))
            .is_some_and(|stem| executables.iter().any(|exe| stem.eq_ignore_ascii_case(exe)))
}

/// The dev suffix before it was hyphenated. Recognised, never produced.
const LEGACY_DEV_SUFFIX: &str = ".dev";

/// `id` with a recognised dev suffix removed, or `id` unchanged.
fn strip_dev_suffix(id: &str) -> &str {
    [DEV_SUFFIX, LEGACY_DEV_SUFFIX]
        .iter()
        .find(|suffix| ends_with_ignore_ascii_case(id, suffix))
        .and_then(|suffix| id.get(..id.len() - suffix.len()))
        .unwrap_or(id)
}

fn ends_with_ignore_ascii_case(haystack: &str, suffix: &str) -> bool {
    haystack.len() > suffix.len()
        && haystack
            .get(haystack.len() - suffix.len()..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
}

/// The release page for a specific version tag (e.g. the running build).
#[must_use]
pub fn release_tag_url(version: &str) -> String {
    format!("{REPO_URL}/releases/tag/v{version}")
}

/// A GUI action the agent's tray (or any external caller) requests by opening
/// an `mousekey-studio://<name>` URL. macOS delivers it to the running GUI via an
/// Apple Event; the GUI parses it back into this enum and dispatches.
///
/// The agent builds URLs with [`DeeplinkCommand::to_url`]; the GUI reads them
/// with [`DeeplinkCommand::parse_url`]. The command names are defined once, in
/// [`DeeplinkCommand::as_name`], so the two sides cannot disagree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeeplinkCommand {
    /// Show / foreground the main window.
    Show,
    /// Open the Settings window.
    OpenSettings,
    /// Open Settings on the About page.
    OpenAbout,
    /// Run a manual update check and open Settings on the Updates page, where
    /// its status is rendered.
    CheckForUpdates,
    /// Quit the GUI.
    Quit,
}

impl DeeplinkCommand {
    /// The URL scheme OpenLogi registers with LaunchServices.
    pub const SCHEME: &str = "mousekey-studio";

    /// The wire name for this command — the host component of its URL.
    #[must_use]
    pub const fn as_name(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::OpenSettings => "open-settings",
            Self::OpenAbout => "open-about",
            Self::CheckForUpdates => "check-for-updates",
            Self::Quit => "quit",
        }
    }

    /// Build the `mousekey-studio://<name>` URL for this command.
    #[must_use]
    pub fn to_url(self) -> String {
        format!("{}://{}", Self::SCHEME, self.as_name())
    }

    /// Parse a command from its wire name (the part after `mousekey-studio://`).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "show" => Some(Self::Show),
            "open-settings" => Some(Self::OpenSettings),
            "open-about" => Some(Self::OpenAbout),
            "check-for-updates" => Some(Self::CheckForUpdates),
            "quit" => Some(Self::Quit),
            _ => None,
        }
    }

    /// Parse a full `mousekey-studio://…` URL. The command lives in the URL's host
    /// component, so any trailing path or query (`mousekey-studio://show/`,
    /// `mousekey-studio://show?x=1`) is ignored. Returns `None` for a foreign scheme
    /// or an unknown command.
    #[must_use]
    pub fn parse_url(url: &str) -> Option<Self> {
        let rest = url.strip_prefix(Self::SCHEME)?.strip_prefix("://")?;
        let name = rest.split(['/', '?']).next().unwrap_or(rest);
        Self::from_name(name)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        AGENT_ID, APP_ID, DeeplinkCommand, Helper, OVERLAY_ID, dev_id, helper_bundle_root,
        is_dev_id, is_openlogi_foreground_id,
    };

    const ALL: [DeeplinkCommand; 5] = [
        DeeplinkCommand::Show,
        DeeplinkCommand::OpenSettings,
        DeeplinkCommand::OpenAbout,
        DeeplinkCommand::CheckForUpdates,
        DeeplinkCommand::Quit,
    ];

    #[test]
    fn url_round_trips() {
        for cmd in ALL {
            assert_eq!(DeeplinkCommand::parse_url(&cmd.to_url()), Some(cmd));
        }
    }

    #[test]
    fn parse_url_ignores_trailing_path_and_query() {
        assert_eq!(
            DeeplinkCommand::parse_url("mousekey-studio://show/"),
            Some(DeeplinkCommand::Show)
        );
        assert_eq!(
            DeeplinkCommand::parse_url("mousekey-studio://open-settings?from=tray"),
            Some(DeeplinkCommand::OpenSettings)
        );
    }

    #[test]
    fn parse_url_rejects_foreign_scheme_and_unknown_command() {
        assert_eq!(DeeplinkCommand::parse_url("https://example.com/show"), None);
        assert_eq!(DeeplinkCommand::parse_url("openlogi://show"), None);
        assert_eq!(DeeplinkCommand::parse_url("mousekey-studio://bogus"), None);
        assert_eq!(DeeplinkCommand::parse_url("mousekey-studio://"), None);
    }

    #[test]
    fn dev_ids_round_trip() {
        for id in [APP_ID, AGENT_ID, OVERLAY_ID] {
            assert!(is_dev_id(&dev_id(id)), "{id} suffixed must read as dev");
            assert!(!is_dev_id(id), "{id} is production");
        }
    }

    #[test]
    fn helper_bundles_are_named_after_their_display_names() {
        assert_eq!(
            Helper::Agent.bundle_dir(),
            "Contents/Library/LoginItems/MouseKeyStudio Agent.app"
        );
        assert_eq!(
            Helper::Overlay.dev_bundle_dir(),
            "Contents/Library/LoginItems/MouseKeyStudio Overlay Dev.app"
        );
        assert_eq!(Helper::Agent.executable_candidates().len(), 2);
        assert!(
            Helper::Agent
                .executable_candidates()
                .iter()
                .all(|path| !path.contains("OpenLogi"))
        );
    }

    #[test]
    fn a_helper_bundle_root_is_three_levels_above_its_executable() {
        let packaged = Path::new(
            "/Applications/OpenLogi.app/Contents/Library/LoginItems/MouseKeyStudio Agent.app/Contents/MacOS/openlogi-agent",
        );
        assert_eq!(
            helper_bundle_root(packaged),
            Some(Path::new(
                "/Applications/OpenLogi.app/Contents/Library/LoginItems/MouseKeyStudio Agent.app"
            ))
        );
        let dev = Path::new(
            "/Users/me/OpenLogi/target/dev/OpenLogi.app/Contents/Library/LoginItems/MouseKeyStudio Agent Dev.app/Contents/MacOS/openlogi-agent",
        );
        assert_eq!(
            helper_bundle_root(dev),
            Some(Path::new(
                "/Users/me/OpenLogi/target/dev/OpenLogi.app/Contents/Library/LoginItems/MouseKeyStudio Agent Dev.app"
            ))
        );
        assert_eq!(
            helper_bundle_root(Path::new("target/debug/openlogi-agent")),
            None
        );
    }

    #[test]
    fn the_legacy_dotted_suffix_still_reads_as_dev() {
        // A stale `target/dev` bundle from before the rename must not fall
        // through to the production config directory and IPC socket.
        assert!(is_dev_id("io.github.maxitservice.mousekeystudio.agent.dev"));
        assert!(is_dev_id("io.github.maxitservice.mousekeystudio.dev"));
    }

    #[test]
    fn a_bare_suffix_is_not_a_dev_id() {
        assert!(!is_dev_id("-dev"));
        assert!(!is_dev_id(".dev"));
        assert!(!is_dev_id(""));
    }

    #[test]
    fn matching_ignores_case_but_not_position() {
        assert!(is_dev_id("io.github.maxitservice.mousekeystudio.agent-DEV"));
        assert!(!is_dev_id("org.openlogi.dev-agent"));
    }

    #[test]
    fn our_own_processes_are_recognised_in_both_identifier_shapes() {
        for id in [APP_ID, AGENT_ID, OVERLAY_ID] {
            assert!(is_openlogi_foreground_id(id), "{id}");
            assert!(is_openlogi_foreground_id(&dev_id(id)), "dev {id}");
        }
        // Windows reports a lower-cased executable path; a dev build runs the
        // cargo artifact out of `target/`.
        assert!(is_openlogi_foreground_id(
            r"c:\program files\MouseKeyStudio\MouseKeyStudio.exe"
        ));
        assert!(is_openlogi_foreground_id(
            r"c:\program files\MouseKeyStudio\mks-agent.exe"
        ));
        assert!(is_openlogi_foreground_id(
            r"c:\src\openlogi\target\debug\mousekey-studio.exe"
        ));
    }

    #[test]
    fn a_foreign_app_that_merely_starts_the_same_way_is_not_ours() {
        for upstream in [
            "org.openlogi.openlogi",
            "org.openlogi.agent",
            r"C:\OpenLogi\OpenLogi.exe",
            r"C:\OpenLogi\openlogi-agent.exe",
        ] {
            assert!(
                !is_openlogi_foreground_id(upstream),
                "must not claim {upstream}"
            );
        }
        assert!(!is_openlogi_foreground_id(
            "io.github.maxitservice.mousekeystudio.helper"
        ));
        assert!(!is_openlogi_foreground_id(r"c:\apps\openlogic.exe"));
        assert!(!is_openlogi_foreground_id("com.apple.Safari"));
        assert!(!is_openlogi_foreground_id(""));
    }
}
