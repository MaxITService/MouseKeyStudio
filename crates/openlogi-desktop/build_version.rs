//! A persistent local revision shared by debug and release builds.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash as _, Hasher as _};
use std::io;
use std::path::Path;
use std::{env, fs};

pub fn generate() -> (String, u16) {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).join("../..");
    let mut hash = DefaultHasher::new();
    for input in [
        "crates",
        "assets/plus",
        "vendor/gpui-pre",
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/config.toml",
    ] {
        println!("cargo:rerun-if-changed=../../{input}");
        fingerprint(&root.join(input), &root, &mut hash).expect("fingerprint build sources");
    }
    let fingerprint = format!("{:016x}", hash.finish());
    // Outside target/: cleaning Cargo artifacts must not reset the revision.
    // This generated record is deliberately excluded from the fingerprint.
    let record = root.join("custom-build-version.txt");
    let previous = match fs::read_to_string(&record) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => panic!("read custom build revision: {error}"),
    };
    let (revision, changed) = match previous.as_deref() {
        Some(text) => {
            let (number, saved) = text
                .trim()
                .split_once(' ')
                .expect("valid build revision record");
            let number: u16 = number.parse().expect("numeric build revision");
            if saved == fingerprint {
                (number, false)
            } else {
                (
                    number
                        .checked_add(1)
                        .expect("build revision fits Windows resources"),
                    true,
                )
            }
        }
        None => (1, true),
    };
    if changed {
        fs::write(record, format!("{revision} {fingerprint}\n"))
            .expect("persist custom build revision");
    }
    // The visible version is the plain package version; the local revision
    // only fills the fourth Windows VERSIONINFO field so rebuilds stay distinct.
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    println!("cargo:rustc-env=MKS_BUILD_VERSION={version}");
    (version, revision)
}

fn fingerprint(path: &Path, root: &Path, hash: &mut DefaultHasher) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_symlink() {
        return Ok(());
    }
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            fingerprint(&entry.path(), root, hash)?;
        }
    } else if metadata.is_file() {
        path.strip_prefix(root)
            .expect("source inside workspace")
            .hash(hash);
        fs::read(path)?.hash(hash);
    }
    Ok(())
}
