use std::collections::VecDeque;

use super::*;

async fn scripted(executable: &Path, steps: &[(&str, Option<&str>)]) -> InstallationSource {
    let mut steps: VecDeque<_> = steps.iter().copied().collect();
    let result = detect_with(executable, async |command| {
        let (program, output) = steps.pop_front().expect("no unexpected probes");
        assert_eq!(command.as_std().get_program(), program);
        let args: Vec<_> = command.as_std().get_args().collect();
        if args.first() == Some(&std::ffi::OsStr::new("--show")) {
            assert_eq!(
                args,
                [
                    "--show",
                    "--showformat=${db:Status-Status}",
                    "--",
                    "mousekey-studio:amd64"
                ]
            );
        } else {
            assert_eq!(args.last().copied(), Some(executable.as_os_str()));
            let expected: &[&str] = match program {
                "dpkg-query" => &["--search", "--"],
                "rpm" => &["--query", "--file", "--queryformat", "%{NAME}", "--"],
                "pacman" => &["-Qqo", "--"],
                _ => panic!("unexpected package manager"),
            };
            assert_eq!(&args[..args.len() - 1], expected);
        }
        output.map(str::to_owned)
    })
    .await;
    assert!(steps.is_empty(), "all expected probes must run");
    result
}

#[tokio::test]
async fn nix_uses_resolved_store_boundary_and_skips_package_commands() {
    assert_eq!(
        scripted(
            Path::new("/nix/store/abc-mousekey-studio/bin/mousekey-studio-desktop"),
            &[]
        )
        .await,
        InstallationSource::Nix
    );
    assert_eq!(
        scripted(
            Path::new("/nix/store-backup/mousekey-studio-desktop"),
            &[("dpkg-query", None), ("rpm", None), ("pacman", None)]
        )
        .await,
        InstallationSource::Unknown
    );
}

#[tokio::test]
async fn deb_requires_exact_file_ownership_and_installed_status() {
    let executable = Path::new("/usr/bin/mousekey-studio-desktop");
    let owned = "mousekey-studio:amd64: /usr/bin/mousekey-studio-desktop";
    assert_eq!(
        scripted(
            executable,
            &[
                ("dpkg-query", Some(owned)),
                ("dpkg-query", Some("installed"))
            ]
        )
        .await,
        InstallationSource::LinuxPackage(LinuxPackage::Deb)
    );
    for status in [None, Some("config-files"), Some("unpacked")] {
        assert_eq!(
            scripted(
                executable,
                &[
                    ("dpkg-query", Some(owned)),
                    ("dpkg-query", status),
                    ("rpm", None),
                    ("pacman", None),
                ]
            )
            .await,
            InstallationSource::Unknown
        );
    }
    for output in [
        "mousekey-studio-extra: /usr/bin/mousekey-studio-desktop",
        "mousekey-studio: /usr/bin/mousekey-studio-desktop-old",
        "mousekey-studio:amd64, another: /usr/bin/mousekey-studio-desktop",
        "diversion by mousekey-studio from: /usr/bin/mousekey-studio-desktop",
    ] {
        assert_eq!(
            scripted(
                executable,
                &[
                    ("dpkg-query", Some(output)),
                    ("rpm", None),
                    ("pacman", None)
                ]
            )
            .await,
            InstallationSource::Unknown
        );
    }
}

#[tokio::test]
async fn rpm_and_arch_require_the_mousekey_studio_package_not_a_similar_name() {
    let executable = Path::new("/opt/custom path/mousekey-studio-desktop");
    assert_eq!(
        scripted(
            executable,
            &[("dpkg-query", None), ("rpm", Some("mousekey-studio"))]
        )
        .await,
        InstallationSource::LinuxPackage(LinuxPackage::Rpm)
    );
    assert_eq!(
        scripted(
            executable,
            &[
                ("dpkg-query", None),
                ("rpm", Some("other")),
                ("pacman", Some("mousekey-studio"))
            ]
        )
        .await,
        InstallationSource::LinuxPackage(LinuxPackage::Arch)
    );
    assert_eq!(
        scripted(
            executable,
            &[
                ("dpkg-query", None),
                ("rpm", Some("mousekey-studio-extra")),
                ("pacman", Some("other\nmousekey-studio"))
            ]
        )
        .await,
        InstallationSource::Unknown
    );
}

#[tokio::test]
async fn missing_query_tool_is_inconclusive() {
    let temp = tempfile::tempdir().unwrap();
    assert_eq!(query(Command::new(temp.path().join("missing"))).await, None);
}

#[cfg(unix)]
#[tokio::test]
async fn query_rejects_failure_and_times_out_instead_of_trusting_stdout() {
    let mut success = Command::new("/bin/sh");
    success.args(["-c", "printf 'mousekey-studio\\n'"]);
    assert_eq!(query(success).await.as_deref(), Some("mousekey-studio"));
    let mut failure = Command::new("/bin/sh");
    failure.args(["-c", "printf mousekey-studio; exit 1"]);
    assert_eq!(query(failure).await, None);
    let temp = tempfile::tempdir().unwrap();
    let pidfile = temp.path().join("pid");
    let mut stalled = Command::new("/bin/sh");
    stalled
        .args(["-c", "echo $$ > \"$1\"; exec /bin/sleep 10", "sh"])
        .arg(&pidfile);
    assert_eq!(query(stalled).await, None);
    #[cfg(target_os = "linux")]
    {
        let pid = std::fs::read_to_string(pidfile).unwrap();
        assert!(
            !Path::new("/proc").join(pid.trim()).exists(),
            "the timed-out query must be killed and reaped before returning"
        );
    }
}
