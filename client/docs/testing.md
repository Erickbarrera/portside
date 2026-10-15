# Client testing

- **Unit**: each crate; property tests (`proptest`) for path resolution, snapshot diff,
  chunking, crypto round-trip.
- **Catalog contract** (`portside-api/tests/contract.rs`, ADR-0010): examples generated from
  `docs/specs/static-catalog/v1.json` must deserialize and round-trip through the Rust models.
- **Integration**: `wiremock` fake catalog host (`portside-api/tests/client.rs`,
  `portside-catalog/tests/catalog.rs`); fake GitHub API and downloads with synthetic zip/tar
  archives (`portside-installer/tests/install.rs`: resume, checksums, host allowlist, unsafe
  paths, unsigned consent). The disk-image test runs only on macOS; CI job `installer` runs
  the crate on macOS and Windows. `portside-launcher` tests build synthetic programs and
  `.app` bundles: launch paths, `Info.plist` lookup, exit codes, Stop reaching helper
  processes and killing a game that ignores it (process tests are Unix only; CI's
  `installer` job also runs the crate on macOS and Windows). `portside-sync` tests run two "devices" against one temporary vault folder (round trips, conflicts, plain readable files in the folder, port ids that can't escape the vault).
  `portside-atproto/tests/oauth.rs` runs the whole sign-in against a mock account host and
  sign-in server (PAR, DPoP nonces, token exchange, renewal, refused state);
  `portside-social/tests/social.rs` covers friends, chat through the proxy, invites, board
  grouping, thread stitching and posting chains. Synthetic accounts only; no live Bluesky
  calls in CI.
- **Golden manifests**: run resolver against every manifest in a fixtures copy of the
  manifests repo; assert one asset per build.
- **UI**: Vitest + Testing Library for components; Playwright against `pnpm --filter ui dev`
  with mocked IPC for flows (install, conflict picker, sign-in).
- **Manual matrix per release**: Win 11, macOS (arm64), Ubuntu LTS, Steam Deck, one Android
  phone. Checklist in release PR template.
- **Never** use real ROMs/assets; synthetic files with known hashes stand in.
