# Mouse & Key Studio — Windows portable

Extract the entire ZIP into a writable folder. Open `MouseKeyStudio.exe` or
`Start-MouseKeyStudio.cmd`. Keep the agent and overlay beside the GUI.
Administrator privileges are not required for ordinary use. The executables
are unsigned. Login startup is controlled by the application's settings;
the ZIP has no installer or service installation step.

Modifications by **Maxim Fomin**.

## Where settings live

In the portable ZIP, profiles and macros default to **`settings/config.toml`
inside the extracted application folder**. The app creates the `settings`
subfolder when it first saves settings. No folder selection is required.
The folder must be writable; if it is read-only, move the entire portable app
to a writable location or choose a writable settings folder in the app.

The small `mousekey-studio-portable.toml` file beside the executables contains
`directory = "settings"`. Keep it with the GUI, agent and overlay. Its relative
path lets the settings travel with the whole portable folder, including when
its drive letter changes. Do not extract a newer ZIP over that file or your
existing settings: unpack updates separately and preserve your chosen pointer
and `settings` folder. Portable updates are manual downloads.

In **Settings → General → Settings folder**, the **settings pointer** section
shows both possible pointer locations, their complete paths and path lengths:

- **Beside the portable app** — the pointer travels with the app's folder.
- **In my user profile** — the pointer stays on this computer. It is scoped to
  this particular portable folder, so two portable copies remain independent.
  Moving or renaming that folder does not transfer a user-profile pointer;
  move the pointer beside the app first if you want to carry your choice along.

The app checks write access using a temporary file that it writes and removes,
without replacing your pointer. It reports failures and disables unavailable
destinations. **Keep pointer here** moves only the pointer; it does not move,
merge or delete settings. **Copy full path** copies the exact path without
visual line-break markers. Access can change after a check; an actual move
failure is still reported. No administrator request is made.

The installed edition retains its user-profile pointer and does not implicitly
become portable just because its directory happens to be writable.
An absolute `XDG_CONFIG_HOME` overrides the user-profile configuration base.

**WARNING: SETTINGS MAY SEEM TO DISAPPEAR AFTER CHANGING FOLDERS.**

In **Settings → General → Settings folder**, choose another existing folder and
confirm the warning. The folder should contain `config.toml` directly.
An empty folder starts with defaults; existing profiles and macros are not copied,
merged or deleted. Quit the GUI **and** the tray/background agent, then reopen
the app. Closing only the main window is not a full restart.

The current folder and the folder selected for next restart are shown separately.
To recover previous settings, choose their old folder and fully restart again.
To recover without opening the app, close both processes and edit the pointer
at the full path displayed in Settings. A portable pointer accepts a path
relative to the app's folder, or an absolute path for an external folder. A
user-profile pointer for this portable copy lives under the configuration
base's `portable-locations` subfolder. It takes priority over the bundled
pointer until you move it back using the app. To reset a portable choice to
its default, remove that copy's user-profile pointer if one exists, and set
`directory = "settings"` in the pointer beside the app. **Keep the portable
marker file itself**: removing it makes a future launch use installed-style
configuration instead. Never delete your actual `config.toml` to reset the
folder choice. Caches and logs are not moved by these controls.

Existing profiles from earlier versions are not copied automatically. To
reuse them, first back up the old `config.toml`, then copy it into the new
portable `settings` folder while the GUI and agent are fully closed. Do not
overwrite a different existing profile file without reviewing both copies.

## Risks and disclaimer

**Read before using the software.**

Incorrect clicks, pointer movement, stuck or repeated key presses, and macros sent to the wrong window can activate unintended controls. Shell commands run with your user account privileges, including administrator privileges when enabled. These actions may delete or overwrite files, send messages, confirm purchases or change device settings. Do not use this software in safety-critical systems or wherever an input error or malfunction could cause death, serious injury or other harm.

Mouse & Key Studio is provided "as is" and "as available", without warranties of any kind. You use it entirely at your own risk. To the maximum extent permitted by applicable law, Maxim Fomin and all other authors, contributors and distributors disclaim all liability for any claims, losses or damages arising from use of, inability to use, or reliance on the software, including death, serious injury, other personal injury, property damage, data loss, lost profits and direct, indirect or consequential damages, even if advised of the possibility. This notice does not exclude or limit liability that cannot lawfully be excluded or limited. Back up your settings and data, and review and test bindings, macros and shell commands in a safe environment before use.

## Source and licenses

Mouse & Key Studio is an independently modified distribution based on
[AprilNEA/OpenLogi](https://github.com/AprilNEA/OpenLogi), not an official upstream
release. Preserve the included MIT/Apache-2.0 and third-party license notices.
The orange mouse artwork is original; upstream proprietary brand art is not
included. Not affiliated with Logitech or endorsed by upstream maintainers.


## MSI edition

The MSI installs this application for the current user under
`%LOCALAPPDATA%\Programs\MouseKeyStudio` and adds `bin\mks.exe` to the user PATH.
It uses the user-profile settings pointer by default, rather than portable
`settings`. Its receipt must match the actual running GUI before in-app updates
are offered. It neither upgrades nor removes upstream OpenLogi.
An optional administrator-autostart feature requires a separate explicit UAC
request; normal installation and use do not require it.
Before uninstalling, turn off launch at login in the app. If administrator
autostart was enabled, disable it there first; removing that separately approved
scheduled task may require administrator approval. Uninstalling preserves profiles.

Only builds containing this project's trusted minisign public key can use its
signed MSI update feed. Unsigned development builds and portable copies use
manual downloads from this project's GitHub Releases. Never replace the pointer
or settings when applying a portable ZIP update. See `NAME-AND-ORIGIN.md` for
upstream attribution and license links.
