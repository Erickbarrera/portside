-- Local copy of the catalog (client/docs/architecture.md, "Local data").
-- `summary` is the PortSummary JSON exactly as the API sent it; the other columns exist for
-- filtering and ordering.
CREATE TABLE ports (
  id         TEXT PRIMARY KEY,
  title      TEXT NOT NULL,
  type       TEXT NOT NULL,
  status     TEXT NOT NULL,
  stars      INTEGER,
  summary    TEXT NOT NULL
) STRICT;

CREATE TABLE port_os (
  port_id TEXT NOT NULL REFERENCES ports(id) ON DELETE CASCADE,
  os      TEXT NOT NULL,
  PRIMARY KEY (port_id, os)
) STRICT;

CREATE INDEX ports_by_stars ON ports (stars DESC, id);

-- Title search. Kept in sync by the triggers below.
CREATE VIRTUAL TABLE ports_fts USING fts5(title, content='ports', content_rowid='rowid');

CREATE TRIGGER ports_ai AFTER INSERT ON ports BEGIN
  INSERT INTO ports_fts(rowid, title) VALUES (new.rowid, new.title);
END;
CREATE TRIGGER ports_ad AFTER DELETE ON ports BEGIN
  INSERT INTO ports_fts(ports_fts, rowid, title) VALUES ('delete', old.rowid, old.title);
END;
CREATE TRIGGER ports_au AFTER UPDATE ON ports BEGIN
  INSERT INTO ports_fts(ports_fts, rowid, title) VALUES ('delete', old.rowid, old.title);
  INSERT INTO ports_fts(rowid, title) VALUES (new.rowid, new.title);
END;

-- Sync bookkeeping: rev, etag, last_synced_at.
CREATE TABLE sync_state (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;
