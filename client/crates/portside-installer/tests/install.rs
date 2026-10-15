//! End-to-end installs against a mock GitHub (API and downloads) with synthetic archives.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use portside_api::models::PortDetail;
use portside_core::Platform;
use portside_core::types::{Arch, Os};
use portside_installer::{
    BuildFit, HostPolicy, InstallError, InstallPlan, Installed, Installer, InstallerConfig,
    Library, Verification,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const WINDOWS: Platform = Platform {
    os: Os::Windows,
    arch: Arch::X86_64,
};
const MAC_ARM: Platform = Platform {
    os: Os::Macos,
    arch: Arch::Arm64,
};

struct Env {
    server: MockServer,
    dir: tempfile::TempDir,
}

impl Env {
    async fn new() -> Result<Env, Box<dyn std::error::Error>> {
        Ok(Env {
            server: MockServer::start().await,
            dir: tempfile::tempdir()?,
        })
    }

    fn library_dir(&self) -> std::path::PathBuf {
        self.dir.path().join("library")
    }

    fn installer(&self, platform: Platform) -> Result<Installer, InstallError> {
        Installer::new(
            InstallerConfig {
                library_dir: self.library_dir(),
                platform,
                user_agent: "portside-test".into(),
                github_api: self.server.uri(),
                hosts: HostPolicy::local_for_tests(&["127.0.0.1"]),
            },
            Library::open_in_memory()?,
        )
    }

    fn url(&self, file: &str) -> String {
        format!("{}/dl/{file}", self.server.uri())
    }

    /// Serves `bytes` at `/dl/<name>`, honoring `Range: bytes=N-`.
    async fn serve(&self, name: &str, bytes: Vec<u8>) {
        Mock::given(method("GET"))
            .and(path(format!("/dl/{name}")))
            .respond_with(Ranged(bytes))
            .mount(&self.server)
            .await;
    }

    /// `/repos/example/game/releases/latest` lists `files` as (name, bytes, publish digest).
    async fn release(&self, files: &[(&str, &[u8], bool)]) {
        let assets: Vec<Value> = files
            .iter()
            .map(|(name, bytes, digest)| {
                let mut a = json!({
                    "name": name,
                    "size": bytes.len(),
                    "browser_download_url": self.url(name),
                });
                if *digest {
                    a["digest"] = json!(format!("sha256:{}", sha256(bytes)));
                }
                a
            })
            .collect();
        Mock::given(method("GET"))
            .and(path("/repos/example/game/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tag_name": "v1.0",
                "html_url": "https://github.com/example/game/releases/tag/v1.0",
                "assets": assets,
            })))
            .mount(&self.server)
            .await;
        for (name, bytes, _) in files {
            self.serve(name, bytes.to_vec()).await;
        }
    }
}

struct Ranged(Vec<u8>);

impl Respond for Ranged {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let from = req
            .headers
            .get("range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("bytes="))
            .and_then(|v| v.trim_end_matches('-').parse::<usize>().ok());
        match from {
            Some(n) if n < self.0.len() => ResponseTemplate::new(206)
                .insert_header(
                    "content-range",
                    format!("bytes {n}-{}/{}", self.0.len() - 1, self.0.len()),
                )
                .set_body_bytes(self.0[n..].to_vec()),
            _ => ResponseTemplate::new(200).set_body_bytes(self.0.clone()),
        }
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn port(builds: Value, releases: Value) -> Result<PortDetail, serde_json::Error> {
    serde_json::from_value(json!({
        "id": "example-port",
        "game_key": "examplegame",
        "title": "Example Port",
        "type": "decompilation",
        "status": "playable",
        "os_status": [],
        "stars": null,
        "archived": false,
        "screenshot_url": null,
        "play_url": null,
        "updated_at": "2026-10-13T00:00:00Z",
        "description": null,
        "repo": "example/game",
        "homepage": null,
        "license": null,
        "last_commit": null,
        "upstream_score": null,
        "ratings": null,
        "video_url": null,
        "note": null,
        "status_notes": null,
        "manifest": {
            "schema": 1,
            "id": "example-port",
            "game_key": "examplegame",
            "status": "playable",
            "source": { "type": "github_releases", "repo": "example/game", "channel": "stable" },
            "builds": builds,
        },
        "manifest_rev": "1",
        "verified": false,
        "latest_releases": releases,
    }))
}

fn windows_zip_build() -> Value {
    json!([{
        "os": "windows", "arch": "x86_64", "asset": "Example-*-Win64.zip", "archive": "zip",
        "strip_root": true, "launch": { "exe": "example.exe" }
    }])
}

/// A zip of `(name, contents, unix mode)`; a `None` content makes a directory.
fn zip(entries: &[(&str, Option<&[u8]>, u32)]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        for (name, body, mode) in entries {
            let opts = zip::write::SimpleFileOptions::default().unix_permissions(*mode);
            match body {
                Some(b) => {
                    w.start_file(*name, opts)?;
                    w.write_all(b)?;
                }
                None => w.add_directory(*name, opts)?,
            }
        }
        w.finish()?;
    }
    Ok(buf.into_inner())
}

fn example_zip() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    zip(&[
        ("Example Game/", None, 0o755),
        ("Example Game/example.exe", Some(b"synthetic exe"), 0o644),
        (
            "Example Game/data/readme.txt",
            Some(b"synthetic data"),
            0o644,
        ),
    ])
}

async fn run(
    installer: &Installer,
    plan: &InstallPlan,
    accept_unsigned: bool,
) -> Result<Installed, InstallError> {
    let cancel = Arc::new(AtomicBool::new(false));
    installer
        .install(plan, accept_unsigned, &cancel, &|_| {})
        .await
}

#[tokio::test]
async fn installs_a_zip_from_github() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    env.release(&[
        ("Example-1.0-Win64.zip", &bytes, true),
        ("Example-1.0-Mac.zip", b"other", true),
    ])
    .await;
    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    assert_eq!(plan.summary.version, "v1.0");
    assert_eq!(plan.summary.asset, "Example-1.0-Win64.zip");
    assert_eq!(plan.summary.fit, BuildFit::Native);
    assert_eq!(plan.summary.verification, Verification::Github);
    assert_eq!(plan.summary.installed_version, None);

    let installed = run(&installer, &plan, false).await?;
    let dir = env.library_dir().join("example-port/v1.0");
    assert_eq!(Path::new(&installed.dir), dir);
    assert_eq!(std::fs::read(dir.join("example.exe"))?, b"synthetic exe");
    assert!(dir.join("data/readme.txt").is_file());
    assert_eq!(
        std::fs::read_to_string(env.library_dir().join("example-port/current"))?,
        "v1.0"
    );
    assert_eq!(installed.sha256.as_deref(), Some(sha256(&bytes).as_str()));
    // Nothing left behind but the install.
    assert!(
        !env.library_dir()
            .join(".partial/example-port--Example-1.0-Win64.zip.part")
            .exists()
    );
    assert_eq!(
        std::fs::read_dir(env.library_dir().join(".staging"))?.count(),
        0
    );

    assert_eq!(installer.library().list()?, vec![installed.clone()]);
    let again = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    assert_eq!(again.summary.installed_version.as_deref(), Some("v1.0"));

    installer.uninstall("example-port").await?;
    assert!(!env.library_dir().join("example-port").exists());
    assert!(installer.library().list()?.is_empty());
    Ok(())
}

#[tokio::test]
async fn resumes_a_partial_download() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    env.release(&[("Example-1.0-Win64.zip", &bytes, true)])
        .await;
    let part = env
        .library_dir()
        .join(".partial/example-port--Example-1.0-Win64.zip.part");
    std::fs::create_dir_all(part.parent().ok_or("no parent")?)?;
    std::fs::write(&part, &bytes[..40])?;

    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    run(&installer, &plan, false).await?;
    let ranged = env
        .server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.headers.get("range").and_then(|v| v.to_str().ok()) == Some("bytes=40-"))
        .count();
    assert_eq!(ranged, 1);
    Ok(())
}

#[tokio::test]
async fn rejects_a_file_that_does_not_match_its_checksum() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    // The listing's digest is for different bytes than the ones served.
    Mock::given(method("GET"))
        .and(path("/repos/example/game/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tag_name": "v1.0", "html_url": "",
            "assets": [{ "name": "Example-1.0-Win64.zip", "size": bytes.len(),
                         "browser_download_url": env.url("Example-1.0-Win64.zip"),
                         "digest": format!("sha256:{}", sha256(b"something else")) }],
        })))
        .mount(&env.server)
        .await;
    env.serve("Example-1.0-Win64.zip", bytes).await;
    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    assert!(matches!(
        run(&installer, &plan, false).await,
        Err(InstallError::HashMismatch { .. })
    ));
    assert!(installer.library().list()?.is_empty());
    assert!(!env.library_dir().join("example-port/v1.0").exists());
    Ok(())
}

#[tokio::test]
async fn uses_a_published_checksum_file() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    let sums = format!(
        "{}  Example-1.0-Win64.zip\n{}  Example-1.0-Mac.zip\n",
        sha256(&bytes),
        sha256(b"x")
    );
    env.release(&[
        ("Example-1.0-Win64.zip", &bytes, false),
        ("SHA256SUMS.txt", sums.as_bytes(), false),
    ])
    .await;
    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    assert_eq!(plan.summary.verification, Verification::ChecksumFile);
    let installed = run(&installer, &plan, false).await?;
    assert_eq!(installed.verified, Verification::ChecksumFile);
    Ok(())
}

#[tokio::test]
async fn upstream_checksum_must_exist() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    env.release(&[("Example-1.0-Win64.zip", &bytes, false)])
        .await;
    let mut builds = windows_zip_build();
    builds[0]["checksum"] = json!("upstream");
    let installer = env.installer(WINDOWS)?;
    let planned = installer.plan(&port(builds, json!([]))?).await;
    assert!(matches!(planned, Err(InstallError::ChecksumMissing(_))));
    Ok(())
}

#[tokio::test]
async fn prefers_releases_from_the_portside_api() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    env.serve("Example-2.0-Win64.zip", bytes.clone()).await;
    let releases = json!([{
        "id": "r2", "port_id": "example-port", "tag": "v2.0", "name": null,
        "published_at": "2026-10-13T00:00:00Z", "prerelease": false, "revoked": false,
        "html_url": "https://github.com/example/game/releases/tag/v2.0",
        "assets": [{ "os": "windows", "arch": "x86_64", "name": "Example-2.0-Win64.zip",
                     "url": env.url("Example-2.0-Win64.zip"), "size": bytes.len(),
                     "sha256": sha256(&bytes) }],
    }]);
    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), releases)?)
        .await?;
    assert_eq!(plan.summary.version, "v2.0");
    assert_eq!(plan.summary.verification, Verification::Portside);
    run(&installer, &plan, false).await?;
    let api_calls = env
        .server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|r| r.url.path().starts_with("/repos/"))
        .count();
    assert_eq!(api_calls, 0);
    Ok(())
}

#[tokio::test]
async fn refuses_downloads_from_other_hosts() -> TestResult {
    let env = Env::new().await?;
    Mock::given(method("GET"))
        .and(path("/repos/example/game/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tag_name": "v1.0", "html_url": "",
            "assets": [{ "name": "Example-1.0-Win64.zip", "size": 3,
                         "browser_download_url": env.url("Example-1.0-Win64.zip") }],
        })))
        .mount(&env.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dl/Example-1.0-Win64.zip"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", "http://files.example.invalid/x.zip"),
        )
        .mount(&env.server)
        .await;
    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    match run(&installer, &plan, false).await {
        Err(InstallError::HostNotAllowed(host)) => assert_eq!(host, "files.example.invalid"),
        other => panic!("expected HostNotAllowed, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn reports_github_rate_limits() -> TestResult {
    let env = Env::new().await?;
    Mock::given(method("GET"))
        .and(path("/repos/example/game/releases/latest"))
        .and(header("accept", "application/vnd.github+json"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "1800000000"),
        )
        .mount(&env.server)
        .await;
    let installer = env.installer(WINDOWS)?;
    let planned = installer.plan(&port(windows_zip_build(), json!([]))?).await;
    assert!(matches!(
        planned,
        Err(InstallError::RateLimited {
            reset_at: Some(1_800_000_000)
        })
    ));
    Ok(())
}

#[tokio::test]
async fn no_build_for_this_platform() -> TestResult {
    let env = Env::new().await?;
    let installer = env.installer(Platform {
        os: Os::Linux,
        arch: Arch::X86_64,
    })?;
    let planned = installer.plan(&port(windows_zip_build(), json!([]))?).await;
    assert!(matches!(planned, Err(InstallError::NoBuild { .. })));
    Ok(())
}

#[tokio::test]
async fn unsigned_mac_apps_need_consent() -> TestResult {
    let env = Env::new().await?;
    let bytes = zip(&[
        (
            "Example.app/Contents/MacOS/example",
            Some(b"synthetic binary"),
            0o755,
        ),
        ("Example.app/Contents/Info.plist", Some(b"<plist/>"), 0o644),
    ])?;
    env.release(&[("Example-Mac.zip", &bytes, true)]).await;
    let builds = json!([{
        "os": "macos", "arch": "x86_64", "asset": "Example-Mac.zip", "archive": "zip",
        "launch": { "app_bundle": "Example.app" }, "unsigned": true
    }]);
    let installer = env.installer(MAC_ARM)?;
    let plan = installer.plan(&port(builds, json!([]))?).await?;
    assert!(plan.summary.unsigned);
    assert_eq!(plan.summary.fit, BuildFit::Translated);
    assert!(matches!(
        run(&installer, &plan, false).await,
        Err(InstallError::NeedsConsent)
    ));
    let installed = run(&installer, &plan, true).await?;
    let exe = Path::new(&installed.dir).join("Example.app/Contents/MacOS/example");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&exe)?.permissions().mode() & 0o777, 0o755);
    }
    assert!(exe.is_file());
    Ok(())
}

#[tokio::test]
async fn missing_launch_target_fails_the_install() -> TestResult {
    let env = Env::new().await?;
    let bytes = zip(&[("Example Game/other.exe", Some(b"x"), 0o644)])?;
    env.release(&[("Example-1.0-Win64.zip", &bytes, true)])
        .await;
    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    assert!(matches!(
        run(&installer, &plan, false).await,
        Err(InstallError::LaunchTargetMissing(t)) if t == "example.exe"
    ));
    assert!(!env.library_dir().join("example-port/v1.0").exists());
    Ok(())
}

#[tokio::test]
async fn refuses_archive_paths_outside_the_install() -> TestResult {
    let env = Env::new().await?;
    let bytes = zip(&[("../escape.exe", Some(b"x"), 0o644)])?;
    env.release(&[("Example-1.0-Win64.zip", &bytes, true)])
        .await;
    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    assert!(matches!(
        run(&installer, &plan, false).await,
        Err(InstallError::UnsafePath(_))
    ));
    assert!(!env.library_dir().join(".staging/escape.exe").exists());
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn never_writes_through_an_archive_symlink() -> TestResult {
    let env = Env::new().await?;
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        w.add_symlink("up", "..", opts)?;
        w.start_file("up/escape.exe", opts)?;
        w.write_all(b"x")?;
        w.finish()?;
    }
    env.release(&[("Example-1.0-Win64.zip", buf.get_ref(), true)])
        .await;
    let installer = env.installer(WINDOWS)?;
    let plan = installer
        .plan(&port(windows_zip_build(), json!([]))?)
        .await?;
    assert!(matches!(
        run(&installer, &plan, false).await,
        Err(InstallError::UnsafePath(_))
    ));
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn installs_a_tarball_and_marks_the_exe_executable() -> TestResult {
    let env = Env::new().await?;
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    let body = b"synthetic appimage";
    let mut h = tar::Header::new_gnu();
    h.set_size(body.len() as u64);
    h.set_mode(0o644);
    h.set_cksum();
    tar.append_data(&mut h, "example/Example.AppImage", &body[..])?;
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    link.set_cksum();
    tar.append_link(&mut link, "example/latest", "Example.AppImage")?;
    let bytes = tar.into_inner()?.finish()?;
    env.release(&[("example-linux.tar.gz", &bytes, true)]).await;
    let builds = json!([{
        "os": "linux", "arch": "x86_64", "asset": "example-linux.tar.gz", "archive": "tar.gz",
        "strip_root": true, "launch": { "exe": "Example.AppImage", "chmod_x": true }
    }]);
    let installer = env.installer(Platform {
        os: Os::Linux,
        arch: Arch::X86_64,
    })?;
    let plan = installer.plan(&port(builds, json!([]))?).await?;
    let installed = run(&installer, &plan, false).await?;
    let dir = Path::new(&installed.dir);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(dir.join("Example.AppImage"))?
            .permissions()
            .mode()
            & 0o111,
        0o111
    );
    assert_eq!(
        std::fs::read_link(dir.join("latest"))?,
        Path::new("Example.AppImage")
    );
    Ok(())
}

/// Builds a real disk image with `hdiutil`, so this only runs on macOS.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn installs_from_a_disk_image() -> TestResult {
    let env = Env::new().await?;
    let src = env.dir.path().join("volume");
    let exe = src.join("Example.app/Contents/MacOS/example");
    std::fs::create_dir_all(exe.parent().ok_or("no parent")?)?;
    std::fs::write(&exe, b"synthetic binary")?;
    std::os::unix::fs::symlink("/Applications", src.join("Applications"))?;
    let dmg = env.dir.path().join("Example.dmg");
    let status = std::process::Command::new("hdiutil")
        .args([
            "create",
            "-quiet",
            "-fs",
            "HFS+",
            "-format",
            "UDZO",
            "-volname",
            "Example",
            "-srcfolder",
        ])
        .arg(&src)
        .arg(&dmg)
        .status()?;
    assert!(status.success());
    env.release(&[("Example.dmg", &std::fs::read(&dmg)?, true)])
        .await;
    let builds = json!([{
        "os": "macos", "arch": "universal", "asset": "Example.dmg", "archive": "dmg",
        "launch": { "app_bundle": "Example.app" }
    }]);
    let installer = env.installer(MAC_ARM)?;
    let plan = installer.plan(&port(builds, json!([]))?).await?;
    let installed = run(&installer, &plan, false).await?;
    let dir = Path::new(&installed.dir);
    assert_eq!(
        std::fs::read(dir.join("Example.app/Contents/MacOS/example"))?,
        b"synthetic binary"
    );
    assert!(std::fs::symlink_metadata(dir.join("Applications")).is_err());
    Ok(())
}
