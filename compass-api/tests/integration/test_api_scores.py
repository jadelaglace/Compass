"""Integration tests for scores API endpoint."""
from unittest.mock import AsyncMock, MagicMock

import pytest
from fastapi.testclient import TestClient
from src.db.database import get_db


class TestScoresEndpoint:
    def test_update_score_not_found(self, client: TestClient):
        """POST /scores/update for non-existent entity should return 404."""
        mock_db = MagicMock()
        mock_db.get_entity = AsyncMock(return_value=None)

        app = client.app
        app.dependency_overrides[get_db] = lambda: mock_db
        try:
            response = client.post("/scores/update", json={
                "entity_id": "nonexistent",
                "interest": 7.0,
            })
            assert response.status_code == 404
        finally:
            app.dependency_overrides.clear()
