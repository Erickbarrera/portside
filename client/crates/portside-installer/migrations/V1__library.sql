-- Installed ports. `record` is the JSON of `Installed`; the columns beside it are for lookups.
CREATE TABLE installs (
    port_id      TEXT NOT NULL,
    version      TEXT NOT NULL,
    title        TEXT NOT NULL,
    installed_at INTEGER NOT NULL,
    record       TEXT NOT NULL,
    PRIMARY KEY (port_id, version)
) STRICT;

-- The version each port launches; older rows are kept for rollback.
CREATE TABLE current_version (
    port_id TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    FOREIGN KEY (port_id, version) REFERENCES installs (port_id, version) ON DELETE CASCADE
) STRICT;
