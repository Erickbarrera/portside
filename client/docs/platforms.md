# Platform specifics

## Windows
- Installer: NSIS (Tauri bundler) per-user, no admin. WebView2 evergreen bootstrapper.
- Code signing: [TBD: Azure Trusted Signing or OV cert]. SmartScreen reputation takes time.
- Saves often in `{install}` or `%APPDATA%`.

## macOS
- Universal binary (x86_64 + arm64). Signed + notarized (Developer ID) [TBD account].
- Upstream ports are often unsigned → the user OKs it at install and the installer clears
  `com.apple.quarantine` on that bundle (install-launch.md, ADR-0014).
- Rosetta warning for x86_64-only builds on Apple Silicon at install; launching one without
  Rosetta fails with `E_NEEDS_ROSETTA` and the `softwareupdate --install-rosetta` hint.
- Games launch through the bundle's executable, not Launch Services (ADR-0017), so they may
  open behind Portside's window.

## Linux / Steam Deck
- Distribute: AppImage + Flatpak (Flathub) + .deb. Flatpak needs filesystem permission for
  the library dir and `--talk-name=org.freedesktop.Flatpak` to spawn host processes via
  `flatpak-spawn --host` [verify]; document limits.
- Steam Deck: "Add to Steam" for Game Mode (write a non-Steam shortcut to
  `shortcuts.vdf`), controller-first "Big Picture" UI mode (`?mode=couch`), gamepad nav via
  Gamepad API in the webview.
- Prefer AppImage ports; mark builds needing glibc ≥ X.

## Android
- Distribution: direct APK from website, F-Droid, Obtainium. Not Google Play (installing
  other APKs via `REQUEST_INSTALL_PACKAGES` is restricted for stores).
- Port install: download APK → `PackageInstaller` session via Kotlin plugin → user confirms.
- Saves: Android 11+ blocks `Android/data/<other app>`. Sync only where manifest declares a
  SAF-accessible root; user grants a tree via `ACTION_OPEN_DOCUMENT_TREE`; persist URI
  permission. Optional Shizuku path later.
- Launch: `getLaunchIntentForPackage`; playtime via `UsageStatsManager` (needs user grant)
  or foreground heuristics.
- min SDK 26 [TBD], target latest.
