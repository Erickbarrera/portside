# Download page

- Build-time: `lib/releases.ts` fetches latest `client-v*` release via GitHub API
  (token in CI env) and maps assets: `*_x64-setup.exe` → Windows, `*_universal.dmg` → macOS,
  `*_amd64.AppImage`/`.deb` → Linux, `*.apk` → Android [match final Tauri asset names].
- Rebuild the site on each client release (GitHub Actions `repository_dispatch` or
  workflow in the same repo triggered by the release).
- Client-side island: detect OS/arch via `navigator.userAgentData.getHighEntropyValues`
  with UA fallback; highlight the matching card; show checksums (sha256) and signature
  info.
- Also list: Flathub link, F-Droid / Obtainium instructions, release notes link, beta
  channel (collapsed).
