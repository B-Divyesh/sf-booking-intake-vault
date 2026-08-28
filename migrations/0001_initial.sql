PRAGMA foreign_keys = ON;

CREATE TABLE workspaces (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  business_name TEXT NOT NULL,
  passphrase_hash TEXT NOT NULL,
  timezone TEXT NOT NULL,
  region TEXT NOT NULL,
  deletion_days INTEGER NOT NULL CHECK (deletion_days BETWEEN 1 AND 90),
  created_at TEXT NOT NULL
);

CREATE TABLE sessions (
  token_hash TEXT PRIMARY KEY,
  expires_at TEXT NOT NULL
);

CREATE TABLE form_fields (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  field_type TEXT NOT NULL,
  required INTEGER NOT NULL DEFAULT 0,
  visibility TEXT NOT NULL CHECK (visibility IN ('worker', 'admin')),
  sort_order INTEGER NOT NULL,
  options_json TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE bookings (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  delete_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'new' CHECK (status IN ('new', 'assigned', 'complete')),
  worker_name TEXT
);

CREATE TABLE responses (
  booking_id TEXT NOT NULL REFERENCES bookings(id) ON DELETE CASCADE,
  field_id TEXT NOT NULL,
  label_snapshot TEXT NOT NULL,
  visibility_snapshot TEXT NOT NULL CHECK (visibility_snapshot IN ('worker', 'admin')),
  value TEXT NOT NULL,
  sort_order INTEGER NOT NULL,
  PRIMARY KEY (booking_id, field_id)
);

CREATE TABLE worker_tokens (
  token_hash TEXT PRIMARY KEY,
  booking_id TEXT NOT NULL REFERENCES bookings(id) ON DELETE CASCADE,
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX bookings_delete_at_idx ON bookings(delete_at);
CREATE INDEX worker_tokens_booking_idx ON worker_tokens(booking_id);
