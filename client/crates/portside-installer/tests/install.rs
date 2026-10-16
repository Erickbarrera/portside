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
    let source = json!({ "type": "github_releases", "repo": "example/game", "channel": "stable" });
    port_from(source, builds, releases)
}

fn port_from(
    source: Value,
    builds: Value,
    releases: Value,
) -> Result<PortDetail, serde_json::Error> {
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
            "source": source,
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
        .install(plan, None, accept_unsigned, &cancel, &|_| {})
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
async fn installs_to_another_library_and_updates_there() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    env.release(&[("Example-1.0-Win64.zip", &bytes, true)])
        .await;
    let card = env.dir.path().join("card");
    std::fs::create_dir(&card)?;
    let sd = card.join("Portside");
    let installer = env.installer(WINDOWS)?;
    let detail = port(windows_zip_build(), json!([]))?;
    let never = Arc::new(AtomicBool::new(false));

    let plan = installer.plan(&detail).await?;
    let installed = installer
        .install(&plan, Some(&sd), false, &never, &|_| {})
        .await?;
    assert_eq!(Path::new(&installed.dir), sd.join("example-port/v1.0"));
    assert!(!env.library_dir().join("example-port").exists());

    // Installing again (an update) stays on the card, whatever is asked.
    let plan = installer.plan(&detail).await?;
    let again = installer
        .install(&plan, None, false, &never, &|_| {})
        .await?;
    assert_eq!(again.dir, installed.dir);

    // With the card out, an update can't land anywhere else.
    std::fs::rename(&card, env.dir.path().join("card-out"))?;
    let plan = installer.plan(&detail).await?;
    let out = installer.install(&plan, None, false, &never, &|_| {}).await;
    assert!(matches!(out, Err(InstallError::StorageMissing(p)) if p == sd));
    std::fs::rename(env.dir.path().join("card-out"), &card)?;

    installer.uninstall("example-port").await?;
    assert!(!sd.join("example-port").exists());
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

/// A `url_list` port whose site is the mock server under the name `localhost`, which the
/// installer's own (GitHub) host list doesn't include.
fn listed_port(url: &str, bytes: &[u8]) -> Result<PortDetail, serde_json::Error> {
    let source = json!({
        "type": "url_list", "version": "3.1", "released": "2026-09-30",
        "release_url": "https://downloads.example.invalid/news/3.1", "hosts": ["localhost"],
    });
    let builds = json!([{
        "os": "windows", "arch": "x86_64", "asset": "example-3.1-win64.zip", "archive": "zip",
        "url": url, "sha256": sha256(bytes), "size": bytes.len(),
        "strip_root": true, "launch": { "exe": "example.exe" }
    }]);
    port_from(source, builds, json!([]))
}

fn localhost_url(env: &Env, file: &str) -> String {
    env.url(file).replace("127.0.0.1", "localhost")
}

#[tokio::test]
async fn installs_from_a_listed_site() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    env.serve("example-3.1-win64.zip", bytes.clone()).await;
    let installer = env.installer(WINDOWS)?;
    let url = localhost_url(&env, "example-3.1-win64.zip");
    let plan = installer.plan(&listed_port(&url, &bytes)?).await?;
    assert_eq!(plan.summary.version, "3.1");
    assert_eq!(
        plan.summary.release_url,
        "https://downloads.example.invalid/news/3.1"
    );
    assert_eq!(plan.summary.verification, Verification::Portside);
    assert_eq!(plan.summary.repo, None);
    let installed = run(&installer, &plan, false).await?;
    assert!(Path::new(&installed.dir).join("example.exe").is_file());
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
async fn listed_files_stay_on_their_hosts() -> TestResult {
    let env = Env::new().await?;
    let bytes = example_zip()?;
    let installer = env.installer(WINDOWS)?;
    // The installer's own host (GitHub's place in tests) isn't one of the port's hosts.
    let elsewhere = env.url("example-3.1-win64.zip");
    match installer.plan(&listed_port(&elsewhere, &bytes)?).await {
        Err(InstallError::HostNotAllowed(host)) => assert_eq!(host, "127.0.0.1"),
        other => panic!("expected HostNotAllowed, got {other:?}"),
    }
    // Nor may the site redirect elsewhere.
    Mock::given(method("GET"))
        .and(path("/dl/example-3.1-win64.zip"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", "http://files.example.invalid/x.zip"),
        )
        .mount(&env.server)
        .await;
    let url = localhost_url(&env, "example-3.1-win64.zip");
    let plan = installer.plan(&listed_port(&url, &bytes)?).await?;
    match run(&installer, &plan, false).await {
        Err(InstallError::HostNotAllowed(host)) => assert_eq!(host, "files.example.invalid"),
        other => panic!("expected HostNotAllowed, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
async fn listed_files_need_a_checksum() -> TestResult {
    let env = Env::new().await?;
    let installer = env.installer(WINDOWS)?;
    let url = localhost_url(&env, "example-3.1-win64.zip");
    let mut port = listed_port(&url, b"x")?;
    if let Some(m) = port.manifest.as_mut() {
        m.builds[0].sha256 = None;
    }
    match installer.plan(&port).await {
        Err(InstallError::ChecksumMissing(name)) => assert_eq!(name, "example-3.1-win64.zip"),
        other => panic!("expected ChecksumMissing, got {other:?}"),
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
    let installer = env.installer(LINUX)?;
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
    let installer = env.installer(LINUX)?;
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

#[tokio::test]
async fn installs_a_7z_archive() -> TestResult {
    let env = Env::new().await?;
    let src = tempfile::tempdir()?;
    let game = src.path().join("Example Game");
    std::fs::create_dir_all(game.join("data"))?;
    std::fs::write(game.join("example.exe"), b"synthetic exe")?;
    std::fs::write(game.join("data/readme.txt"), b"synthetic data")?;
    let bytes = sevenz_rust2::compress(src.path(), std::io::Cursor::new(Vec::new()))?.into_inner();
    env.release(&[("Example-1.0-Win64.7z", &bytes, true)]).await;
    let builds = json!([{
        "os": "windows", "arch": "x86_64", "asset": "Example-*-Win64.7z", "archive": "7z",
        "strip_root": true, "launch": { "exe": "example.exe" }
    }]);
    let installer = env.installer(WINDOWS)?;
    let plan = installer.plan(&port(builds, json!([]))?).await?;
    let installed = run(&installer, &plan, false).await?;
    let dir = Path::new(&installed.dir);
    assert_eq!(std::fs::read(dir.join("example.exe"))?, b"synthetic exe");
    assert_eq!(
        std::fs::read(dir.join("data/readme.txt"))?,
        b"synthetic data"
    );
    Ok(())
}

const LINUX: Platform = Platform {
    os: Os::Linux,
    arch: Arch::X86_64,
};

#[tokio::test]
async fn installs_an_archive_inside_an_archive() -> TestResult {
    let env = Env::new().await?;
    let inner = example_zip()?;
    let outer = zip(&[
        ("readme.txt", Some(b"synthetic readme"), 0o644),
        ("Example-1.0.zip", Some(&inner), 0o644),
    ])?;
    env.release(&[("Example-1.0-Win64.zip", &outer, true)])
        .await;
    let builds = json!([{
        "os": "windows", "arch": "x86_64", "asset": "Example-*-Win64.zip", "archive": "zip",
        "nested": { "file": "Example-*.zip", "archive": "zip" },
        "strip_root": true, "launch": { "exe": "example.exe" }
    }]);
    let installer = env.installer(WINDOWS)?;
    let plan = installer.plan(&port(builds, json!([]))?).await?;
    let installed = run(&installer, &plan, false).await?;
    let dir = Path::new(&installed.dir);
    assert_eq!(std::fs::read(dir.join("example.exe"))?, b"synthetic exe");
    assert!(dir.join("data/readme.txt").is_file());
    // Only the inner archive's files are kept.
    assert!(!dir.join("readme.txt").exists());
    assert!(!dir.join("Example-1.0.zip").exists());
    Ok(())
}

#[tokio::test]
async fn a_nested_glob_must_match_one_file() -> TestResult {
    let env = Env::new().await?;
    let outer = zip(&[
        ("a/Example.zip", Some(b"x"), 0o644),
        ("b/Example.zip", Some(b"y"), 0o644),
    ])?;
    env.release(&[("Example-1.0-Win64.zip", &outer, true)])
        .await;
    let builds = json!([{
        "os": "windows", "arch": "x86_64", "asset": "Example-*-Win64.zip", "archive": "zip",
        "nested": { "file": "*.zip", "archive": "zip" }, "launch": { "exe": "example.exe" }
    }]);
    let installer = env.installer(WINDOWS)?;
    let plan = installer.plan(&port(builds, json!([]))?).await?;
    assert!(matches!(
        run(&installer, &plan, false).await,
        Err(InstallError::NestedArchive { count: 2, .. })
    ));
    Ok(())
}

#[tokio::test]
async fn saves_an_appimage_under_its_launch_name() -> TestResult {
    let env = Env::new().await?;
    env.release(&[("Example-v1.0-x86_64.AppImage", b"synthetic appimage", true)])
        .await;
    let builds = json!([{
        "os": "linux", "arch": "x86_64", "asset": "Example-*-x86_64.AppImage",
        "archive": "appimage", "launch": { "exe": "Example.AppImage" }
    }]);
    let installer = env.installer(LINUX)?;
    let plan = installer.plan(&port(builds, json!([]))?).await?;
    let installed = run(&installer, &plan, false).await?;
    let file = Path::new(&installed.dir).join("Example.AppImage");
    assert_eq!(std::fs::read(&file)?, b"synthetic appimage");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&file)?.permissions().mode() & 0o111,
            0o111
        );
    }
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
