//! Which top-level windows belong to the Windows shell rather than to an
//! application. Pure, so the decision is tested on every host.
//!
//! `explorer.exe` is both the shell and File Explorer: the taskbar, the
//! notification area and the desktop are its windows, and so is every folder
//! window. Only the window class tells them apart, and only for windows the
//! shell process owns — another process may reuse a class name.

/// A shell window that is not an application the user works in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShellSurface {
    /// The wallpaper and desktop icons.
    Desktop,
    /// The taskbar, the notification area, and the shell's window switchers.
    Chrome,
}

/// Classify a top-level window from its class and whether the shell process
/// owns it. A folder window (`CabinetWClass`) is File Explorer, an
/// application, and so is not a surface.
pub(super) fn shell_surface(class: Option<&str>, owned_by_shell: bool) -> Option<ShellSurface> {
    if !owned_by_shell {
        return None;
    }
    match class? {
        "WorkerW" | "Progman" => Some(ShellSurface::Desktop),
        "Shell_TrayWnd"
        | "Shell_SecondaryTrayWnd"
        | "NotifyIconOverflowWindow"
        | "TopLevelWindowForOverflowXamlIsland"
        | "TaskListThumbnailWnd"
        | "XamlExplorerHostIslandWindow"
        | "MultitaskingViewFrame"
        | "ForegroundStaging" => Some(ShellSurface::Chrome),
        _ => None,
    }
}

/// Whether `path` is one of the shell's own UI hosts — Start, Search, the
/// notification centre — whose every window is shell chrome. `explorer.exe`
/// is not one: it also hosts File Explorer, see [`shell_surface`].
pub(super) fn is_shell_host(path: &str) -> bool {
    const HOSTS: [&str; 4] = [
        "shellexperiencehost.exe",
        "startmenuexperiencehost.exe",
        "searchhost.exe",
        "shellhost.exe",
    ];
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    HOSTS.iter().any(|host| file.eq_ignore_ascii_case(host))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_window_is_file_explorer_not_the_shell() {
        assert_eq!(shell_surface(Some("CabinetWClass"), true), None);
    }

    #[test]
    fn the_taskbar_and_tray_are_shell_chrome() {
        for class in [
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
            "NotifyIconOverflowWindow",
            "TopLevelWindowForOverflowXamlIsland",
        ] {
            assert_eq!(
                shell_surface(Some(class), true),
                Some(ShellSurface::Chrome),
                "{class}"
            );
        }
    }

    #[test]
    fn the_desktop_is_its_own_surface() {
        assert_eq!(
            shell_surface(Some("WorkerW"), true),
            Some(ShellSurface::Desktop)
        );
        assert_eq!(
            shell_surface(Some("Progman"), true),
            Some(ShellSurface::Desktop)
        );
    }

    #[test]
    fn a_shell_class_in_another_process_is_an_application() {
        assert_eq!(shell_surface(Some("WorkerW"), false), None);
        assert_eq!(shell_surface(Some("Shell_TrayWnd"), false), None);
        assert_eq!(shell_surface(None, true), None);
    }

    #[test]
    fn shell_hosts_are_named_by_executable_but_explorer_is_not_one() {
        assert!(is_shell_host(
            r"c:\windows\systemapps\microsoft.windows.startmenuexperiencehost_cw5n1h2txyewy\startmenuexperiencehost.exe"
        ));
        assert!(is_shell_host(r"C:\Windows\SystemApps\SearchHost.exe"));
        assert!(!is_shell_host(r"c:\windows\explorer.exe"));
        assert!(!is_shell_host(r"c:\tools\editor.exe"));
    }
}
