CREATE TABLE content_gc_queue (
    entry_id INTEGER PRIMARY KEY,
    queued_at TEXT NOT NULL
);

CREATE TABLE content_gc_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    legacy_reconciled INTEGER NOT NULL DEFAULT 0 CHECK (legacy_reconciled IN (0, 1))
);

INSERT INTO content_gc_state (id, legacy_reconciled)
VALUES (1, 0);
