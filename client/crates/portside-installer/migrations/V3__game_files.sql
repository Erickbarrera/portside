-- Where each port's game files were last copied from (docs/specs/manifest-format.md,
-- "game_files"), so an update copies them again from the same place. Kept after uninstall.
CREATE TABLE game_files (
    port_id TEXT PRIMARY KEY,
    store   TEXT NOT NULL,
    dir     TEXT NOT NULL
) STRICT;
