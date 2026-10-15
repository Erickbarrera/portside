# Spec: API surface (retired)

Portside has no hosted API (ADR-0015, 2026-10-14). The catalog the API used to serve is now
a set of static files: see [static-catalog.md](static-catalog.md).

The endpoints this page described for accounts, devices, saves, telemetry, the AppView and
chat were never built. Their replacements are open questions for Milestones 3 to 5 (see
ADR-0015, "Consequences"):
- Sign-in: AT Protocol OAuth from the app as a public client; client metadata is a static
  file on the website.
- Saves: snapshot index and head pointer in the user's own storage
  ([save-sync-protocol.md](save-sync-protocol.md), v2 pending).
- Reviews, compat reports, boards, chat: [TBD: decide before Milestone 3].

## Changelog
- 2026-10-13 — draft; catalog routes implemented (Milestone 1); OpenAPI copy for the client
  contract (ADR-0010).
- 2026-10-14 — retired with the backend (ADR-0015). The OpenAPI copy at
  `docs/specs/openapi/v1.json` is replaced by `docs/specs/static-catalog/v1.json`.
