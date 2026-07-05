"""Tests for /graph endpoints."""
import pytest
from fastapi.testclient import TestClient
from unittest.mock import AsyncMock, MagicMock, patch

from tests.conftest import *  # imports db, mock_db, client fixtures


class TestNeighbors:
    """Test GET /graph/neighbors/{entity_id}"""

    def test_404_for_nonexistent(self, client):
        """Non-existent entity returns 404."""
        resp = client.get("/graph/neighbors/nonexistent-id")
        assert resp.status_code == 404

    def test_empty_returns_empty_lists(self, mock_db):
        """Entity with no refs returns empty nodes/edges arrays."""
        from src.db.database import Database, set_db

        db = Database()
        # Reconnect to the temp db used by the test
        from tests.conftest import _DB_PATH
        from pathlib import Path
        db._conn = mock_db._conn  # reuse the existing connection
        set_db(db)

        entity_id = "know-lone-001"
        now = "2026-05-05T00:00:00Z"
        mock_db.conn.execute(
            'INSERT INTO entities (id, title, entity_type, category, file_path, vault_path, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)',
            (entity_id, "Lone Entity", "knowledge", "Inbox", "/vault/Inbox/lone.md", "Inbox/lone.md", now, now),
        )
        mock_db.conn.execute(
            'INSERT INTO scores (entity_id, interest, strategy, consensus, final_score, updated_at) VALUES (?, ?, ?, ?, ?, ?)',
            (entity_id, 5.0, 5.0, 0.0, 5.0, now),
        )
        mock_db.commit()

        # Use the mock_db's connection via client fixture which has app context
        with patch("src.core.rust_client.rust_client") as mock_rust:
            mock_rust.compute_score = AsyncMock(return_value=MagicMock(
                final_score=5.0, decay_factor=0.95, days_elapsed=1.0,
            ))
            mock_rust.parse_refs = AsyncMock(return_value=MagicMock(refs=[]))
            from src.main import app
            with TestClient(app) as tc:
                resp = tc.get(f"/graph/neighbors/{entity_id}")
                assert resp.status_code == 200
                data = resp.json()
                assert data["nodes"] == []
                assert data["edges"] == []
                assert data["total_neighbors"] == 0

    def test_response_schema_shape(self, mock_db):
        """Response contains expected keys and types."""
        from src.db.database import Database, set_db

        entity_id = "know-schema-001"
        now = "2026-05-05T00:00:00Z"
        mock_db.conn.execute(
            'INSERT INTO entities (id, title, entity_type, category, file_path, vault_path, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)',
            (entity_id, "Schema Test", "knowledge", "Inbox", "/vault/Inbox/schema.md", "Inbox/schema.md", now, now),
        )
        mock_db.conn.execute(
            'INSERT INTO scores (entity_id, interest, strategy, consensus, final_score, updated_at) VALUES (?, ?, ?, ?, ?, ?)',
            (entity_id, 5.0, 5.0, 0.0, 5.0, now),
        )
        mock_db.commit()

        with patch("src.core.rust_client.rust_client") as mock_rust:
            mock_rust.compute_score = AsyncMock(return_value=MagicMock(
                final_score=5.0, decay_factor=0.95, days_elapsed=1.0,
            ))
            mock_rust.parse_refs = AsyncMock(return_value=MagicMock(refs=[]))
            from src.main import app
            with TestClient(app) as tc:
                resp = tc.get(f"/graph/neighbors/{entity_id}")
                assert resp.status_code == 200
                data = resp.json()
                assert "nodes" in data
                assert "edges" in data
                assert "total_neighbors" in data
                assert isinstance(data["nodes"], list)
                assert isinstance(data["edges"], list)
                assert isinstance(data["total_neighbors"], int)