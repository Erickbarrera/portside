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
- Distribute: AppImage + .deb. No Flatpak or Flathub package (Erick, 2026-10-16).
- Steam Deck: controllers work everywhere (ui.md, "Controller"); in Game Mode Steam Input
  hands Portside a virtual controller. "Add to Steam" (below) and couch mode (ui.md, on by
  itself in Game Mode) work on every OS. Deck only: install to the SD card (below), and
  Steam's on-screen keyboard for text fields (ui.md, "Couch mode"). Windows-only ports run
  through Proton on any x86_64 Linux (below). Building needs `libudev-dev` (gilrs).
- Prefer AppImage ports; mark builds needing glibc ≥ X.

## SD card (Steam Deck only, 2026-10-16)
On a Steam Deck (DMI board vendor `Valve`) with an SD card in, the install step offers
"Internal storage" or "SD card", with the free space on each; the choice is remembered.
`portside-core::paths::sd_card()` finds the card in `/proc/mounts`: the `mmcblk` device
mounted under `/run/media` (`/run/media/deck/<label>`, or `/run/media/mmcblk0p1` before
SteamOS 3.5). Games go in `<card>/Portside/<id>/<version>/`, with the download and unpacking
on the card too. Updates stay where the game is. With the card out, Play and updates say to
put it back in (`E_STORAGE_MISSING`), and Uninstall only forgets the game. Not on other
computers, by Erick's choice. `PORTSIDE_SD_CARD=<folder>` stands in for a card when testing
elsewhere. Not yet: moving an installed game between internal storage and the card.

## Proton (x86_64 Linux, 2026-10-16)
A port with no Linux build but a Windows x86_64 (or universal) build installs that build
and runs it through Proton, Steam's compatibility layer (`BuildFit::Proton`). The install
step says so. Portside doesn't download Proton: it uses one the player installed through
Steam (`portside_steam::find_proton`), looking in every Steam library's `steamapps/common`
(from `steamapps/libraryfolders.vdf`) and in `compatibilitytools.d`. It prefers Valve's
newest numbered release, then Proton Experimental, then community builds such as
GE-Proton; `PORTSIDE_PROTON=<folder>` picks one. Without any, the install step fails with
`E_NO_PROTON` and says how to install it in Steam.
Each game gets its own prefix in `{app_data}/proton/<port>`, kept on uninstall (it may hold
saves). Launch: `proton run <exe>` with `STEAM_COMPAT_DATA_PATH` and
`STEAM_COMPAT_CLIENT_INSTALL_PATH`; Stop ends the whole process group, wineserver included.
Cloud saves use the manifest's Windows save folder inside the prefix (save-sync.md).
[verify on a Deck: Proton 9 outside Steam's runtime container]

## Add to Steam (every OS, 2026-10-16)
The game page of an installed game offers "Add to Steam" when Steam is installed and has
been signed in to (crate `portside-steam`, src-tauri `steam.rs`). It adds a non-Steam
shortcut to the last-used account's `userdata/<id>/config/shortcuts.vdf`:
- Program: Portside itself (`$APPIMAGE` for the AppImage, else its executable), launch
  options `--play <port_id>`, tag "Portside". Updates move games to new folders, so the
  shortcut never points at the game. Started that way, Portside opens the game page, starts
  the game (cloud saves sync as usual) and quits after it exits with code 0 and its saves
  have synced. A crash, or a launch that fails (a save conflict), keeps Portside open on the
  game page. Uninstalled later, the shortcut opens the page with Install.
- Artwork: the catalog's `screenshot_url`, downloaded (https, at most 15 MB) and cropped to
  Steam's tall (`<appid>p.png`, 600×900), wide (`<appid>.png`, 920×430) and hero
  (`<appid>_hero.png`, 1920×620) sizes in `config/grid/`. Without a picture Steam shows
  its own placeholder. No icon yet. Allowed by docs/legal/policy.md rule 6 (Erick,
  2026-10-16): the copy stays on the player's computer.
- Steam reads the file when it starts and may write over it while running, so it is only
  changed with Steam closed. With Steam open the page asks first, then closes Steam
  (`steam://exit`, up to 30 s), writes, and opens it again (`steam://open/games`). When
  Steam started Portside (Game Mode, Big Picture: `SteamGameId`, `SteamAppId` or
  `SteamGamepadUI` set) Portside can't close it; the page says to open Portside on its own
  (Desktop Mode on a Steam Deck).
- Steam folders: Linux `~/.local/share/Steam`, `~/.steam/steam`, Flatpak and Snap; macOS
  `~/Library/Application Support/Steam`; Windows `Program Files (x86)\Steam`
  [TBD: Steam installed elsewhere on Windows, from the registry].
- Not yet: a second Portside started by Steam while one is open runs alongside it (no
  single-instance lock); "Remove from Steam".

## Android (2026-10-18)
Same app as the desktop (Tauri 2's Android target), one APK for arm64 and x86_64 phones,
tablets and Chromebooks, package `gg.portside.app` (`src-tauri/tauri.android.conf.json`; the
desktop identifier stays `gg.portside.desktop`). The Android Studio project is
`src-tauri/gen/android`, generated by `pnpm tauri android init` and committed with our
changes: `AppsPlugin.kt` and the permissions in `AndroidManifest.xml`.
- Distribution: the APK on the GitHub release (build-release.md, "Android"), later F-Droid,
  Obtainium. Not Google Play (installing other APKs via `REQUEST_INSTALL_PACKAGES` is
  restricted for stores).
- Which games: only builds a manifest declares for `os: android` (`archive: apk` with
  `android_package`). Desktop builds never run here; there is no emulation layer. Five ports
  have one so far (manifests/COVERAGE.md).
- Install: Portside downloads and checks the APK like any build (same hosts, checksums),
  then `AppsPlugin.install` writes it into a `PackageInstaller` session; Android shows its
  own confirm screen. The first time, Android sends the player to Settings to allow
  Portside to install apps (`E_INSTALL_PERMISSION`; install again afterwards). Refused or
  cancelled, the library goes back to what was installed before (`E_ANDROID_INSTALL`,
  `E_CANCELLED`). The APK file is deleted once installed. Updates install over the old
  version; Android refuses one signed by someone else ("Uninstall it first").
- Launch: `getLaunchIntentForPackage`. Android runs the game as its own app, so Portside
  doesn't follow it: no Playing state, Stop or play time yet [TBD: `UsageStatsManager`].
- Uninstall: Android's own screen (`ACTION_DELETE`); keeping the game keeps it in the
  library. Games removed in Android's settings drop out of the library the next time it
  loads. Package visibility: `<queries>` for launcher apps; Portside also sees the apps it
  installed.
- Not on Android: Add to Steam, Proton, SD card choice, self-update (new versions come from
  the website), "Show files", cloud saves (`E_UNSUPPORTED`; Android 11+ blocks
  `Android/data/<other app>`, so sync needs a SAF tree the player grants with
  `ACTION_OPEN_DOCUMENT_TREE`, from a `{saf:…}` save root [TBD]). Controllers: gilrs has no
  Android backend; the webview's own focus handling applies.
- HTTPS: the system certificate verifier needs JNI setup Portside doesn't do, so Android
  builds use Mozilla's roots (`portside-core::http`).
- min SDK 24 (Tauri's default), target the latest.
