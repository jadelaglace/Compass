-- Compass SQLite Schema
-- FTS5 + sync triggers + indexes
-- NOTE: PRAGMA journal_mode=WAL and PRAGMA foreign_keys=ON are set in init_db(),
--       not here, to avoid pragma parsing issues in executescript().

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
    strategy                     REAL DEFAULT 5.0,
    consensus                    REAL DEFAULT 0.0,
    final_score                  REAL DEFAULT 0.0,
    interest_half_life_days      REAL DEFAULT 30.0,
    strategy_half_life_days      REAL DEFAULT 365.0,
    consensus_half_life_days     REAL DEFAULT 60.0,
    manual_override              INTEGER DEFAULT 0,
    updated_at                   TEXT NOT NULL
);

-- Bidirectional references
-- FK on source_id enforced, target_id intentionally unconstrained
-- (a note may reference a future note not yet in the system)
CREATE TABLE IF NOT EXISTS "references" (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id   TEXT NOT NULL REFERENCES entities(id),
    target_id   TEXT NOT NULL,
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
    category
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_entities_category ON entities(category);
CREATE INDEX IF NOT EXISTS idx_scores_final_score ON scores(final_score DESC);
CREATE INDEX IF NOT EXISTS idx_references_source ON "references"(source_id);
CREATE INDEX IF NOT EXISTS idx_references_target ON "references"(target_id);
CREATE INDEX IF NOT EXISTS idx_timeline_entity ON timeline_events(entity_id);

-- FTS5 sync triggers
-- NOTE: FTS5 plain tables support DELETE statements directly
CREATE TRIGGER IF NOT EXISTS entities_fts_insert
AFTER INSERT ON entities BEGIN
    INSERT INTO entities_fts(id, title, category)
    VALUES (new.id, new.title, new.category);
END;

CREATE TRIGGER IF NOT EXISTS entities_fts_delete
AFTER DELETE ON entities BEGIN
    DELETE FROM entities_fts WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS entities_fts_update
AFTER UPDATE ON entities BEGIN
    DELETE FROM entities_fts WHERE id = old.id;
    INSERT INTO entities_fts(id, title, category)
    VALUES (new.id, new.title, new.category);
END;
