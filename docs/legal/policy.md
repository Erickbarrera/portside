# Legal & content policy

Not legal advice. [TBD: legal review before public alpha — DMCA §1201, per-port licenses,
trademark use of game names/art.]

## Hard rules
1. Portside never hosts, mirrors, proxies, caches or links to ROMs, ISOs, disc images,
   BIOS files, decrypted keys or extracted game assets.
2. Binaries are downloaded by the client directly from the upstream project's own release
   page. We do not re-host them (exception: a maintainer explicitly grants permission and
   the license allows it — record it in the manifest `redistribution` field).
3. Asset import is local-only. Files are hashed on-device and compared to published
   hashes; they are never uploaded. Save backups must exclude imported assets.
4. Forum/chat rules forbid requesting or sharing ROMs/assets or links to them; moderators
   remove such content from Portside surfaces and labeler marks it.
5. Respect upstream licenses: display license, link to source, never strip notices.
6. Game names are used descriptively. Screenshots and covers are linked, never copied or
   re-hosted, and only from three sources (Erick, 2026-10-14; ADR-0019, ADR-0020): the
   upstream awesome-decomp-games list's own screenshots, the social preview image a port's
   maintainers set on their GitHub repo, and community-made game covers on SteamGridDB
   (no NSFW or humor images). A maintainer who asks gets their image removed (rule 7); a
   wrong or unwanted picture from any of the three is replaced or turned off in
   `manifests/covers.yaml`.
   Exception (Erick, 2026-10-15): the website's screenshots of the desktop app show these
   covers as the app displays them, which copies them into the screenshot files. A cover
   turned off or taken down here is also removed from those screenshots by recapturing them
   (`pnpm screenshots` in `web/`).
   [TBD: covered by the legal review above]
7. Honor takedown requests from port maintainers who don't want to be listed.

## Repository hygiene
`.gitignore` blocks common ROM/disc extensions. Tests use synthetic files.
