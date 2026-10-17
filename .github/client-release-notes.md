<!-- Template for client releases (used by .github/workflows/client-release.yml).
     Edit "What's new" for each release. -->

## What's new

Steam Deck and controller support, on every system:

- **Controllers:** move around the whole app with a gamepad (or the arrow keys).
- **Add to Steam:** put an installed game in Steam's library, with artwork, so you can start
  it from Game Mode or Big Picture.
- **Couch mode:** a bigger, full-screen layout with an on-screen keyboard, on by itself in
  Steam's Game Mode and Big Picture (Settings → Couch mode).
- **Steam Deck:** install games to the SD card, and text fields open Steam's own keyboard.
- **Proton:** on Linux and the Steam Deck, ports that only have a Windows build install and
  run through Proton, if Steam has it installed.

Also new: installed games in the sidebar, ports downloaded from their own site, and 7z
archives. Portside is in alpha: expect rough edges and please report bugs as issues.

## Downloads

| System | File |
| --- | --- |
| macOS (Apple Silicon and Intel) | `Portside_<version>_universal.dmg` |
| Windows | `Portside_<version>_x64-setup.exe` (or the `.msi`) |
| Linux / Steam Deck | `Portside_<version>_amd64.AppImage`, `.deb` or `.rpm` |

## These builds are not signed yet

- **macOS:** open the app once, close the warning, then go to System Settings → Privacy &
  Security and click **Open Anyway**. If macOS says the app "is damaged", run
  `xattr -dr com.apple.quarantine /Applications/Portside.app` in Terminal.
- **Windows:** SmartScreen may warn about an unknown publisher; click **More info → Run anyway**.
- **Linux:** make the AppImage executable (`chmod +x`) and run it.

Portside never downloads or shares game files: bring your own.
