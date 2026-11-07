//! Platform and OS integration helpers.

#[cfg(windows)]
pub(crate) mod admin_autostart;
pub mod app_icon;
pub mod installation;
pub mod os;
pub mod registration;
pub mod updater;
