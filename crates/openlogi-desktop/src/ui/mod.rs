//! Shared settings-app UI components and theme.

pub(crate) mod action;
pub(crate) mod battery;
pub(crate) mod callouts;
pub(crate) mod carousel;
pub(crate) mod choice_card;
pub(crate) mod commit_slider;
pub(crate) mod components;
#[cfg(debug_assertions)]
pub(crate) mod gallery;
pub(crate) mod scroll;
pub mod section;
#[cfg(all(debug_assertions, feature = "ui-snapshots"))]
pub(crate) mod snapshots;
pub mod spacing;
pub mod status;
pub mod theme;
