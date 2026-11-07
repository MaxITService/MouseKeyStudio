# Mouse & Key Studio — made for my G502 LIGHTSPEED; very likely not to work with other devices
[![Hits](https://hits.sh/github.com/MaxITService/MouseKeyStudio.svg?style=flat)](https://hits.sh/github.com/MaxITService/MouseKeyStudio/)

A native, local-first Logitech device manager for Windows, based on
[AprilNEA/OpenLogi](https://github.com/AprilNEA/OpenLogi). Written in Rust;
profiles and macros stay on your computer in plain TOML.

This is an independently modified distribution, not an official upstream
release. The original MIT/Apache-2.0 notices and contributor credits are retained.
The application icon is original artwork; upstream proprietary branding is excluded.

Modifications by **Maxim Fomin**.

## Windows builds

See [manual GitHub Actions build](docs/GITHUB-BUILD.md). Only Windows x64 is
packaged here. The workflow runs exclusively through **Actions → Mouse & Key Studio
Windows packages → Run workflow** on `main`, producing compact ZIP and MSI artifacts. It does
not run on pull requests or pushes and does not publish releases.

Extract the entire ZIP and launch `MouseKeyStudio.exe`. Read the included
[portable guide](docs/PORTABLE.md) before changing the settings folder.

## Settings folder

**Settings → General → Settings folder** lets you select an existing folder.
Portable builds use their own `settings` subfolder by default. Installed builds
use `%USERPROFILE%\.config\mousekey-studio` on Windows (or
`$XDG_CONFIG_HOME/mousekey-studio` when overridden). The separate
pointer section checks write access and lets you keep the pointer beside the
portable app or in your user profile, showing complete paths and path lengths.
Moving the pointer preserves the selected settings folder.

> **Warning: your profiles and macros may seem to disappear.** Changing folders
> does not move, merge or delete settings. An empty folder starts with defaults.
> The change applies after fully quitting and restarting both the app and its
> background agent. Choose the old folder again to recover its settings.

## Risks and disclaimer

**Read before using the software.**

Incorrect clicks, pointer movement, stuck or repeated key presses, and macros sent to the wrong window can activate unintended controls. Shell commands run with your user account privileges, including administrator privileges when enabled. These actions may delete or overwrite files, send messages, confirm purchases or change device settings. Do not use this software in safety-critical systems or wherever an input error or malfunction could cause death, serious injury or other harm.

Mouse & Key Studio is provided "as is" and "as available", without warranties of any kind. You use it entirely at your own risk. To the maximum extent permitted by applicable law, Maxim Fomin and all other authors, contributors and distributors disclaim all liability for any claims, losses or damages arising from use of, inability to use, or reliance on the software, including death, serious injury, other personal injury, property damage, data loss, lost profits and direct, indirect or consequential damages, even if advised of the possibility. This notice does not exclude or limit liability that cannot lawfully be excluded or limited. Back up your settings and data, and review and test bindings, macros and shell commands in a safe environment before use.

See [why the name changed and the original licenses](docs/NAME-AND-ORIGIN.md).

## Development and licenses

- [Build instructions](docs/GITHUB-BUILD.md)
- [Developer handbook](docs/DEVELOPMENT.md) (includes upstream platform tooling)
- [Macro agent guide](docs/MACRO-AGENT-GUIDE.md)
- [Original upstream README and credits](docs/UPSTREAM-README.md)
- [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)

Windows is a young port. The portable executables are unsigned. Upstream
macOS/Linux sources and local verification tools remain available, but their
release pipelines are archived and inactive in this distribution.

---

## My projects

- [AivoRelay](https://github.com/MaxITService/AIVORelay) — AI voice dictation and text-to-speech for Windows.
- [OneClickPrompts](https://github.com/MaxITService/OneClickPrompts) — reusable prompt buttons for AI chats.
- [Mouse & Key Studio](https://github.com/MaxITService/MouseKeyStudio) — Logitech device settings, button remapping and macros for Windows.
- [Console2Ai](https://github.com/MaxITService/Console2Ai) — send a PowerShell buffer to AI.
- [AI for Complete Beginners](https://medium.com/@maxim.fomins/ai-for-complete-beginners-guide-llms-f19c4b8a8a79) — an introductory guide to LLMs.
- [Ping-Plotter](https://github.com/MaxITService/Ping-Plotter-PS51) — a PowerShell ping plotting script.
