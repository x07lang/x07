CREATE TABLE __x07_migrations (
  id TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE projects (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  schema_hash TEXT NOT NULL DEFAULT ''
);
