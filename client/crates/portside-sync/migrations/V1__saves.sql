-- This device's view of cloud saves (docs/specs/save-sync-protocol.md).
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE games (
    port_id     TEXT PRIMARY KEY,
    -- Snapshot this device last matched, and its files as JSON {path: {blob, size, mtime}}.
    base_id     TEXT,
    base_files  TEXT NOT NULL DEFAULT '{}',
    -- Folder the player chose for this game's saves; wins over the manifest.
    custom_root TEXT,
    -- The manifest's `saves` section, kept so sync works offline.
    rules       TEXT,
    -- Unix seconds of the last finished sync, and the last error.
    last_sync   INTEGER,
    last_error  TEXT
);
