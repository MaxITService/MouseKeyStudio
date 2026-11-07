//! A bootstrap file locates config.toml; portable paths travel with the binaries.

use super::{PathsError, Profile, xdg_config_home};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
};

mod pointer;
pub use pointer::{
    ConfigPointerChoice, ConfigPointerLocation, config_pointer_choices, set_config_pointer_location,
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Preference {
    directory: PathBuf,
}

const PORTABLE_MARKER: &str = "mousekey-studio-portable.toml";
static POINTER_WRITES: Mutex<()> = Mutex::new(());

/// Whether an executable belongs to an explicitly marked portable folder.
/// A damaged pointer still identifies a portable distribution: never offer an
/// MSI update as a recovery path for it. Does filesystem I/O; call off the UI thread.
pub fn is_portable_executable(executable: &Path) -> Result<bool, PathsError> {
    executable.parent().map_or(Ok(false), |directory| {
        directory
            .join(PORTABLE_MARKER)
            .try_exists()
            .map_err(location_error)
    })
}

fn lock_writes() -> Result<MutexGuard<'static, ()>, PathsError> {
    POINTER_WRITES.lock().map_err(location_error)
}

#[derive(Clone)]
enum Bootstrap {
    User(PathBuf),
    Portable { root: PathBuf, user_marker: PathBuf },
}

impl Bootstrap {
    fn for_profile(profile: Profile) -> Result<Self, PathsError> {
        static PRODUCTION: OnceLock<Bootstrap> = OnceLock::new();
        static DEV: OnceLock<Bootstrap> = OnceLock::new();
        let cell = match profile {
            Profile::Production => &PRODUCTION,
            Profile::Dev => &DEV,
        };
        if let Some(bootstrap) = cell.get() {
            return Ok(bootstrap.clone());
        }
        let user = xdg_config_home()?.join(profile.app_dir());
        if profile == Profile::Production {
            let executable = std::env::current_exe().map_err(location_error)?;
            if let Some(portable) = Self::beside(&executable, &user)? {
                return Ok(cell.get_or_init(|| portable).clone());
            }
        }
        Ok(cell.get_or_init(|| Self::User(user)).clone())
    }

    fn beside(executable: &Path, user: &Path) -> Result<Option<Self>, PathsError> {
        let Some(directory) = executable.parent() else {
            return Ok(None);
        };
        if !is_portable_executable(executable)? {
            return Ok(None);
        }
        let root = directory.canonicalize().map_err(location_error)?;
        // A user-profile choice belongs to this portable copy, never to all
        // copies or to the installed edition. All sibling processes use it.
        let identity = format!("{:x}", Sha256::digest(root.as_os_str().as_encoded_bytes()));
        Ok(Some(Self::Portable {
            root,
            user_marker: user
                .join("portable-locations")
                .join(format!("{identity}.toml")),
        }))
    }

    fn marker_at(&self, location: ConfigPointerLocation) -> Result<PathBuf, PathsError> {
        match (self, location) {
            (Self::User(directory), ConfigPointerLocation::UserProfile) => {
                Ok(directory.join("config-location.toml"))
            }
            (Self::Portable { user_marker, .. }, ConfigPointerLocation::UserProfile) => {
                Ok(user_marker.clone())
            }
            (Self::Portable { root, .. }, ConfigPointerLocation::PortableFolder) => {
                Ok(root.join(PORTABLE_MARKER))
            }
            (Self::User(_), ConfigPointerLocation::PortableFolder) => {
                Err(location_error("this is not a portable distribution"))
            }
        }
    }

    fn selected(&self) -> Result<ConfigPointerLocation, PathsError> {
        match self {
            Self::User(_) => Ok(ConfigPointerLocation::UserProfile),
            Self::Portable { user_marker, .. } => {
                if user_marker.try_exists().map_err(location_error)? {
                    Ok(ConfigPointerLocation::UserProfile)
                } else {
                    Ok(ConfigPointerLocation::PortableFolder)
                }
            }
        }
    }

    fn marker(&self) -> Result<PathBuf, PathsError> {
        self.marker_at(self.selected()?)
    }

    fn resolve(&self, directory: PathBuf) -> Result<PathBuf, PathsError> {
        if directory.is_absolute() {
            return Ok(directory);
        }
        if let Self::Portable { root, .. } = self
            && is_portable_relative(&directory)
        {
            return Ok(root.join(directory));
        }
        Err(PathsError::ConfigLocation(
            "use an absolute directory, or a path inside the portable folder".into(),
        ))
    }

    fn stored_path(&self, selected: &Path) -> PathBuf {
        if let Self::Portable { root, .. } = self {
            if let Ok(relative) = selected.strip_prefix(root)
                && is_portable_relative(&relative_path(relative))
            {
                return relative_path(relative);
            }
            // Canonical paths also handle different drive-letter/path casing
            // returned by the Windows folder picker. External folders stay absolute.
            if let Ok(selected_real) = selected.canonicalize()
                && let Ok(relative) = selected_real.strip_prefix(root)
            {
                return relative_path(relative);
            }
        }
        selected.to_path_buf()
    }
}

fn is_portable_relative(directory: &Path) -> bool {
    !directory.as_os_str().is_empty()
        && directory.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

fn relative_path(relative: &Path) -> PathBuf {
    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative.to_path_buf()
    }
}

fn read_next(bootstrap: &Bootstrap) -> Result<PathBuf, PathsError> {
    match std::fs::read_to_string(bootstrap.marker()?) {
        Ok(text) => {
            let preference: Preference = toml::from_str(&text).map_err(location_error)?;
            bootstrap.resolve(preference.directory)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match bootstrap {
            Bootstrap::User(default) => Ok(default.clone()),
            Bootstrap::Portable { .. } => Err(location_error(error)),
        },
        Err(error) => Err(location_error(error)),
    }
}

fn location_error(error: impl std::fmt::Display) -> PathsError {
    PathsError::ConfigLocation(error.to_string())
}

pub(super) fn active(profile: Profile) -> Result<PathBuf, PathsError> {
    static PRODUCTION: OnceLock<PathBuf> = OnceLock::new();
    static DEV: OnceLock<PathBuf> = OnceLock::new();
    let cell = match profile {
        Profile::Production => &PRODUCTION,
        Profile::Dev => &DEV,
    };
    if let Some(path) = cell.get() {
        return Ok(path.clone());
    }
    resolve_active(&Bootstrap::for_profile(profile)?, cell)
}

fn resolve_active(bootstrap: &Bootstrap, cell: &OnceLock<PathBuf>) -> Result<PathBuf, PathsError> {
    if let Some(path) = cell.get() {
        return Ok(path.clone());
    }
    let selected = read_next(bootstrap)?;
    Ok(cell.get_or_init(|| selected).clone())
}

/// Directory that will be used after restarting the GUI and background agent.
/// Portable distributions keep their marker beside the executables; other
/// installations keep it under the default XDG configuration root.
pub fn next_config_dir() -> Result<PathBuf, PathsError> {
    read_next(&Bootstrap::for_profile(Profile::current())?)
}

/// Remember an existing absolute directory for the next full app restart.
/// Existing configuration files are neither moved nor overwritten.
/// Portable distributions store folders inside their bundle as relative paths.
pub fn set_next_config_dir(directory: &Path) -> Result<(), PathsError> {
    let _guard = lock_writes()?;
    save_next(&Bootstrap::for_profile(Profile::current())?, directory)
}

fn save_next(bootstrap: &Bootstrap, directory: &Path) -> Result<(), PathsError> {
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(PathsError::ConfigLocation(
            "choose an existing absolute directory".into(),
        ));
    }
    let marker = bootstrap.marker()?;
    if let Bootstrap::User(default) = bootstrap {
        std::fs::create_dir_all(default).map_err(location_error)?;
    }
    let text = toml::to_string(&Preference {
        directory: bootstrap.stored_path(directory),
    })
    .map_err(location_error)?;
    crate::config::write_atomic(&marker, text.as_bytes()).map_err(location_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_changes_only_after_restart_and_preserves_both_configs() {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("original");
        let selected = temp.path().join("selected folder");
        std::fs::create_dir_all(&original).unwrap();
        std::fs::create_dir_all(&selected).unwrap();
        std::fs::write(original.join("config.toml"), "original").unwrap();
        std::fs::write(selected.join("config.toml"), "selected").unwrap();
        let running = OnceLock::new();
        assert_eq!(
            resolve_active(&Bootstrap::User(original.clone()), &running).unwrap(),
            original
        );
        save_next(&Bootstrap::User(original.clone()), &selected).unwrap();
        assert_eq!(
            read_next(&Bootstrap::User(original.clone())).unwrap(),
            selected
        );
        assert_eq!(
            resolve_active(&Bootstrap::User(original.clone()), &running).unwrap(),
            original
        );
        assert_eq!(
            resolve_active(&Bootstrap::User(original.clone()), &OnceLock::new()).unwrap(),
            selected
        );
        assert_eq!(
            std::fs::read_to_string(original.join("config.toml")).unwrap(),
            "original"
        );
        assert_eq!(
            std::fs::read_to_string(selected.join("config.toml")).unwrap(),
            "selected"
        );
        save_next(&Bootstrap::User(original.clone()), &original).unwrap();
        assert_eq!(
            read_next(&Bootstrap::User(original.clone())).unwrap(),
            original
        );
    }

    #[test]
    fn relative_missing_and_malformed_locations_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        assert!(
            save_next(
                &Bootstrap::User(temp.path().to_path_buf()),
                Path::new("relative")
            )
            .is_err()
        );
        assert!(
            save_next(
                &Bootstrap::User(temp.path().to_path_buf()),
                &temp.path().join("missing")
            )
            .is_err()
        );
        std::fs::write(temp.path().join("config-location.toml"), "invalid TOML").unwrap();
        assert!(
            read_next(&Bootstrap::User(temp.path().to_path_buf())).is_err(),
            "never silently load empty defaults after a broken marker"
        );
        std::fs::write(
            temp.path().join("config-location.toml"),
            "directory = 'relative'",
        )
        .unwrap();
        read_next(&Bootstrap::User(temp.path().to_path_buf())).unwrap_err();
    }

    #[test]
    fn failed_marker_write_preserves_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("original");
        let selected = temp.path().join("selected");
        std::fs::create_dir_all(original.join("config-location.toml")).unwrap();
        std::fs::create_dir_all(&selected).unwrap();
        std::fs::write(original.join("config.toml"), "original").unwrap();
        assert!(save_next(&Bootstrap::User(original.clone()), &selected).is_err());
        assert_eq!(
            std::fs::read_to_string(original.join("config.toml")).unwrap(),
            "original"
        );
        assert!(!selected.join("config.toml").exists());
    }
}
