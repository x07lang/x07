CREATE TABLE drift_log (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id),
  detected_at TEXT NOT NULL DEFAULT (datetime('now')),
  expected_hash TEXT NOT NULL,
  actual_hash TEXT NOT NULL,
  resolved INTEGER NOT NULL DEFAULT 0
);
