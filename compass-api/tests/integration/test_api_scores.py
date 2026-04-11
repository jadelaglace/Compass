"""Integration tests for scores API endpoint."""
import pytest
from fastapi.testclient import TestClient


class TestScoresEndpoint:
    def test_update_score_not_found(self, client: TestClient):
        """POST /scores/update for non-existent entity should return 404."""
        response = client.post("/scores/update", json={
            "entity_id": "nonexistent",
            "interest": 7.0,
        })
        assert response.status_code == 404
