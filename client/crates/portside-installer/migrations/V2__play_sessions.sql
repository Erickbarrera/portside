-- One row per time a game ran. `ended_at` moves forward while it runs (a heartbeat), so a
-- crash or a forced quit still counts the time up to the last beat. Kept after uninstall.
CREATE TABLE play_sessions (
    id         INTEGER PRIMARY KEY,
    port_id    TEXT NOT NULL,
    version    TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at   INTEGER NOT NULL
) STRICT;

CREATE INDEX play_sessions_port ON play_sessions (port_id);
