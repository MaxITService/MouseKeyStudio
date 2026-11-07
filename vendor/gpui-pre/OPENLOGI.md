# OpenLogi GPUI scroll patch

Source: the unmodified crates.io `gpui-pre` **0.3.4** source archive, based on
zed-industries/zed `69164008341295ad481bb11c0334a712ca8c23e3`.
The original Apache-2.0 license, normalized manifest, and source are retained.

Local changes:

- `src/window/wheel.rs` and `src/window/wheel/motion.rs`: optional application-wide wheel animation, scheduled on
  native frames. Fractional **line** deltas preserve each component's line height;
  pixel/trackpad events stay direct. New input cancels stale motion, and reversing
  an axis discards its old pending distance.
- `src/window.rs`: dispatch through that owner before component event handlers.
- `src/elements/div.rs`: clamp every wheel event to measured limits and consume
  only events that move this region. This also covers uniform lists and enables
  nested scrolling to pass to its parent at an edge.
- `tests/openlogi_wheel.rs` and its manifest entry: exercise the production motion
  owner without upstream unit tests that require fonts absent from the archive.

OpenLogi opts in through `ui::scroll::init`. The same dispatch serves third-party
menus, dialogs, inputs, native divs, virtual lists, and both scroll axes. There is
no second animation implementation in application components.

When updating GPUI, diff these files against the exact published source, port
only these changes, then run the desktop scroll and interaction regression tests
and native snapshot review. Remove the patch when upstream provides equivalent
behavior. Keep the core and platform version aligned and one GPUI package identity.
