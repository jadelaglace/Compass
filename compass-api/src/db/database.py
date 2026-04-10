"""SQLite database manager — async aiosqlite, WAL mode."""
from __future__ import annotations

import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any, Optional

import aiosqlite

from src import config as cfg

SCHEMA_PATH = Path(__file__).parent / "schema.sql"


# ---- FTS query sanitizer ----

_ESCAPE_CHARS = str.maketrans({
    # SQL / FTS5 injection prevention
    '"': "",     # double-quote stripped (phrase delimiter, not SQL-safe here)
    "'": "",     # single-quote stripped (SQL string injection prevention)
    "(": "",
    ")": "",
    # FTS5 operator stripping (treat user input as plain tokens)
    "*": " ",    # prefix wildcard
    "-": " ",    # negation operator
    "+": " ",    # explicit AND operator
    "^": " ",    # XOR operator
    ":": " ",    # column filter
    "{": " ",
    "}": " ",
    "~": " ",    # approximate match
    "[": " ",
    "]": " ",
    "!": " ",    # NOT shortcut in some FTS5 dialects
})


def _escape_fts_query(raw: str) -> str:
    """Escape FTS5 special characters to treat them as literal search terms.

    FTS5 MATCH has its own query language (* prefix, OR/AND/NOT booleans,
    "phrase" delimiters, etc.).  User input passed raw can alter query semantics
    or cause errors.  Escape all FTS operators so they become plain tokens.
    """
    # Reject oversized input before any processing.
    if len(raw) > 200:
        raise ValueError("Query exceeds maximum length of 200 characters")
    # Strip leading/trailing whitespace; collapse internal runs of spaces.
    token = " ".join(raw.split())
    if not token:
        return '""'  # empty query → match-nothing (safer than match-all)
    # Escape special FTS characters; wrap as phrase to prevent tokenization issues.
    escaped = token.translate(_ESCAPE_CHARS)
    # Strip FTS5 boolean keywords (word-level, case-insensitive) so user
    # text is always treated as literal tokens, never as search operators.
    for kw in ("AND", "OR", "NOT"):
        escaped = re.sub(rf"\b{kw}\b", " ", escaped, flags=re.IGNORECASE)
    # Collapse any resulting double-spaces left by removed operators.
    return " ".join(escaped.split())


async def init_db(db_path: Optional[Path] = None) -> aiosqlite.Connection:
    """Open DB, set WAL mode, enable FK, run schema, return connection (caller owns it)."""
    path = db_path or cfg.DB_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = await aiosqlite.connect(str(path), isolation_level=None)
    conn.row_factory = aiosqlite.Row
    # WAL mode and foreign keys must be set immediately after connect,
    # before any schema statements — not inside the schema file.
    await conn.execute("PRAGMA journal_mode=WAL")
    await conn.execute("PRAGMA foreign_keys=ON")
    # Schema (FTS triggers, tables, indexes — no pragma)
    schema = SCHEMA_PATH.read_text()
    await conn.executescript(schema)
    return conn


class Database:
    """Thin async wrapper around aiosqlite — one connection per Database instance.

    Caller is responsible for explicit transaction control via begin()/commit()/rollback().
    This allows wrapping multiple operations atomically.
    """

    def __init__(self, db_path: Optional[Path] = None) -> None:
        self.db_path = db_path or cfg.DB_PATH
        self._conn: Optional[aiosqlite.Connection] = None

    async def connect(self) -> None:
        self._conn = await init_db(self.db_path)

    async def close(self) -> None:
        if self._conn:
            await self._conn.close()
            self._conn = None

    @property
    def conn(self) -> aiosqlite.Connection:
        if self._conn is None:
            raise RuntimeError("Database not connected — call .connect() first")
        return self._conn

    # ---- explicit transaction control ----

    async def begin(self) -> None:
        """Begin a transaction (EXCLUSIVE for writes)."""
        await self.conn.execute("BEGIN EXCLUSIVE")

    async def commit(self) -> None:
        """Commit the current transaction."""
        await self.conn.execute("COMMIT")

    async def rollback(self) -> None:
        """Roll back the current transaction."""
        await self.conn.execute("ROLLBACK")

    # ---- entities ----

    async def upsert_entity(self, data: dict[str, Any]) -> None:
        """Insert or update an entity. Caller manages transaction."""
        await self.conn.execute(
            """
            INSERT INTO entities (id, file_path, vault_path, title, category,
                                   created_at, updated_at, last_boosted_at,
                                   has_attachments, attachment_refs, metadata)
            VALUES (:id, :file_path, :vault_path, :title, :category,
                    :created_at, :updated_at, :last_boosted_at,
                    :has_attachments, :attachment_refs, :metadata)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                category = excluded.category,
                updated_at = excluded.updated_at,
                last_boosted_at = excluded.last_boosted_at,
                has_attachments = excluded.has_attachments,
                attachment_refs = excluded.attachment_refs,
                metadata = excluded.metadata
            """,
            {
                "id": data["id"],
                "file_path": data["file_path"],
                "vault_path": data["vault_path"],
                "title": data["title"],
                "category": data.get("category", "Inbox"),
                "created_at": data["created_at"],
                "updated_at": data["updated_at"],
                "last_boosted_at": data.get("last_boosted_at"),
                "has_attachments": int(data.get("has_attachments", False)),
                "attachment_refs": json.dumps(data.get("attachment_refs", [])),
                "metadata": json.dumps(data.get("metadata", {})),
            },
        )
        # NOTE: no commit() here — caller controls transaction

    async def upsert_score(self, data: dict[str, Any]) -> None:
        """Insert or update a score. Caller manages transaction."""
        await self.conn.execute(
            """
            INSERT INTO scores (entity_id, interest, strategy, consensus,
                                final_score, interest_half_life_days,
                                strategy_half_life_days, consensus_half_life_days,
                                manual_override, updated_at)
            VALUES (:entity_id, :interest, :strategy, :consensus,
                    :final_score, :interest_half_life_days,
                    :strategy_half_life_days, :consensus_half_life_days,
                    :manual_override, :updated_at)
            ON CONFLICT(entity_id) DO UPDATE SET
                interest = excluded.interest,
                strategy = excluded.strategy,
                consensus = excluded.consensus,
                final_score = excluded.final_score,
                manual_override = excluded.manual_override,
                updated_at = excluded.updated_at
            """,
            {
                "entity_id": data["entity_id"],
                "interest": data.get("interest", 5.0),
                "strategy": data.get("strategy", 5.0),
                "consensus": data.get("consensus", 0.0),
                "final_score": data.get("final_score", 0.0),
                "interest_half_life_days": data.get("interest_half_life_days", 30.0),
                "strategy_half_life_days": data.get("strategy_half_life_days", 365.0),
                "consensus_half_life_days": data.get("consensus_half_life_days", 60.0),
                "manual_override": int(data.get("manual_override", False)),
                "updated_at": data["updated_at"],
            },
        )
        # NOTE: no commit() here — caller controls transaction

    async def upsert_reference(self, source_id: str, target_id: str) -> None:
        """Insert a reference (source→target). FK check disabled intentionally:
        target may not exist yet (forward link to future note).
        Caller manages transaction."""
        now = datetime.utcnow().isoformat() + "Z"
        await self.conn.execute("PRAGMA foreign_keys=OFF")
        try:
            await self.conn.execute(
                'INSERT OR IGNORE INTO "references" (source_id, target_id, created_at) VALUES (?, ?, ?)',
                (source_id, target_id, now),
            )
        finally:
            await self.conn.execute("PRAGMA foreign_keys=ON")
        # NOTE: no commit() here — caller controls transaction

    async def log_event(
        self, entity_id: str, event_type: str, trigger: Optional[str] = None
    ) -> None:
        """Log a timeline event. Caller manages transaction."""
        now = datetime.utcnow().isoformat() + "Z"
        await self.conn.execute(
            "INSERT INTO timeline_events (entity_id, event_type, trigger, created_at) VALUES (?, ?, ?, ?)",
            (entity_id, event_type, trigger, now),
        )
        # NOTE: no commit() here — caller controls transaction

    # ---- atomic create (all-or-nothing) ----

    async def create_entity_full(
        self,
        entity_data: dict[str, Any],
        score_data: dict[str, Any],
        ref_ids: list[str],
        event_type: str = "created",
        event_trigger: str = "api",
    ) -> None:
        """Atomically create entity + score + references + event in one transaction.
        Raises on any failure; rolls back entirely on error."""
        await self.begin()
        try:
            await self.upsert_entity(entity_data)
            await self.upsert_score(score_data)
            for ref_id in ref_ids:
                await self.upsert_reference(entity_data["id"], ref_id)
            await self.log_event(entity_data["id"], event_type, event_trigger)
            await self.commit()
        except Exception:
            await self.rollback()
            raise

    # ---- read operations ----

    async def get_entity(self, entity_id: str) -> Optional[dict[str, Any]]:
        async with self.conn.execute(
            "SELECT * FROM entities WHERE id = ?", (entity_id,)
        ) as cur:
            row = await cur.fetchone()
        return dict(row) if row else None

    async def search_entities(
        self, query: str, limit: int = 20
    ) -> list[dict[str, Any]]:
        # FTS5 MATCH injection guard: _escape_fts_query strips:
        #   - single-quote (SQL string literal injection)
        #   - double-quote (phrase delimiter / SQL safety)
        #   - all FTS5 operators (*+-^:(){}~)
        #   - enforces 200-char max length
        # LIMIT is fully parameterised (? placeholder).
        # Note: MATCH clause must use string interpolation (not ? placeholder) —
        # SQLite FTS5 does not support parameterised MATCH expressions.
        safe_q = _escape_fts_query(query)
        cur = await self.conn.execute(
            f"""
            SELECT e.*, s.final_score
            FROM entities_fts f
            JOIN entities e ON e.id = f.id
            LEFT JOIN scores s ON s.entity_id = e.id
            WHERE entities_fts MATCH '{safe_q}'
            ORDER BY rank
            LIMIT ?
            """,
            (limit,),
        )
        rows = await cur.fetchall()
        return [dict(r) for r in rows]

    async def get_top_entities(
        self, limit: int = 20, category: Optional[str] = None
    ) -> list[dict[str, Any]]:
        # LEFT JOIN + ORDER BY nullable column: filter NULLs for stable sort.
        # SQLite's NULL ordering is implementation-defined; exclude rows
        # without a score to keep ordering deterministic.
        q = """
            SELECT e.*, s.final_score
            FROM entities e
            LEFT JOIN scores s ON s.entity_id = e.id
            WHERE s.final_score IS NOT NULL
        """
        params: list[Any] = []
        if category:
            q += " AND e.category = ?"
            params.append(category)
        q += " ORDER BY s.final_score DESC LIMIT ?"
        params.append(limit)
        cur = await self.conn.execute(q, params)
        rows = await cur.fetchall()
        return [dict(r) for r in rows]

    async def get_references(
        self, entity_id: str
    ) -> tuple[list[str], list[str]]:
        """Return (outgoing_refs, incoming_refs) ids for entity."""
        out_cur = await self.conn.execute(
            'SELECT target_id FROM "references" WHERE source_id = ?',
            (entity_id,),
        )
        out_rows = await out_cur.fetchall()

        in_cur = await self.conn.execute(
            'SELECT source_id FROM "references" WHERE target_id = ?',
            (entity_id,),
        )
        in_rows = await in_cur.fetchall()

        return (
            [r["target_id"] for r in out_rows],
            [r["source_id"] for r in in_rows],
        )


# ---- FastAPI dependency ----

_db_instance: Database | None = None


def set_db(db: Database) -> None:
    global _db_instance
    _db_instance = db


def get_db() -> Database:
    """FastAPI dependency — returns the shared DB instance."""
    if _db_instance is None:
        raise RuntimeError("Database not initialized — call set_db() first")
    return _db_instance
