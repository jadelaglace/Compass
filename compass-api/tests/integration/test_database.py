"""Integration tests for database operations using a temporary SQLite file."""
import pytest
import pytest_asyncio
from src.db.database import Database, init_db


@pytest.mark.asyncio
async def test_get_entity_not_found(tmp_path):
    """get_entity should return None for non-existent ID."""
    db_path = tmp_path / "test.db"
    conn = await init_db(db_path)
    database = Database(db_path)
    database._conn = conn
    try:
        result = await database.get_entity("nonexistent-id")
        assert result is None
    finally:
        await conn.close()


@pytest.mark.asyncio
async def test_create_and_get_entity(tmp_path):
    """Creating an entity and reading it back should return matching data."""
    db_path = tmp_path / "test.db"
    conn = await init_db(db_path)
    database = Database(db_path)
    database._conn = conn
    try:
        now = "2026-04-11T00:00:00+00:00"
        entity_data = {
            "id": "test-entity-1",
            "file_path": "/vault/test.md",
            "vault_path": "test.md",
            "title": "Test Entity",
            "category": "Inbox",
            "created_at": now,
            "updated_at": now,
            "last_boosted_at": now,
            "metadata": {},
        }
        score_data = {
            "entity_id": "test-entity-1",
            "interest": 7.0,
            "strategy": 6.0,
            "consensus": 3.0,
            "final_score": 5.5,
            "updated_at": now,
            "last_boosted_at": now,
        }
        await database.create_entity_full(
            entity_data, score_data, ref_ids=[], event_type="created", event_trigger="test"
        )

        entity = await database.get_entity("test-entity-1")
        assert entity is not None
        assert entity["id"] == "test-entity-1"
        assert entity["title"] == "Test Entity"
        assert entity["category"] == "Inbox"
    finally:
        await conn.close()


@pytest.mark.asyncio
async def test_upsert_score_after_entity(tmp_path):
    """upsert_score should insert and then update a score record for an existing entity."""
    db_path = tmp_path / "test.db"
    conn = await init_db(db_path)
    database = Database(db_path)
    database._conn = conn
    try:
        now = "2026-04-11T00:00:00+00:00"
        # First create an entity (required for FK constraint on scores table)
        entity_data = {
            "id": "test-score-1",
            "file_path": "/vault/test.md",
            "vault_path": "test.md",
            "title": "Score Test",
            "category": "Inbox",
            "created_at": now,
            "updated_at": now,
            "last_boosted_at": now,
            "metadata": {},
        }
        score_data_initial = {
            "entity_id": "test-score-1",
            "interest": 5.0,
            "strategy": 5.0,
            "consensus": 0.0,
            "final_score": 5.0,
            "updated_at": now,
            "last_boosted_at": now,
        }
        await database.create_entity_full(
            entity_data, score_data_initial, ref_ids=[], event_type="created", event_trigger="test"
        )
        # Update the score
        score_data_updated = dict(score_data_initial)
        score_data_updated["final_score"] = 8.0
        await database.upsert_score(score_data_updated)

        # Verify
        async with database.conn.execute(
            "SELECT final_score FROM scores WHERE entity_id = ?", ("test-score-1",)
        ) as cur:
            rows = await cur.fetchall()
        assert len(rows) == 1
        assert rows[0]["final_score"] == 8.0
    finally:
        await conn.close()


@pytest.mark.asyncio
async def test_begin_commit_rollback(tmp_path):
    """begin/commit/rollback should control transaction state correctly."""
    db_path = tmp_path / "test.db"
    conn = await init_db(db_path)
    database = Database(db_path)
    database._conn = conn
    try:
        now = "2026-04-11T00:00:00+00:00"
        entity_data = {
            "id": "tx-test",
            "file_path": "/vault/tx.md",
            "vault_path": "tx.md",
            "title": "TX Test",
            "category": "Inbox",
            "created_at": now,
            "updated_at": now,
            "last_boosted_at": now,
            "metadata": {},
        }
        score_data = {
            "entity_id": "tx-test",
            "interest": 5.0,
            "strategy": 5.0,
            "consensus": 0.0,
            "final_score": 5.0,
            "updated_at": now,
            "last_boosted_at": now,
        }
        # Rollback test
        await database.begin()
        await database.upsert_entity(entity_data)
        await database.rollback()

        row = await database.get_entity("tx-test")
        assert row is None  # rolled back

        # Commit test
        await database.begin()
        await database.upsert_entity(entity_data)
        await database.commit()

        row = await database.get_entity("tx-test")
        assert row is not None  # committed
    finally:
        await conn.close()


@pytest.mark.asyncio
async def test_log_event(tmp_path):
    """log_event should insert a timeline event record."""
    db_path = tmp_path / "test.db"
    conn = await init_db(db_path)
    database = Database(db_path)
    database._conn = conn
    try:
        now = "2026-04-11T00:00:00+00:00"
        entity_data = {
            "id": "event-test",
            "file_path": "/vault/test.md",
            "vault_path": "test.md",
            "title": "Event Test",
            "category": "Inbox",
            "created_at": now,
            "updated_at": now,
            "last_boosted_at": now,
            "metadata": {},
        }
        score_data = {
            "entity_id": "event-test",
            "interest": 5.0,
            "strategy": 5.0,
            "consensus": 0.0,
            "final_score": 5.0,
            "updated_at": now,
            "last_boosted_at": now,
        }
        await database.create_entity_full(
            entity_data, score_data, ref_ids=[], event_type="created", event_trigger="test"
        )
        await database.log_event("event-test", "reviewed", trigger="test")

        async with database.conn.execute(
            "SELECT event_type FROM timeline_events WHERE entity_id = ?", ("event-test",)
        ) as cur:
            rows = await cur.fetchall()
        assert len(rows) == 2
    finally:
        await conn.close()
