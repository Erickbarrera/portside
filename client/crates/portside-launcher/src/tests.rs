use std::fs;

use portside_api::models::{ArchiveKind, ManifestBuild};
use portside_core::types::Arch;
use portside_installer::{BuildFit, Verification};

use super::*;

fn installed(dir: &Path, os: Os, launch: LaunchSpec) -> Installed {
    Installed {
        port_id: "example".to_owned(),
        title: "Example Port".to_owned(),
        version: "v1".to_owned(),
        dir: dir.display().to_string(),
        os,
        arch: Arch::X86_64,
        fit: BuildFit::Native,
        asset: "example.zip".to_owned(),
        release_url: "https://github.com/example/example/releases/tag/v1".to_owned(),
        size: 1,
        sha256: None,
        verified: Verification::None,
        installed_at: 0,
        unsigned: false,
        build: ManifestBuild {
            os,
            arch: Arch::X86_64,
            asset: "example.zip".to_owned(),
            archive: ArchiveKind::Zip,
            nested: None,
            strip_root: None,
            launch: Some(launch),
            checksum: None,
            unsigned: None,
            android_package: None,
            url: None,
            sha256: None,
            size: None,
        },
    }
}

fn exe(name: &str) -> LaunchSpec {
    LaunchSpec {
        exe: Some(name.to_owned()),
        app_bundle: None,
        args: None,
        cwd: None,
        env: None,
        chmod_x: None,
    }
}

#[test]
fn exe_resolves_inside_the_install() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    fs::create_dir_all(dir.path().join("bin"))?;
    fs::write(dir.path().join("bin/game"), "")?;
    let mut spec = exe("bin/game");
    spec.args = Some(vec!["--data".to_owned(), "{install}/data".to_owned()]);
    spec.cwd = Some("bin".to_owned());
    spec.env = Some(BTreeMap::from([(
        "GAME_HOME".to_owned(),
        "{install}".to_owned(),
    )]));
    let cmd = command_for(&installed(dir.path(), Os::Linux, spec))?;
    let root = dir.path().display().to_string();
    assert_eq!(cmd.program, dir.path().join("bin/game"));
    assert_eq!(cmd.cwd, dir.path().join("bin"));
    assert_eq!(cmd.args, vec!["--data".to_owned(), format!("{root}/data")]);
    assert_eq!(cmd.env.get("GAME_HOME"), Some(&root));
    Ok(())
}

#[test]
fn paths_may_not_leave_the_install() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join("game"), "")?;
    for bad in ["../game", "/bin/sh", ""] {
        let err = command_for(&installed(dir.path(), Os::Linux, exe(bad)));
        assert!(
            matches!(err, Err(LaunchError::UnsafePath(_))),
            "{bad}: {err:?}"
        );
    }
    let mut spec = exe("game");
    spec.cwd = Some("../..".to_owned());
    let err = command_for(&installed(dir.path(), Os::Linux, spec));
    assert!(matches!(err, Err(LaunchError::UnsafePath(_))), "{err:?}");
    Ok(())
}

#[test]
fn missing_program_and_android_fail() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let err = command_for(&installed(dir.path(), Os::Windows, exe("game.exe")));
    assert!(matches!(err, Err(LaunchError::Missing(_))), "{err:?}");
    let err = command_for(&installed(dir.path(), Os::Android, exe("game")));
    assert!(matches!(err, Err(LaunchError::Unsupported(Os::Android))));
    Ok(())
}

fn bundle(root: &Path, plist: Option<&str>, files: &[&str]) -> std::io::Result<PathBuf> {
    let app = root.join("Example.app");
    fs::create_dir_all(app.join("Contents/MacOS"))?;
    if let Some(p) = plist {
        fs::write(app.join("Contents/Info.plist"), p)?;
    }
    for f in files {
        fs::write(app.join("Contents/MacOS").join(f), "")?;
    }
    Ok(app)
}

const PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleExecutable</key><string>example-bin</string>
  <key>CFBundleIdentifier</key><string>com.example.port</string>
</dict></plist>"#;

#[test]
fn app_bundle_runs_its_named_executable() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    bundle(dir.path(), Some(PLIST), &["example-bin", "helper"])?;
    let spec = LaunchSpec {
        app_bundle: Some("Example.app".to_owned()),
        ..exe("unused")
    };
    let cmd = command_for(&installed(dir.path(), Os::Macos, spec))?;
    assert_eq!(
        cmd.program,
        dir.path().join("Example.app/Contents/MacOS/example-bin")
    );
    assert_eq!(cmd.cwd, dir.path());
    Ok(())
}

#[test]
fn bundle_without_plist_uses_its_only_program() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let app = bundle(dir.path(), None, &["only"])?;
    assert_eq!(bundle_executable(&app)?, app.join("Contents/MacOS/only"));

    let dir = tempfile::tempdir()?;
    let app = bundle(dir.path(), None, &["a", "b"])?;
    assert!(matches!(
        bundle_executable(&app),
        Err(LaunchError::BadBundle { .. })
    ));

    let evil = PLIST.replace("example-bin", "../../../evil");
    let dir = tempfile::tempdir()?;
    let app = bundle(dir.path(), Some(&evil), &[])?;
    assert!(matches!(
        bundle_executable(&app),
        Err(LaunchError::UnsafePath(_))
    ));
    Ok(())
}

#[cfg(unix)]
mod process {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use std::time::Instant;

    use super::*;

    fn script(dir: &Path, body: &str) -> std::io::Result<LaunchCommand> {
        let path = dir.join("game.sh");
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        Ok(LaunchCommand {
            program: path,
            args: vec!["one".to_owned(), "two words".to_owned()],
            cwd: dir.to_path_buf(),
            env: BTreeMap::from([("EXAMPLE_VAR".to_owned(), "set".to_owned())]),
        })
    }

    #[tokio::test]
    async fn runs_and_reports_the_exit_code() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let cmd = script(dir.path(), r#"echo "$1|$2|$EXAMPLE_VAR|$(pwd)"; exit 3"#)?;
        let log = dir.path().join("logs/example.log");
        let running = spawn(&cmd, Some(&log))?;
        assert!(running.pid() > 0);
        let exit = running.wait(&Notify::new()).await?;
        assert_eq!(
            exit,
            Exit {
                code: Some(3),
                stopped: false
            }
        );
        let out = fs::read_to_string(&log)?;
        let name = dir
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        assert!(out.contains("one|two words|set|"), "{out}");
        assert!(
            out.trim_end().ends_with(name),
            "ran in the wrong folder: {out}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn stop_ends_the_game_and_its_helpers() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let marker = dir.path().join("helper-alive");
        // A helper in the background, as some ports relaunch themselves.
        let cmd = script(
            dir.path(),
            &format!("(sleep 2; touch '{}') & sleep 30", marker.display()),
        )?;
        let running = spawn(&cmd, None)?;
        let stop = Arc::new(Notify::new());
        let waiter = {
            let stop = stop.clone();
            tokio::spawn(async move { running.wait(&stop).await })
        };
        tokio::time::sleep(Duration::from_millis(200)).await;
        let asked = Instant::now();
        stop.notify_one();
        let exit = waiter.await??;
        assert!(exit.stopped);
        assert!(asked.elapsed() < STOP_GRACE, "took {:?}", asked.elapsed());
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert!(!marker.exists(), "the helper outlived Stop");
        Ok(())
    }

    #[tokio::test]
    async fn a_game_that_ignores_stop_is_killed() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let cmd = script(dir.path(), "trap '' TERM; while true; do sleep 1; done")?;
        let running = spawn(&cmd, None)?;
        let stop = Notify::new();
        stop.notify_one();
        tokio::time::sleep(Duration::from_millis(200)).await;
        let exit = running.wait(&stop).await?;
        assert!(exit.stopped);
        assert_eq!(exit.code, None);
        Ok(())
    }

    #[test]
    fn a_program_that_cannot_run_fails_to_spawn() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let _guard = rt.enter();
        let cmd = LaunchCommand {
            program: dir.path().join("nope"),
            args: vec![],
            cwd: dir.path().to_path_buf(),
            env: BTreeMap::new(),
        };
        assert!(matches!(spawn(&cmd, None), Err(LaunchError::Spawn { .. })));
        Ok(())
    }
}

#[test]
fn windows_builds_run_through_proton() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join("Example.exe"), "")?;
    let mut spec = exe("Example.exe");
    spec.args = Some(vec!["-windowed".to_owned()]);
    let mut install = installed(dir.path(), Os::Windows, spec);
    install.fit = BuildFit::Proton;
    let data = tempfile::tempdir()?;
    let setup = ProtonSetup {
        script: PathBuf::from("/steam/steamapps/common/Proton 9.0/proton"),
        steam_dir: PathBuf::from("/steam"),
        prefix: data.path().join("proton/example"),
    };
    let cmd = proton_command_for(&install, &setup)?;
    assert_eq!(cmd.program, setup.script);
    assert_eq!(
        cmd.args,
        vec![
            "run".to_owned(),
            dir.path().join("Example.exe").display().to_string(),
            "-windowed".to_owned()
        ]
    );
    assert_eq!(cmd.cwd, dir.path());
    assert_eq!(
        cmd.env.get("STEAM_COMPAT_DATA_PATH"),
        Some(&setup.prefix.display().to_string())
    );
    assert_eq!(
        cmd.env
            .get("STEAM_COMPAT_CLIENT_INSTALL_PATH")
            .map(String::as_str),
        Some("/steam")
    );
    assert!(setup.prefix.is_dir());
    Ok(())
}
