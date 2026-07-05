"""Integration tests for agent API endpoints."""
from unittest.mock import AsyncMock, MagicMock, patch

import pytest
from fastapi.testclient import TestClient
from src.db.database import get_db


def _make_score_mock(final_score: float = 5.0):
    m = MagicMock()
    m.final_score = final_score
    m.decay_factor = 0.95
    m.days_elapsed = 1.0
    return m


class TestAgentEndpoints:
    def test_get_context_no_candidates(self, client: TestClient):
        """POST /agent/context with no matching entities returns empty context."""
        mock_db = MagicMock()
        mock_db.search_entities = AsyncMock(return_value=[])

        app = client.app
        app.dependency_overrides[get_db] = lambda: mock_db
        try:
            response = client.post("/agent/context", json={"task": "nothing matches", "top_k": 5})
            assert response.status_code == 200
            data = response.json()
            assert data["context"] == []
            assert data["suggested_entities"] == []
            assert "No entities found" in data["reasoning"]
        finally:
            app.dependency_overrides.clear()

    def test_get_context_with_candidates(self, client: TestClient):
        """POST /agent/context returns scored and sorted top-k entities."""
        fake_entities = [
            {
                "id": f"entity-{i}",
                "title": f"Entity {i}",
                "category": "Inbox",
                "interest": float(10 - i),  # descending: 9, 8, 7...
                "strategy": 5.0,
                "consensus": 0.0,
                "last_boosted_at": "2026-04-10T12:00:00+00:00",
            }
            for i in range(4)
        ]
        mock_db = MagicMock()
        mock_db.search_entities = AsyncMock(return_value=fake_entities)

        app = client.app
        app.dependency_overrides[get_db] = lambda: mock_db

        with patch("src.api.agent.rust_client") as mock_rust:
            # Return scores matching interest: entity-0=9.0, entity-1=8.0, etc.
            def score_for(**kwargs):
                interest = kwargs.get("interest", 5.0)
                return _make_score_mock(final_score=interest)

            mock_rust.compute_score = MagicMock(side_effect=score_for)
            response = client.post("/agent/context", json={"task": "test", "top_k": 2})
            assert response.status_code == 200
            data = response.json()
            # top_k=2 → 2 in context, 2 in suggestions
            assert len(data["context"]) == 2
            assert len(data["suggested_entities"]) == 2
            # entity-0 (interest=10.0) should be first
            assert data["context"][0]["id"] == "entity-0"
            assert data["context"][0]["final_score"] == 10.0
            assert "score" in data["reasoning"].lower()

    def test_get_context_top_k_respected(self, client: TestClient):
        """top_k controls how many appear in context vs suggestions."""
        fake_entities = [
            {
                "id": f"entity-{i}",
                "title": f"Entity {i}",
                "category": "Inbox",
                "interest": float(i),
                "strategy": 5.0,
                "consensus": 0.0,
                "last_boosted_at": "2026-04-10T12:00:00+00:00",
            }
            for i in range(6)
        ]
        mock_db = MagicMock()
        mock_db.search_entities = AsyncMock(return_value=fake_entities)

        app = client.app
        app.dependency_overrides[get_db] = lambda: mock_db

        with patch("src.api.agent.rust_client") as mock_rust:
            mock_rust.compute_score = MagicMock(return_value=_make_score_mock(5.0))
            response = client.post("/agent/context", json={"task": "test", "top_k": 3})
            assert response.status_code == 200
            data = response.json()
            assert len(data["context"]) == 3
            assert len(data["suggested_entities"]) == 3

    def test_get_context_schema_fields(self, client: TestClient):
        """Response contains required ContextResponse fields."""
        fake_entities = [
            {
                "id": "test-entity",
                "title": "Test Entity",
                "category": "Inbox",
                "interest": 5.0,
                "strategy": 5.0,
                "consensus": 0.0,
                "last_boosted_at": "2026-04-10T12:00:00+00:00",
            },
        ]
        mock_db = MagicMock()
        mock_db.search_entities = AsyncMock(return_value=fake_entities)

        app = client.app
        app.dependency_overrides[get_db] = lambda: mock_db

        with patch("src.api.agent.rust_client") as mock_rust:
            mock_rust.compute_score = MagicMock(return_value=_make_score_mock(5.0))
            response = client.post("/agent/context", json={"task": "test", "top_k": 1})
            assert response.status_code == 200
            ctx = response.json()["context"][0]
            assert "id" in ctx
            assert "title" in ctx
            assert "final_score" in ctx
