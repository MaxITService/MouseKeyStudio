//! Mouse diagram, hotspots, labels, and binding pickers.

mod force_sensing;
pub mod geometry;
mod hotspots;
mod inspector;
pub mod leader_lines;
mod macro_editor;
pub(crate) mod macro_guide;
mod macro_presets;
mod macro_recorder;
mod macro_text;
mod macro_validation;
#[cfg(all(debug_assertions, feature = "ui-snapshots"))]
pub(crate) mod snapshots;
pub(crate) mod thumbwheel;
pub mod view;
