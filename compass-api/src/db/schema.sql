-- Compass SQLite Schema
-- SQLite WAL mode, FTS5 enabled

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

-- Entities (one per Obsidian note)
CREATE TABLE IF NOT EXISTS entities (
    id              TEXT PRIMARY KEY,
    file_path       TEXT NOT NULL UNIQUE,
    vault_path      TEXT NOT NULL,
    title           TEXT NOT NULL,
    category        TEXT NOT NULL DEFAULT 'Inbox',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    last_boosted_at TEXT,
    has_attachments INTEGER DEFAULT 0,
    attachment_refs TEXT,
    metadata        TEXT
);

-- Scores per entity
CREATE TABLE IF NOT EXISTS scores (
    entity_id                   TEXT PRIMARY KEY REFERENCES entities(id),
    interest                     REAL DEFAULT 5.0,
    strategy                      REAL DEFAULT 5.0,
    consensus                     REAL DEFAULT 0.0,
    final_score                  REAL DEFAULT 0.0,
    interest_half_life_days       REAL DEFAULT 30.0,
    strategy_half_life_days       REAL DEFAULT 365.0,
    consensus_half_life_days      REAL DEFAULT 60.0,
    manual_override               INTEGER DEFAULT 0,
    updated_at                    TEXT NOT NULL
);

-- Bidirectional references
CREATE TABLE IF NOT EXISTS "references" (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id   TEXT NOT NULL REFERENCES entities(id),
    target_id   TEXT NOT NULL REFERENCES entities(id),
    created_at  TEXT NOT NULL,
    UNIQUE(source_id, target_id)
);

-- Timeline events
CREATE TABLE IF NOT EXISTS timeline_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id   TEXT NOT NULL REFERENCES entities(id),
    event_type  TEXT NOT NULL,
    trigger     TEXT,
    created_at  TEXT NOT NULL
);

-- FTS5 full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
    id,
    title,
    content,
    category,
    content='entities',
    content_rowid='rowid'
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_entities_category ON entities(category);
CREATE INDEX IF NOT EXISTS idx_scores_final_score ON scores(final_score DESC);
CREATE INDEX IF NOT EXISTS idx_references_source ON "references"(source_id);
CREATE INDEX IF NOT EXISTS idx_references_target ON "references"(target_id);
CREATE INDEX IF NOT EXISTS idx_timeline_entity ON timeline_events(entity_id);
