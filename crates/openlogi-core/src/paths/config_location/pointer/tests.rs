use super::*;
use crate::paths::config_location::{PORTABLE_MARKER, resolve_active, save_next};
use std::{path::PathBuf, sync::OnceLock};

fn portable(root: &Path, user: &Path) -> Bootstrap {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join(PORTABLE_MARKER), "directory = 'settings'\n").unwrap();
    Bootstrap::beside(&root.join("MouseKeyStudio.exe"), user)
        .unwrap()
        .unwrap()
}

#[test]
fn unpacked_bundle_uses_settings_subfolder_for_every_sibling_process() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Portable app");
    let user = temp.path().join("user");
    let bootstrap = portable(&root, &user);
    let expected = root.canonicalize().unwrap().join("settings");
    assert_eq!(read_next(&bootstrap).unwrap(), expected);
    assert!(!expected.exists(), "first-run folder need not exist yet");
    for name in [
        "MouseKeyStudio.exe",
        "mks-agent.exe",
        "mks-overlay.exe",
        "mks.exe",
    ] {
        let sibling = Bootstrap::beside(&root.join(name), &user).unwrap().unwrap();
        assert_eq!(read_next(&sibling).unwrap(), expected);
    }
    let (config, mut file) =
        crate::config::ConfigFile::load_from_path(&expected.join("config.toml")).unwrap();
    file.save(&config).unwrap();
    assert!(expected.join("config.toml").is_file());
    assert!(
        !user.exists(),
        "first launch must not create a user pointer"
    );
}

#[test]
fn portable_pointer_and_settings_survive_moving_the_whole_folder() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("before move");
    let user = temp.path().join("user");
    let bootstrap = portable(&root, &user);
    let settings = read_next(&bootstrap).unwrap();
    std::fs::create_dir_all(&settings).unwrap();
    std::fs::write(settings.join("config.toml"), "private macro sentinel").unwrap();
    let moved = temp.path().join("another drive equivalent");
    std::fs::rename(&root, &moved).unwrap();
    let restarted = Bootstrap::beside(&moved.join("MouseKeyStudio.exe"), &user)
        .unwrap()
        .unwrap();
    let selected = read_next(&restarted).unwrap();
    assert_eq!(selected, moved.canonicalize().unwrap().join("settings"));
    assert_eq!(
        std::fs::read_to_string(selected.join("config.toml")).unwrap(),
        "private macro sentinel"
    );
}

#[test]
fn choosing_local_and_external_settings_preserves_files_and_restart_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("bundle");
    let bootstrap = portable(&root, &temp.path().join("user"));
    let running = OnceLock::new();
    let original = resolve_active(&bootstrap, &running).unwrap();
    let selected = root.canonicalize().unwrap().join("my settings");
    std::fs::create_dir_all(&selected).unwrap();
    std::fs::write(selected.join("config.toml"), "existing").unwrap();
    save_next(&bootstrap, &selected).unwrap();
    let stored: Preference =
        toml::from_str(&std::fs::read_to_string(bootstrap.marker().unwrap()).unwrap()).unwrap();
    assert_eq!(stored.directory, PathBuf::from("my settings"));
    assert_eq!(resolve_active(&bootstrap, &running).unwrap(), original);
    assert_eq!(
        resolve_active(&bootstrap, &OnceLock::new()).unwrap(),
        selected
    );
    let external = temp.path().join("external");
    std::fs::create_dir_all(&external).unwrap();
    save_next(&bootstrap, &external).unwrap();
    let stored: Preference =
        toml::from_str(&std::fs::read_to_string(bootstrap.marker().unwrap()).unwrap()).unwrap();
    assert!(stored.directory.is_absolute());
    assert_eq!(read_next(&bootstrap).unwrap(), external);
    assert_eq!(
        std::fs::read_to_string(selected.join("config.toml")).unwrap(),
        "existing"
    );
}

#[test]
fn moving_pointer_both_ways_keeps_pending_and_active_settings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("bundle");
    let bootstrap = portable(&root, &temp.path().join("user"));
    let running = OnceLock::new();
    let original = resolve_active(&bootstrap, &running).unwrap();
    let pending = root.canonicalize().unwrap().join("other settings");
    std::fs::create_dir_all(&pending).unwrap();
    std::fs::write(pending.join("config.toml"), "never move this file").unwrap();
    save_next(&bootstrap, &pending).unwrap();
    for target in [
        ConfigPointerLocation::UserProfile,
        ConfigPointerLocation::PortableFolder,
    ] {
        relocate(&bootstrap, target).unwrap();
        assert_eq!(bootstrap.selected().unwrap(), target);
        assert_eq!(read_next(&bootstrap).unwrap(), pending);
        assert_eq!(resolve_active(&bootstrap, &running).unwrap(), original);
        assert_eq!(
            std::fs::read_to_string(pending.join("config.toml")).unwrap(),
            "never move this file"
        );
    }
    assert!(
        !bootstrap
            .marker_at(ConfigPointerLocation::UserProfile)
            .unwrap()
            .exists()
    );
}

#[test]
fn pointer_can_move_before_the_first_settings_save() {
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    let original = read_next(&bootstrap).unwrap();
    relocate(&bootstrap, ConfigPointerLocation::UserProfile).unwrap();
    assert_eq!(read_next(&bootstrap).unwrap(), original);
    relocate(&bootstrap, ConfigPointerLocation::PortableFolder).unwrap();
    assert_eq!(read_next(&bootstrap).unwrap(), original);
    assert!(!original.exists());
}

#[test]
fn user_pointer_is_scoped_to_one_portable_copy_and_does_not_touch_installed_pointer() {
    let temp = tempfile::tempdir().unwrap();
    let user = temp.path().join("user");
    let first = portable(&temp.path().join("first"), &user);
    let second = portable(&temp.path().join("second"), &user);
    std::fs::create_dir_all(&user).unwrap();
    std::fs::write(
        user.join("config-location.toml"),
        "installed pointer sentinel",
    )
    .unwrap();
    relocate(&first, ConfigPointerLocation::UserProfile).unwrap();
    assert_eq!(
        first.selected().unwrap(),
        ConfigPointerLocation::UserProfile
    );
    assert_eq!(
        second.selected().unwrap(),
        ConfigPointerLocation::PortableFolder
    );
    assert_ne!(
        first.marker_at(ConfigPointerLocation::UserProfile).unwrap(),
        second
            .marker_at(ConfigPointerLocation::UserProfile)
            .unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(user.join("config-location.toml")).unwrap(),
        "installed pointer sentinel"
    );
}

#[test]
fn probes_never_change_pointer_contents_or_create_a_pointer() {
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    let local = bootstrap.marker().unwrap();
    let before = std::fs::read(&local).unwrap();
    let options = choices(&bootstrap).unwrap();
    assert_eq!(options.len(), 2);
    assert!(options.iter().all(|option| option.write_error().is_none()));
    assert!(
        options
            .iter()
            .find(|option| option.location() == ConfigPointerLocation::PortableFolder)
            .unwrap()
            .is_selected()
    );
    assert_eq!(std::fs::read(local).unwrap(), before);
    let user = bootstrap
        .marker_at(ConfigPointerLocation::UserProfile)
        .unwrap();
    assert!(!user.exists());
    assert_eq!(
        std::fs::read_dir(user.parent().unwrap()).unwrap().count(),
        0,
        "probe leaves no temporary files"
    );
}

#[test]
fn denied_destination_is_visible_and_a_failed_move_preserves_selection() {
    let temp = tempfile::tempdir().unwrap();
    let user = temp.path().join("user");
    let bootstrap = portable(&temp.path().join("bundle"), &user);
    let original = read_next(&bootstrap).unwrap();
    std::fs::write(&user, "a file blocks the destination directory").unwrap();
    let options = choices(&bootstrap).unwrap();
    assert!(
        options
            .iter()
            .find(|option| option.location() == ConfigPointerLocation::UserProfile)
            .unwrap()
            .write_error()
            .is_some()
    );
    assert!(
        relocate(&bootstrap, ConfigPointerLocation::UserProfile).is_err(),
        "failed write must reject move"
    );
    assert_eq!(
        bootstrap.selected().unwrap(),
        ConfigPointerLocation::PortableFolder
    );
    assert_eq!(read_next(&bootstrap).unwrap(), original);
}

#[test]
fn readonly_portable_pointer_can_still_move_to_user_profile() {
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    let marker = bootstrap.marker().unwrap();
    let original_permissions = marker.metadata().unwrap().permissions();
    let mut readonly = original_permissions.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&marker, readonly).unwrap();
    let options = choices(&bootstrap).unwrap();
    let moved = relocate(&bootstrap, ConfigPointerLocation::UserProfile);
    let refused = relocate(&bootstrap, ConfigPointerLocation::PortableFolder);
    std::fs::set_permissions(&marker, original_permissions).unwrap();
    moved.unwrap();
    assert!(
        refused.is_err(),
        "readonly destination must remain unavailable"
    );
    assert!(
        options
            .iter()
            .find(|option| option.location() == ConfigPointerLocation::PortableFolder)
            .unwrap()
            .write_error()
            .is_some()
    );
    assert_eq!(
        bootstrap.selected().unwrap(),
        ConfigPointerLocation::UserProfile
    );
}

#[test]
fn malformed_or_missing_selected_pointer_never_falls_back_to_empty_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    let local = bootstrap.marker().unwrap();
    for contents in [
        "invalid TOML",
        "directory = ''",
        "directory = '../elsewhere'",
        "directory = 'settings'\nunknown = 1",
    ] {
        std::fs::write(&local, contents).unwrap();
        assert!(
            read_next(&bootstrap).is_err(),
            "reject invalid marker: {contents}"
        );
    }
    std::fs::remove_file(local).unwrap();
    assert!(
        read_next(&bootstrap).is_err(),
        "never redirect a running portable context after marker removal"
    );
}

#[test]
fn installed_edition_has_only_the_user_pointer_and_no_implicit_portable_mode() {
    let temp = tempfile::tempdir().unwrap();
    assert!(
        Bootstrap::beside(&temp.path().join("MouseKeyStudio.exe"), temp.path())
            .unwrap()
            .is_none()
    );
    let bootstrap = Bootstrap::User(temp.path().join("installed"));
    let options = choices(&bootstrap).unwrap();
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].location(), ConfigPointerLocation::UserProfile);
    assert!(
        relocate(&bootstrap, ConfigPointerLocation::PortableFolder).is_err(),
        "installed app cannot opt into portable storage"
    );
}

#[test]
fn absolute_parent_segments_are_not_saved_as_invalid_relative_paths() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("bundle");
    let bootstrap = portable(&root, &temp.path().join("user"));
    let external = root.canonicalize().unwrap().join("..").join("external");
    std::fs::create_dir_all(&external).unwrap();
    save_next(&bootstrap, &external).unwrap();
    assert!(read_next(&bootstrap).unwrap().is_absolute());
    let stored: Preference =
        toml::from_str(&std::fs::read_to_string(bootstrap.marker().unwrap()).unwrap()).unwrap();
    assert!(
        stored.directory.is_absolute(),
        "an external path containing .. must not become relative"
    );
}

#[test]
fn moving_pointer_preserves_an_external_settings_path_that_is_offline() {
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    let offline = temp.path().join("unavailable drive");
    let text = toml::to_string(&Preference {
        directory: offline.clone(),
    })
    .unwrap();
    std::fs::write(bootstrap.marker().unwrap(), text).unwrap();
    relocate(&bootstrap, ConfigPointerLocation::UserProfile).unwrap();
    assert_eq!(read_next(&bootstrap).unwrap(), offline);
    assert!(
        !offline.exists(),
        "moving a pointer must not create its target"
    );
}

#[test]
fn invalid_user_pointer_does_not_fall_back_to_bundled_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    relocate(&bootstrap, ConfigPointerLocation::UserProfile).unwrap();
    std::fs::write(bootstrap.marker().unwrap(), "broken TOML").unwrap();
    assert!(
        read_next(&bootstrap).is_err(),
        "never hide a corrupt selected pointer by loading defaults"
    );
    assert!(
        relocate(&bootstrap, ConfigPointerLocation::PortableFolder).is_err(),
        "never discard a broken selected pointer automatically"
    );
}

#[test]
#[cfg(target_os = "windows")]
fn write_probe_detects_an_existing_pointer_locked_against_replacement() {
    use std::os::windows::fs::OpenOptionsExt as _;
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    let local = bootstrap.marker().unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(local)
        .unwrap();
    let options = choices(&bootstrap).unwrap();
    drop(lock);
    assert!(
        options
            .iter()
            .find(|option| option.location() == ConfigPointerLocation::PortableFolder)
            .unwrap()
            .write_error()
            .is_some()
    );
}

#[test]
#[cfg(target_os = "windows")]
fn drive_relative_and_rooted_paths_are_not_accepted_as_portable_relative_paths() {
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    for directory in [r"C:settings", r"\settings"] {
        std::fs::write(
            bootstrap.marker().unwrap(),
            format!("directory = '{directory}'"),
        )
        .unwrap();
        assert!(
            read_next(&bootstrap).is_err(),
            "reject drive-relative or root-relative path {directory}"
        );
    }
}

#[test]
#[cfg(target_os = "windows")]
fn failed_source_removal_keeps_the_user_pointer_authoritative() {
    use std::os::windows::fs::OpenOptionsExt as _;
    let temp = tempfile::tempdir().unwrap();
    let bootstrap = portable(&temp.path().join("bundle"), &temp.path().join("user"));
    relocate(&bootstrap, ConfigPointerLocation::UserProfile).unwrap();
    let user = bootstrap.marker().unwrap();
    let original = read_next(&bootstrap).unwrap();
    // Allow reads but deny deletion, without changing ACLs or using elevation.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&user)
        .unwrap();
    let result = relocate(&bootstrap, ConfigPointerLocation::PortableFolder);
    drop(lock);
    assert!(result.is_err(), "failed source deletion must be reported");
    assert_eq!(
        bootstrap.selected().unwrap(),
        ConfigPointerLocation::UserProfile
    );
    assert_eq!(read_next(&bootstrap).unwrap(), original);
}
