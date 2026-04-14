"""Integration tests for FTS5 sync triggers (INSERT / DELETE / UPDATE on entities).

These tests directly validate the trigger fix in PR #39:
- OLD: INSERT INTO entities_fts(entities_fts, ...) VALUES('delete', ...) — invalid syntax
- NEW: DELETE FROM entities_fts WHERE id = old.id — direct DELETE supported by FTS5 plain tables

All three triggers (insert / delete / update) are exercised to confirm FTS index
stays in sync after each operation.
"""
import pytest
import pytest_asyncio
from pathlib import Path

from src.db.database import Database, init_db


async def _make_db(tmp_path: Path) -> Database:
    """Helper: create a fresh temp DB for each test."""
    db_path = tmp_path / "test_fts.db"
    conn = await init_db(db_path)
    database = Database(db_path)
    database._conn = conn
    return database


def _entity(entity_id: str, title: str, category: str = "Inbox") -> dict:
    now = "2026-04-14T00:00:00+00:00"
    return {
        "id": entity_id,
        "file_path": f"/vault/{entity_id}.md",
        "vault_path": f"{entity_id}.md",
        "title": title,
        "category": category,
        "created_at": now,
        "updated_at": now,
        "last_boosted_at": now,
        "metadata": {},
    }


def _score(entity_id: str) -> dict:
    now = "2026-04-14T00:00:00+00:00"
    return {
        "entity_id": entity_id,
        "interest": 5.0,
        "strategy": 5.0,
        "consensus": 0.0,
        "final_score": 5.0,
        "updated_at": now,
    }


async def _fts_search(database: Database, query: str) -> list[dict]:
    """Raw FTS5 search directly against entities_fts, bypassing the API layer."""
    cur = await database.conn.execute(
        f"SELECT id FROM entities_fts WHERE entities_fts MATCH '{query}' ORDER BY rank"
    )
    rows = await cur.fetchall()
    return [dict(r) for r in rows]


@pytest.mark.asyncio
async def test_fts_insert_trigger(tmp_path):
    """INSERT trigger: new entity should be immediately searchable via FTS5."""
    db = await _make_db(tmp_path)
    try:
        await db.create_entity_full(
            _entity("compass-v2", "Compass Version Two"),
            _score("compass-v2"),
            ref_ids=[],
        )

        results = await _fts_search(db, "Compass")
        ids = [r["id"] for r in results]
        assert "compass-v2" in ids, "Entity must appear in FTS after INSERT"
    finally:
        await db.conn.close()


@pytest.mark.asyncio
async def test_fts_delete_trigger(tmp_path):
    """DELETE trigger: deleted entity should no longer be searchable via FTS5.

    This is the core fix: the old 'INSERT INTO fts(fts, ...) VALUES(delete, ...)'
    syntax was invalid. The new 'DELETE FROM entities_fts WHERE id = old.id' is
    the correct FTS5 plain-table deletion approach.
    """
    db = await _make_db(tmp_path)
    try:
        await db.create_entity_full(
            _entity("to-delete", "Entity To Delete"),
            _score("to-delete"),
            ref_ids=[],
        )

        # Confirm it's indexed
        before = await _fts_search(db, "Delete")
        assert any(r["id"] == "to-delete" for r in before), "Entity must be indexed before DELETE"

        # Delete via raw SQL (mirrors what delete_entity endpoint does:
        # must remove FK-referencing rows first before deleting the entity)
        await db.begin()
        await db.conn.execute('DELETE FROM "references" WHERE source_id = ? OR target_id = ?', ("to-delete", "to-delete"))
        await db.conn.execute("DELETE FROM timeline_events WHERE entity_id = ?", ("to-delete",))
        await db.conn.execute("DELETE FROM scores WHERE entity_id = ?", ("to-delete",))
        await db.conn.execute("DELETE FROM entities WHERE id = ?", ("to-delete",))
        await db.commit()

        # FTS index must be updated by the trigger
        after = await _fts_search(db, "Delete")
        assert not any(r["id"] == "to-delete" for r in after), \
            "Entity must NOT appear in FTS after DELETE — trigger fix required"
    finally:
        await db.conn.close()


@pytest.mark.asyncio
async def test_fts_update_trigger(tmp_path):
    """UPDATE trigger: entity title change should be reflected in FTS5 index."""
    db = await _make_db(tmp_path)
    try:
        await db.create_entity_full(
            _entity("update-me", "Old Title Keyword"),
            _score("update-me"),
            ref_ids=[],
        )

        # Confirm old title is indexed
        before = await _fts_search(db, "OldTitle")
        # Note: FTS5 tokenises on whitespace; search for one unique word
        before_all = await _fts_search(db, "Keyword")
        assert any(r["id"] == "update-me" for r in before_all), "Old title must be indexed"

        # Update the entity title
        now = "2026-04-14T01:00:00+00:00"
        await db.begin()
        await db.conn.execute(
            "UPDATE entities SET title = ?, updated_at = ? WHERE id = ?",
            ("New Title Revised", now, "update-me"),
        )
        await db.commit()

        # Old keyword should be gone; new keyword should appear
        old_results = await _fts_search(db, "Keyword")
        new_results = await _fts_search(db, "Revised")
        assert not any(r["id"] == "update-me" for r in old_results), \
            "Old title keyword must NOT be in FTS after UPDATE"
        assert any(r["id"] == "update-me" for r in new_results), \
            "New title keyword must appear in FTS after UPDATE"
    finally:
        await db.conn.close()


@pytest.mark.asyncio
async def test_fts_multiple_entities_isolation(tmp_path):
    """Deleting one entity must not remove other entities from the FTS index."""
    db = await _make_db(tmp_path)
    try:
        await db.create_entity_full(
            _entity("keep-me", "Keeper Entity Alpha"),
            _score("keep-me"),
            ref_ids=[],
        )
        await db.create_entity_full(
            _entity("remove-me", "Removable Entity Beta"),
            _score("remove-me"),
            ref_ids=[],
        )

        # Delete only the second entity (FK order: refs → events → scores → entity)
        await db.begin()
        await db.conn.execute('DELETE FROM "references" WHERE source_id = ? OR target_id = ?', ("remove-me", "remove-me"))
        await db.conn.execute("DELETE FROM timeline_events WHERE entity_id = ?", ("remove-me",))
        await db.conn.execute("DELETE FROM scores WHERE entity_id = ?", ("remove-me",))
        await db.conn.execute("DELETE FROM entities WHERE id = ?", ("remove-me",))
        await db.commit()

        # "keep-me" must still be searchable
        results = await _fts_search(db, "Keeper")
        assert any(r["id"] == "keep-me" for r in results), \
            "Sibling entity must remain in FTS after another entity is deleted"

        # "remove-me" must be gone
        removed = await _fts_search(db, "Removable")
        assert not any(r["id"] == "remove-me" for r in removed), \
            "Deleted entity must not appear in FTS"
    finally:
        await db.conn.close()
