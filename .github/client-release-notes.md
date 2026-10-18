<!-- Template for client releases (used by .github/workflows/client-release.yml).
     Edit "What's new" for each release. -->

## What's new

- **Portside updates itself:** from this version on, Portside checks for a new version when
  it starts and every few hours, and offers to update and restart (Settings → Updates).
  Coming from 0.2.0 or older? Download this version by hand once.
- **Play time on every device:** with cloud saves on, play time adds up across your
  computers, and the Library sorts by recently played.
- **A much bigger catalog:** Portside now reads the catalog from its new home, with hundreds
  more ports.

Portside is in alpha: expect rough edges and please report bugs as issues.

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
