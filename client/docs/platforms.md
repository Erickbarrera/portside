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
- Steam Deck: controllers work everywhere (ui.md, "Controller"); in Game Mode Steam Input
  hands Portside a virtual controller. "Add to Steam" (below) and couch mode (ui.md, on by
  itself in Game Mode) work on every OS. Deck only: install to the SD card (below); planned:
  Steam's on-screen keyboard for text fields. Building needs `libudev-dev` (gilrs).
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
