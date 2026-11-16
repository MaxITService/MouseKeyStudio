//! This process's administrator rights, and the Windows relaunch that gains
//! them.

/// The argument an elevated relaunch starts with: the new process waits for
/// the old one to release the single-instance lock instead of exiting.
pub(crate) const RELAUNCH_ARG: &str = "--elevated-relaunch";

/// Whether this process runs with administrator rights. Elevation is fixed for
/// the process lifetime, so the token is read once.
pub(crate) fn is_elevated() -> bool {
    static ELEVATED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ELEVATED.get_or_init(openlogi_permissions::is_elevated)
}

/// Whether this process was started by [`relaunch_elevated`].
pub(crate) fn is_relaunch() -> bool {
    std::env::args().any(|arg| arg == RELAUNCH_ARG)
}

/// Why an elevated relaunch did not start.
#[cfg(windows)]
#[derive(Debug)]
pub(crate) enum RelaunchError {
    /// The user declined the UAC prompt — an ordinary answer, not a failure.
    Cancelled,
    /// Windows could not start the process.
    Failed(std::io::Error),
}

/// Start this executable again with administrator rights, through the UAC
/// prompt. Blocks until the user answers it, so call it off the UI thread.
#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "raw win32: ShellExecuteExW with the runas verb — localized here"
)]
pub(crate) fn relaunch_elevated() -> Result<(), RelaunchError> {
    use windows_sys::Win32::Foundation::ERROR_CANCELLED;
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt as _;
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    let exe = std::env::current_exe().map_err(RelaunchError::Failed)?;
    let file = wide(exe.as_os_str());
    let verb = wide("runas".as_ref());
    let parameters = wide(RELAUNCH_ARG.as_ref());
    let directory = exe.parent().map(|dir| wide(dir.as_os_str()));

    // SAFETY: every pointer refers to a NUL-terminated buffer that outlives the
    // call; the zeroed struct is valid for every field not set here.
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "SHELLEXECUTEINFOW is about a hundred bytes"
        )]
        {
            info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        }
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = parameters.as_ptr();
        info.lpDirectory = directory.as_ref().map_or(std::ptr::null(), Vec::as_ptr);
        info.nShow = SW_SHOWNORMAL;
        if ShellExecuteExW(&raw mut info) == 0 {
            let error = std::io::Error::last_os_error();
            return Err(
                if error.raw_os_error() == i32::try_from(ERROR_CANCELLED).ok() {
                    RelaunchError::Cancelled
                } else {
                    RelaunchError::Failed(error)
                },
            );
        }
        if !info.hProcess.is_null() {
            windows_sys::Win32::Foundation::CloseHandle(info.hProcess);
        }
    }
    Ok(())
}
