# Add an OpenLogi macro with a desktop assistant

You can ask a desktop assistant to create and assign a macro for you. For example,
install [Claude Desktop with Cowork](https://claude.com/download) or
[ChatGPT Desktop with Codex](https://chatgpt.com/download/). Use a mode that can
read local files and operate desktop applications, and grant access to this folder.
Available computer-control capabilities depend on the assistant and its settings.

**This file travels with the portable app.** It is `MACRO-AGENT-GUIDE.md` in the
same folder as `MouseKeyStudio.exe`; no fixed installation drive or username is required.
Keep it in that folder when moving or sharing OpenLogi. In the macro editor, open
**Syntax guide…** (or press **F1** while editing a macro), then **Copy guide path**.
Give that path to your assistant, for example:

> Read the OpenLogi instructions at "D:\My apps\OpenLogi\MACRO-AGENT-GUIDE.md".
> On my G502, in the Helldivers profile, assign Forward to hold B, tap Up, Right,
> Down, Right with 50 ms between events, then release B. Add it for me.

Replace the example path with the actual path copied from your app. Inside this
folder, `./MACRO-AGENT-GUIDE.md` refers to this file. The executable is
`./MouseKeyStudio.exe` on Windows. Moving the application folder does not necessarily
move user settings; use the app's configuration location, not an assumed adjacent
config file.
The portable ZIP defaults to `./settings/config.toml`. The user can redirect it
and can choose whether its pointer stays beside the app or in the user profile.
Read the current folder shown in Settings; do not infer it from that default,
move the pointer, or change storage preferences merely to add a macro.

## Instructions for the agent

1. Read this entire file. Resolve paths relative to this file's directory rather
   than your process's working directory. Use the already running OpenLogi when
   possible; otherwise launch the adjacent GUI executable.
2. Establish the target **device, profile, physical control, and Normal/G-Shift
   layer** from the user's request and the app. Ask only for missing choices that
   would change the assignment. Never infer the target from the last selected row.
3. Inspect the current assignment. If replacing an existing macro, use **Copy
   macro** to save its complete JSON to a clearly named backup file before editing.
   Preserve other controls, profiles, application assignments and device settings.
4. Translate the request into the JSON format below. Keep key-down and key-up
   events balanced, including release cleanup. Do not enable repetition unless
   requested. Match the user's game/application key bindings rather than assuming
   the examples' bindings apply everywhere.
5. Select the device and profile, select the physical control and editing layer,
   then open the action editor. Use **Paste macro** for a complete JSON document,
   or **Edit macro as text** to replace the document directly. Use **Load example…**
   only as an editable starting point. Clipboard use replaces clipboard content.
6. Check every phase, timing, repeat flag and the target control before saving.
   **Use text in step editor** validates the draft without assigning it.
   **Save macro** commits the assignment to the selected control/profile/layer.
   The user's request to add a macro authorizes this save. Resolve validation
   errors rather than discarding fields or silently altering timing.
7. Reopen the assigned macro, copy it back, and compare the parsed fields and
   event sequence with the requested result. Take a screenshot when desktop
   control is available. Report the device, profile, layer, button and saved macro.
   Do not trigger the macro in a game, type into another app, or run a shell
   command merely to test it unless the user authorized that behavior.

If you can read files but cannot control the desktop, save the proposed JSON to
`<macro-name>.openlogi-macro.json` beside this guide (or another user-writable
location), and explain how to paste it into **Paste macro** and press **Save macro**.
State clearly that it has not yet been assigned. Do not claim completion from
creating a file alone. The shipped CLI does **not** provide a macro import or
binding-write command; do not invent one. Do not edit the live TOML behind a
running app, restart the agent, install development tools, or rebuild OpenLogi to
add a macro.

## Complete macro JSON

All five fields are required. The three phases are **strings**, not arrays. Empty
phases use `""`. Unknown fields, comments and trailing commas are rejected.

```json
{
  "name": "Ctrl + C",
  "on_press": "+LeftCtrl, 50ms, +C, 50ms, -C, 50ms, -LeftCtrl",
  "while_held": "",
  "on_release": "",
  "repeat_while_held": false
}
```

The user's Ctrl+V example releases Ctrl before releasing V:

```json
{
  "name": "Ctrl + V",
  "on_press": "+LeftCtrl, 50ms, +V, 50ms, -LeftCtrl, 50ms, -V",
  "while_held": "",
  "on_release": "",
  "repeat_while_held": false
}
```

## Key notation and timing

| Text | Meaning |
|---|---|
| `+C` | Press C and keep it down |
| `-C` | Release C |
| `C` or `tap C` | Press and release C, two events |
| `50ms` | Wait 50 milliseconds (0.05 seconds) |
| `+LeftCtrl, C, -LeftCtrl` | Hold left Ctrl while tapping C |
| `+Comma, -Comma` | Press and release the comma key |
| `+Semicolon, -Semicolon` | Press and release the semicolon key |
| `+-, --` | Press and release the minus key |
| `+=, -=` | Press and release equals |

Separate steps with commas, semicolons or newlines. Names ignore case. Delays are
whole, non-negative milliseconds; `0ms` is allowed. In JSON a newline inside a
string is written `\n`, a double quote `\"`, and a backslash `\\`.

Use explicit modifier sides: `LeftCtrl`, `RightCtrl`, `LeftShift`, `RightShift`,
`LeftAlt`, `RightAlt`, `LeftWin`, `RightWin`. Bare `Ctrl` is not a macro key name.
`Ctrl+C` is shortcut-editor notation: a macro needs separate events instead.
On macOS Win maps to Command; use LeftWin for Command shortcuts.

Common names include A–Z, 0–9, F1–F20, Enter, Escape, Backspace, Tab, Space,
PrintScreen, Insert, Home, PageUp, Delete, End, PageDown,
Right, Left, Down, Up, Comma and Semicolon. Punctuation is a physical key:
producing `+` on a US layout needs `+LeftShift, =, -LeftShift`. The active keyboard
layout determines the resulting character. CapsLock, media keys and numpad-specific
keys are not supported in keyboard macros. Open the in-app syntax guide for the
complete supported key list generated from the current parser, aliases and all
accepted input forms. Do not guess unsupported key names.

## Phases, repetition and cancellation

- **On press** starts when the assigned physical control is pressed.
- **While holding** follows On press. With repetition off it runs once; with
  `repeat_while_held: true` this phase loops while the control remains down.
- **On release** runs when the physical control is released. Release can interrupt
  earlier phases in a held macro, so put essential cleanup here for keys that
  span the hold.

A macro with only **On press** steps is a one-shot sequence: a quick click starts
the complete sequence, and releasing the physical button does not interrupt it.
Adding **While holding** or **On release** steps makes the macro follow the
physical button's hold/release lifecycle. Early release then skips the remaining
press/hold steps and starts **On release**. Repetition applies only to a nonempty
**While holding** phase; it does not repeat an On-press-only sequence.
Cancellation (for example, a binding change or agent shutdown) differs from an
ordinary release: it stops playback and releases keys still owned by the macro.

The summary counts the events and programmed delays for one pass through all
three phases. It excludes the user's hold duration and extra repetitions.
Recording appends events and timing to the selected phase. Escape stops recording
without recording Escape; moving focus away also stops it. Recording releases
keys that were still down when it stopped. Review the result before assigning it.

## Helldivers: hold one key while tapping many

These examples use the user's saved B/arrow bindings, not universal game defaults.
EAGLE Airstrike holds B throughout Up → Right → Down → Right:

```json
{
  "name": "EAGLE Airstrike",
  "on_press": "+B, 50ms, +Up, 50ms, -Up, 50ms, +Right, 50ms, -Right, 50ms, +Down, 50ms, -Down, 50ms, +Right, 50ms, -Right, 50ms, -B",
  "while_held": "",
  "on_release": "",
  "repeat_while_held": false
}
```

Resupply holds B throughout Down → Down → Up → Right:

```json
{
  "name": "Resupply",
  "on_press": "+B, 50ms, +Down, 50ms, -Down, 50ms, +Down, 50ms, -Down, 50ms, +Up, 50ms, -Up, 50ms, +Right, 50ms, -Right, 50ms, -B",
  "while_held": "",
  "on_release": "",
  "repeat_while_held": false
}
```

For a key held for the entire physical-button press, use phases instead:

```json
{
  "name": "Hold B, enter sequence, release B",
  "on_press": "+B, 50ms",
  "while_held": "Up, 50ms, Right, 50ms, Down, 50ms, Right",
  "on_release": "-B",
  "repeat_while_held": false
}
```

The Ctrl+C, Ctrl+V, EAGLE Airstrike and Resupply examples are one-shot sequences
and finish after a quick click. Only the final phased example can be interrupted
by early physical release; its release phase releases B even if the arrow
sequence did not finish. Choose the form according to the requested interaction.
Assigning one macro replaces that control's action; shortcut, open-app and shell-command
sections are alternative action types, not additional macro steps.
