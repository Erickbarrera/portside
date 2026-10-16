# UI

Mockups: Claude design canvas "Portside Client Mockups" (library, game page, cloud saves +
conflict, sign-in, Android library, discussion board, friends & chat):
https://claude.ai/artifact/NyBGECoCXEh8HJtVj5ixeu

## Navigation (desktop)
Sidebar: Library · Discover · Community · Friends · Downloads · Cloud saves · Collections ·
profile (avatar and name, or Sign in) · Settings.
Built so far: Library (a grid of cover cards with play time, last played and Play/Stop;
installs in progress), Discover (a grid of cover cards; search + system/status/type filters
kept in the URL, plus a freshness/offline line for the saved catalog), `/port/:id` (cover
image at the top; offline banner
when only the saved summary is available; install panel: confirm with size, source and
warnings, progress with Cancel, then Play/Stop, play time, Show in Finder/Explorer and Uninstall; a crash shows its exit
code and a link to the log), Settings (Bluesky account and Sign out, saved catalog, "Check for changes"). Social
(2026-10-14): `/signin`; Friends (messages with unread counts, add a friend, friends with
Message/Unfollow, Follows you); `/chat/:id` (bubbles, invite cards with Play or Get the
game, Invite to play, message requests); Community (All discussions, Portside help, Feature requests; New thread on the last two); the game
page's Discussions section and Invite a friend dialog; `/port/:id/board` (sub-forum tabs,
search, latest/top, New thread); `/thread/:uri` (first post stitched, replies, reply box).
Friends shows an unread badge in the sidebar.
Below the nav the sidebar lists installed games, Steam-style (2026-10-16,
`InstalledList.svelte`): a count, a search box, one row per game (small cover, title without
its "(…)" note unless two ports of one game are installed), sorted by title, scrolling on
its own. Rows link to the game page and say Playing, or show progress while installing or
updating.
Other entries appear when their milestone lands.

Covers (`ui/src/lib/components/Cover.svelte`): the catalog's `screenshot_url`, linked from
upstream, the port's repo or SteamGridDB and never copied (docs/legal/policy.md rule 6). A game without one, or whose
image fails to load, gets a generated cover: a gradient picked from its `game_key` (so
every port of a game matches) with the title, minus the trailing "(…)" note. The game page
shows a thin strip of those colors instead of a large image. Mock fixtures use synthetic
SVG "screenshots", never game art.
Routes: `/library`, `/discover`, `/community`, `/port/:id`, `/port/:id/board`,
`/thread/:uri` (the at:// URI encoded as one segment), `/downloads`, `/saves`,
`/saves/:port`, `/friends`, `/chat/:conversation`, `/settings`, `/signin`.

## Android
Bottom nav: Library · Discover · Saves · Profile. Same routes, responsive layouts.

## Couch mode (Steam Deck / TV)
Large tiles, focus rings, D-pad navigation, A/B mapped to select/back.

## Brand
Purple accent and the green pirate-parrot icon (chosen by Erick 2026-10-13, shared with the
website). Icon source: `src-tauri/icons/icon-source.svg` (the website's favicon); app icons in
`src-tauri/icons/` are generated from it with
`pnpm tauri icon src-tauri/icons/icon-source.svg -o src-tauri/icons`. The mockups
above still show the earlier yellow accent; the tokens below win.

## Design tokens
Source of truth in code: `ui/src/app.css` (same values as `web/src/styles/global.css`).
```
--bg: #0F0E17; --surface-1: #15131F; --surface-2: #1A1726; --surface-3: #242036;
--border: #2A2640; --border-strong: #3A3354;
--text: #E8EAEE; --text-2: #AEB6C4; --text-3: #9AA3B2; --text-4: #7E8798;
--accent: #A78BFA; --on-accent: #160F2E;
--status-perfect: #4CC38A; --status-playable: #7FB6FF; --status-in-progress: #D9A441;
--status-boots: #E07A5F; --status-not-playable: #5A6272;
--radius-s: 8px; --radius-m: 12px; --radius-pill: 999px;
--font-sans: "IBM Plex Sans"; --font-mono: "IBM Plex Mono";
```
Fonts bundled locally (OFL, `@fontsource/ibm-plex-*`), not loaded from Google at runtime.

## Accessibility
Real buttons/links, visible focus, 4.5:1 contrast, status never by color alone (always a
label), reduced-motion respected, all features keyboard + gamepad reachable.

## i18n
`paraglide-js` or `svelte-i18n` [decide]; English first; strings never inline in components.
