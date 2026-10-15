<!-- Template for client releases (used by .github/workflows/client-release.yml).
     Edit "What's new" for each release. -->

## What's new

The first public alpha of Portside: browse the catalog of community game ports, install and
update ports that have an install manifest, launch them, and sign in with Bluesky for
friends, chat and discussion boards. Expect rough edges; please report bugs as issues.

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
