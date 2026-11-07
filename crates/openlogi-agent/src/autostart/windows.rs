//! Windows: keep `HKCU\…\Run\MouseKeyStudioAgent` pointed at the running agent
//! executable so the next login relaunches it, or remove the value when
//! disabled.
//!
//! Administrator autostart uses the scheduled task configured explicitly by the
//! desktop app. Background reconciliation never requests elevation.

use openlogi_core::config::AppSettings;
use tracing::{debug, warn};

/// HKCU autostart subkey + value name for the agent.
const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "MouseKeyStudioAgent";

pub(super) fn reconcile(settings: &AppSettings) {
    if let Err(e) = apply(settings) {
        warn!(error = %e, enabled = settings.launch_at_login, "agent autostart reconcile failed");
    }
}

fn apply(settings: &AppSettings) -> std::io::Result<()> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let (run, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_SUBKEY)?;
    if settings.launch_at_login && !settings.autostart_as_admin {
        let exe = std::env::current_exe()?;
        // Windows parses Run-key values as command lines, so a bare path with
        // spaces (e.g. under "C:\Program Files\") is split at the first space and
        // the launch silently fails. Quote it. Built via OsString so a non-UTF-8
        // path survives exactly (no lossy `display()`).
        let mut quoted = std::ffi::OsString::from("\"");
        quoted.push(exe.as_os_str());
        quoted.push("\"");
        run.set_value(RUN_VALUE, &quoted)?;
        debug!(value = RUN_VALUE, "agent autostart registry value set");
    } else {
        match run.delete_value(RUN_VALUE) {
            Ok(()) => debug!(value = RUN_VALUE, "agent autostart registry value removed"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("agent autostart registry value already absent");
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
