"""Integration tests for entity API endpoints."""
from unittest.mock import AsyncMock, MagicMock

import pytest
from fastapi.testclient import TestClient
from src.db.database import get_db


def _make_async_db_mock(get_entity_result=None):
    """Build a fully-async mock Database for endpoint tests."""
    mock = MagicMock()
    mock.get_entity = AsyncMock(return_value=get_entity_result)
    mock.begin = AsyncMock()
    mock.commit = AsyncMock()
    mock.rollback = AsyncMock()
    mock.execute = AsyncMock()
    return mock


class TestEntityEndpoints:
    def test_get_entity_not_found(self, client: TestClient):
        """GET /entities/{id} with non-existent ID should return 404."""
        app = client.app
        app.dependency_overrides[get_db] = lambda: _make_async_db_mock(get_entity_result=None)
        try:
            response = client.get("/entities/nonexistent-id")
            assert response.status_code == 404
        finally:
            app.dependency_overrides.clear()

    def test_delete_entity_not_found(self, client: TestClient):
        """DELETE /entities/{id} with non-existent ID should return 404."""
        app = client.app
        app.dependency_overrides[get_db] = lambda: _make_async_db_mock(get_entity_result=None)
        try:
            response = client.delete("/entities/nonexistent-id")
            assert response.status_code == 404
        finally:
            app.dependency_overrides.clear()
