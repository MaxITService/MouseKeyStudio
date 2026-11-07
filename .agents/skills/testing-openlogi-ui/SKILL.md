---
name: testing-openlogi-ui
description: "Verifies OpenLogi native UI with focused GPUI tests, offscreen PNG snapshots of any UI state, the component gallery, and mock-agent workflows. Use when changing or testing desktop, overlay, shared controls, theme, scale, localization, or UI interactions."
---

# Test OpenLogi UI

Choose evidence that proves the affected behavior, then inspect the rendered result.

## Choose the test surface

1. Read [GUI integration rules](../../../.agents/rules/gui.md) and load
   [gpui-kit](../gpui-kit/SKILL.md). For visual or interaction changes, also load
   [gpui-kit-design-guides](../gpui-kit-design-guides/SKILL.md).
2. Name the expected result and a failing case before editing. Use pure tests for
   logic, GPUI tests for rendered interactions, and the running app for OS behavior.
3. Use the gallery for reusable controls; update the affected gallery entry.
   Use the mock agent for device panels and IPC-driven state without hardware.
   Test the overlay process separately; a desktop ring preview does not verify it.
4. Read [development setup](../../../docs/DEVELOPMENT.md) before launching.
   Check the available native display and build tools. Browser DOM automation does
   not test GPUI. If native rendering is blocked, report the blocker and run the
   strongest available tests instead.

## Run without hardware

Run from the repository root. These examples use POSIX environment syntax;
on Windows, set the same variables using the active shell's syntax.

For isolated controls in a debug build:

```sh
OPENLOGI_PROFILE=dev OPENLOGI_DEV_AGENT=0 OPENLOGI_COMPONENT_GALLERY=1 cargo run -p openlogi-desktop
```

For app workflows, start the mock first. After it binds its endpoint, start the
desktop in a second terminal or managed service:

```sh
OPENLOGI_PROFILE=dev cargo run -p openlogi-agent --bin openlogi-agent-mock
OPENLOGI_PROFILE=dev OPENLOGI_DEV_AGENT=0 cargo run -p openlogi-desktop
```

- Keep all test processes on the same profile. Do not use `prod` to bypass a lock.
  Identify a conflicting process before stopping it; preserve installed apps.
- On macOS, `cargo run` refreshes the dev bundle; `cargo build` alone does not.
  Quit the previous dev GUI before relaunching. `OPENLOGI_DEV_AGENT=0` prevents the
  runner from replacing the mock with real helpers. Linux/Windows use the raw binary.
- If macOS rejects an external production agent, confirm separate profiles before
  using `OPENLOGI_ALLOW_EXTERNAL_AGENT=1`; do not bypass a same-profile conflict.
- Confirm the connected agent has the `-mock` version suffix before changing settings.
  If it disconnects, restore the mock before continuing; the GUI can auto-start helpers.
- For repeatable captured inventory, append `-- --fixture <profile.json>` to the
  mock command. Fixture mode freezes mock time; demo mode animates battery/pairing.
  Do not wait for demo pairing transitions in fixture mode.

## Screenshot any UI state

The `ui-snapshots` feature turns a debug desktop build into an offscreen
renderer: it builds named scenes in a fixed state, renders them on the host's
real GPUI platform with the app's themes and assets, writes PNGs, and exits.
It needs no agent, hardware, or screen capture, and does not take focus. Use it
to inspect a visual change and iterate on it before running the app.

```sh
cargo build -p openlogi-desktop --features ui-snapshots
OPENLOGI_UI_SNAPSHOTS=macro ./target/debug/openlogi-desktop    # .exe on Windows
```

- `OPENLOGI_UI_SNAPSHOTS`: comma-separated substrings of scene names; `all`
  renders every scene. A filter that matches nothing lists the available scenes.
- `OPENLOGI_UI_SNAPSHOT_THEMES` (`light,dark` by default),
  `OPENLOGI_UI_SNAPSHOT_SCALES` (UI scale percents, `100` by default), and
  `OPENLOGI_UI_SNAPSHOT_DIR` (`target/ui-snapshots` by default).
- Output: `<scene>--<theme>--<scale>.png`, with `/` in the scene name replaced
  by `_`. The binary prints one `snapshot: <path>` line per file.
- Open the PNGs with your image-viewing tool. To check detail, crop the region
  under review into your scratch directory, and delete the crops afterwards.
- On Windows the capture windows are cloaked. On macOS and Linux they may
  flash on screen briefly.

Add a scene for every state you need to review. Put it in the owning module's
`snapshots.rs`, under `#[cfg(all(debug_assertions, feature = "ui-snapshots"))]`,
and register it in `scenes()` in
[`ui/snapshots.rs`](../../../crates/openlogi-desktop/src/ui/snapshots.rs).
A scene is a name (`area/state`), a logical window size, and a `build`
function. `build` drives the view into the state the way that module's tests do.
[`features/mouse/snapshots.rs`](../../../crates/openlogi-desktop/src/features/mouse/snapshots.rs)
is the worked example: a picker that is open, a seeded macro, an overflowing
step list, an error, and a narrow layout.

- State comes from an in-memory `AppState` (an ephemeral config, no agent, no
  devices). Seed anything else a panel reads inside the scene.
- The locale is fixed to `en`.
- A snapshot shows one idle frame. Hover, focus, and recent-scroll effects do
  not appear: tooltips, hover highlights, and scrollbars that hide on idle.
  Show those states in the scene, or assert them in a GPUI test.
- Cover the risky shapes of each change: empty, short, overflowing, error, and
  narrow, in both themes. Check every affected scale.
- Snapshots support the GPUI interaction test; they do not replace it. Run
  `cargo clippy -p openlogi-desktop --all-targets --features ui-snapshots -- -D warnings`
  when you touch scene code.

## Verify the affected contract

Use the installed GPUI API, not unverified upstream test helpers. The
[gallery smoke test](../../../crates/openlogi-desktop/src/ui/gallery.rs),
[picker interaction test](../../../crates/openlogi-desktop/src/features/profiles/picker.rs),
and [ring scroll test](../../../crates/openlogi-desktop/src/features/action_ring/editor.rs)
show working local harnesses. Run the affected test, for example:

```sh
cargo test -p openlogi-desktop gallery_renders_without_application_state
```

- Assert an interaction's visible result and owner state, not just successful render.
  Exercise the relevant negative state: disabled control, offline device, invalid
  input, empty list, or IPC failure. Do not manufacture test-only state to pass.
- For visible changes, render representative affected states, capture screenshots
  (see [Screenshot any UI state](#screenshot-any-ui-state)), and inspect them. Check affected light/dark themes, supported interface scales,
  narrow layouts, and open menus or dialogs. A smoke test is not pixel verification.
- For localized UI, change language while the view is open and inspect cached text.
  Apply the catalog and wiring checks in [i18n rules](../../../.agents/rules/i18n.md).
- Apply [change verification](../verifying-openlogi-changes/SKILL.md) once stable.
  Desktop tests run on macOS in CI; a Linux CI pass does not cover those tests.

Report commands and results, exercised states, and an inspected screenshot for
visual work. Separate in-process tests, running-app checks, and real-hardware checks.
Mock success does not prove HID writes, OS permissions, input hooks, or physical
device behavior. Name every relevant OS or hardware check that was not run.
