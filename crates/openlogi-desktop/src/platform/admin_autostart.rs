//! Explicit, interactive authorization of the per-user elevated logon task.
//! Never called by startup or background config reconciliation.

use std::path::Path;

/// Desired logon behavior, applied before committing the UI preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Preferences {
    pub(crate) administrator: bool,
    pub(crate) at_login: bool,
}

/// Cancellation is an ordinary outcome and leaves the saved switches alone.
pub(crate) enum Outcome {
    Applied,
    Cancelled,
}

/// The elevated logon task is named `{prefix}{account SID}-{profile}`; the
/// setup script and [`start_agent`] both build it from here.
const TASK_NAME_PREFIX: &str = "MouseKeyStudioAgent-Admin-";

fn task_name_suffix() -> String {
    format!("-{}", openlogi_core::paths::Profile::current().env_value())
}

fn task_name(sid: &str) -> String {
    format!("{TASK_NAME_PREFIX}{sid}{}", task_name_suffix())
}

/// Start the agent through its elevated logon task. Running one's own task
/// needs no UAC prompt, so this is how an unelevated GUI brings the agent back
/// with the administrator rights the user authorized once. `false` when the
/// task is absent or disabled, or Windows refuses — the caller then starts the
/// agent directly.
pub(crate) fn start_agent() -> bool {
    use std::os::windows::process::CommandExt as _;

    let Some(sid) = openlogi_permissions::current_user_sid() else {
        return false;
    };
    let Some(system_root) = std::env::var_os("SystemRoot") else {
        return false;
    };
    std::process::Command::new(std::path::Path::new(&system_root).join("System32/schtasks.exe"))
        .args(["/Run", "/TN", &task_name(&sid)])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|output| output.status.success())
}

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn setup_script(exe: &Path, preferences: Preferences) -> Result<String, String> {
    let exe = exe.to_str().ok_or("agent path is not valid Unicode")?;
    let operation = if preferences.administrator {
        format!(
            r"
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
if ($identity.User.Value -ne $user) {{ throw 'Administrator sign-in account required' }}
$action = New-ScheduledTaskAction -Execute {}
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew
Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null
{}
",
            literal(exe),
            if preferences.at_login {
                ""
            } else {
                "Disable-ScheduledTask -TaskName $name | Out-Null"
            },
        )
    } else {
        "if (Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue) { Unregister-ScheduledTask -TaskName $name -Confirm:$false }".to_owned()
    };
    Ok(format!(
        "$ErrorActionPreference = 'Stop'; $name = {} + $user + {}; try {{ {operation}; exit 0 }} catch {{ exit 1 }}",
        literal(TASK_NAME_PREFIX),
        literal(&task_name_suffix()),
    ))
}

/// Ask Windows once at the user's explicit toggle, and wait off the UI thread.
/// No credentials or writable elevated helper script are stored.
pub(crate) fn configure(exe: &Path, preferences: Preferences) -> Result<Outcome, String> {
    use std::os::windows::process::CommandExt as _;

    let script = setup_script(exe, preferences)?;
    // Capture the caller's SID *before* elevation: alternate admin credentials
    // must not quietly install this user's mouse agent for a different account.
    let wrapper = format!(
        r"$ErrorActionPreference = 'Stop'
$user = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$script = '$user = ' + [char]39 + $user + [char]39 + ';' + {}
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
try {{
    $child = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList @('-NoProfile', '-NonInteractive', '-WindowStyle', 'Hidden', '-EncodedCommand', $encoded) -Verb RunAs -WindowStyle Hidden -Wait -PassThru
    exit $child.ExitCode
}} catch {{
    if ($_.Exception.NativeErrorCode -eq 1223 -or $_.Exception.InnerException.NativeErrorCode -eq 1223) {{ exit 2 }}
    exit 1
}}",
        literal(&script),
    );
    let powershell = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .ok_or("Windows system directory is unavailable")?
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let status = std::process::Command::new(powershell)
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command"])
        .arg(wrapper)
        .creation_flags(CREATE_NO_WINDOW) // UAC remains visible.
        .status()
        .map_err(|error| error.to_string())?;
    match status.code() {
        Some(0) => Ok(Outcome::Applied),
        Some(2) => Ok(Outcome::Cancelled),
        _ => Err("Windows could not update administrator autostart".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setup_script_and_the_start_name_the_same_task() {
        let script = setup_script(
            Path::new(r"C:gent.exe"),
            Preferences {
                administrator: true,
                at_login: true,
            },
        )
        .expect("a Unicode path");
        let suffix = task_name_suffix();
        assert!(
            script.contains(&format!(
                "$name = 'MouseKeyStudioAgent-Admin-' + $user + '{suffix}';"
            )),
            "{script}"
        );
        assert_eq!(
            task_name("S-1-5-21-1"),
            format!("MouseKeyStudioAgent-Admin-S-1-5-21-1{suffix}")
        );
    }

    #[test]
    fn executable_paths_are_literal_even_with_apostrophes_and_powershell_syntax() {
        assert_eq!(
            literal("C:\\O'Brien\\$mouse.exe"),
            "'C:\\O''Brien\\$mouse.exe'"
        );
    }
}
