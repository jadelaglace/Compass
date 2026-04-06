"""SQLite database manager — async aiosqlite, WAL mode."""
from __future__ import annotations

import asyncio
import json
from pathlib import Path
from typing import Any, Optional

import aiosqlite

from src import config as cfg

SCHEMA_PATH = Path(__file__).parent / "schema.sql"


async def init_db(db_path: Optional[Path] = None) -> aiosqlite.Connection:
    """Open DB, run schema, return connection (caller owns it)."""
    path = db_path or cfg.DB_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = await aiosqlite.connect(str(path))
    conn.row_factory = aiosqlite.Row
    # WAL + foreign keys
    await conn.execute("PRAGMA journal_mode=WAL")
    await conn.execute("PRAGMA foreign_keys=ON")
    # Schema
    schema = SCHEMA_PATH.read_text()
    await conn.executescript(schema)
    await conn.commit()
    return conn


class Database:
    """Thin async wrapper around aiosqlite — one connection per Database instance."""

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

    # ---- entities ----

    async def upsert_entity(self, data: dict[str, Any]) -> None:
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
        await self.conn.commit()

    async def upsert_score(self, data: dict[str, Any]) -> None:
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
        await self.conn.commit()

    async def get_entity(self, entity_id: str) -> Optional[dict[str, Any]]:
        async with self.conn.execute(
            "SELECT * FROM entities WHERE id = ?", (entity_id,)
        ) as cur:
            row = await cur.fetchone()
        return dict(row) if row else None

    async def search_entities(
        self, query: str, limit: int = 20
    ) -> list[dict[str, Any]]:
        cur = await self.conn.execute(
            """
            SELECT e.*, s.final_score
            FROM entities_fts f
            JOIN entities e ON e.id = f.id
            LEFT JOIN scores s ON s.entity_id = e.id
            WHERE entities_fts MATCH ?
            ORDER BY rank
            LIMIT ?
            """,
            (query, limit),
        )
        rows = await cur.fetchall()
        return [dict(r) for r in rows]

    async def get_top_entities(
        self, limit: int = 20, category: Optional[str] = None
    ) -> list[dict[str, Any]]:
        q = """
            SELECT e.*, s.final_score
            FROM entities e
            LEFT JOIN scores s ON s.entity_id = e.id
        """
        params: list[Any] = []
        if category:
            q += " WHERE e.category = ?"
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

    async def upsert_reference(self, source_id: str, target_id: str) -> None:
        from datetime import datetime
        now = datetime.utcnow().isoformat() + "Z"
        # Disable FK check — target may not exist yet (backlink to future note)
        await self.conn.execute("PRAGMA foreign_keys=OFF")
        await self.conn.execute(
            'INSERT OR IGNORE INTO "references" (source_id, target_id, created_at) VALUES (?, ?, ?)',
            (source_id, target_id, now),
        )
        await self.conn.execute("PRAGMA foreign_keys=ON")
        await self.conn.commit()

    async def log_event(
        self, entity_id: str, event_type: str, trigger: Optional[str] = None
    ) -> None:
        from datetime import datetime
        now = datetime.utcnow().isoformat() + "Z"
        await self.conn.execute(
            "INSERT INTO timeline_events (entity_id, event_type, trigger, created_at) VALUES (?, ?, ?, ?)",
            (entity_id, event_type, trigger, now),
        )
        await self.conn.commit()


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
