//! Pointer placement and non-destructive write probes, shared by all processes.

use super::{Bootstrap, PathsError, Preference, Profile, location_error, read_next};
use std::{io::Write as _, path::Path};

/// Where a configuration-directory pointer is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigPointerLocation {
    /// A file in the current user's configuration directory.
    UserProfile,
    /// A file beside the portable GUI, agent and overlay executables.
    PortableFolder,
}

/// A pointer destination and the result of a non-destructive write probe.
#[derive(Clone, Debug)]
pub struct ConfigPointerChoice {
    location: ConfigPointerLocation,
    path: std::path::PathBuf,
    selected: bool,
    write_error: Option<String>,
}

impl ConfigPointerChoice {
    /// Destination represented by this option.
    #[must_use]
    pub const fn location(&self) -> ConfigPointerLocation {
        self.location
    }

    /// Full path of the pointer file, including its filename.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether this is the currently selected pointer destination.
    #[must_use]
    pub const fn is_selected(&self) -> bool {
        self.selected
    }

    /// Why the probe failed, or `None` after a successful write-and-discard probe.
    /// Access can change afterwards, so the eventual move still handles errors.
    #[must_use]
    pub fn write_error(&self) -> Option<&str> {
        self.write_error.as_deref()
    }
}

/// Inspect the pointer destinations and probe atomic writes without replacing
/// either pointer. Run on a worker: this performs filesystem I/O.
pub fn config_pointer_choices() -> Result<Vec<ConfigPointerChoice>, PathsError> {
    choices(&Bootstrap::for_profile(Profile::current())?)
}

fn choices(bootstrap: &Bootstrap) -> Result<Vec<ConfigPointerChoice>, PathsError> {
    let selected = bootstrap.selected()?;
    let mut locations = vec![ConfigPointerLocation::UserProfile];
    if matches!(bootstrap, Bootstrap::Portable { .. }) {
        locations.push(ConfigPointerLocation::PortableFolder);
    }
    locations
        .into_iter()
        .map(|location| {
            let path = bootstrap.marker_at(location)?;
            let write_error = probe(&path).err().map(|error| error.to_string());
            Ok(ConfigPointerChoice {
                location,
                path,
                selected: selected == location,
                write_error,
            })
        })
        .collect()
}

fn probe(marker: &Path) -> std::io::Result<()> {
    match marker.metadata() {
        Ok(metadata) if !metadata.is_file() || metadata.permissions().readonly() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "the pointer is read-only or is not a regular file",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        // GENERIC_WRITE | DELETE checks the existing destination's ACL and
        // sharing locks without writing it. Atomic replacement needs deletion.
        match std::fs::OpenOptions::new()
            .access_mode(0x4000_0000 | 0x0001_0000)
            .share_mode(7)
            .open(marker)
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let parent = marker.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "pointer has no parent folder",
        )
    })?;
    std::fs::create_dir_all(parent)?;
    let mut staging = atomic_write_file::AtomicWriteFile::open(marker)?;
    staging.write_all(b"configuration pointer write probe\n")?;
    staging.as_file().sync_all()?;
    // Discard explicitly so a cleanup failure is reported, not hidden by Drop.
    staging.discard()
}

/// Move the pointer, preserving both the current and pending settings folders.
/// Profiles and macros are never copied, deleted or overwritten. Portable mode
/// must already be declared by the marker shipped beside the executables.
pub fn set_config_pointer_location(location: ConfigPointerLocation) -> Result<(), PathsError> {
    let _guard = super::lock_writes()?;
    relocate(&Bootstrap::for_profile(Profile::current())?, location)
}

fn relocate(bootstrap: &Bootstrap, location: ConfigPointerLocation) -> Result<(), PathsError> {
    if bootstrap.selected()? == location {
        return Ok(());
    }
    let directory = read_next(bootstrap)?;
    let destination = bootstrap.marker_at(location)?;
    probe(&destination).map_err(location_error)?;
    let text = toml::to_string(&Preference {
        directory: bootstrap.stored_path(&directory),
    })
    .map_err(location_error)?;
    crate::config::write_atomic(&destination, text.as_bytes()).map_err(location_error)?;
    if location == ConfigPointerLocation::PortableFolder
        && let Bootstrap::Portable { user_marker, .. } = bootstrap
    {
        // The user pointer has priority. Remove it only after the destination
        // is durable. If removal fails, it remains the active pointer and the
        // caller receives the error; the settings selection is unchanged.
        std::fs::remove_file(user_marker).map_err(location_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
