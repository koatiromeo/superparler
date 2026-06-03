CREATE TABLE IF NOT EXISTS transcriptions (
    id           TEXT PRIMARY KEY NOT NULL,
    created_at   TEXT NOT NULL,
    text         TEXT NOT NULL,
    duration_ms  INTEGER NOT NULL,
    engine       TEXT NOT NULL,
    language     TEXT NOT NULL,
    target_app   TEXT,
    enhanced     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_transcriptions_created_at ON transcriptions (created_at DESC);
