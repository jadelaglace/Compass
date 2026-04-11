"""Integration tests for entity API endpoints."""
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient


class TestEntityEndpoints:
    def test_get_entity_not_found(self, client: TestClient):
        """GET /entities/{id} with non-existent ID should return 404."""
        response = client.get("/entities/nonexistent-id")
        assert response.status_code == 404

    def test_delete_entity_not_found(self, client: TestClient):
        """DELETE /entities/{id} with non-existent ID should return 404."""
        response = client.delete("/entities/nonexistent-id")
        assert response.status_code == 404
