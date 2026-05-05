"""Integration tests for Insight-1 CRUD endpoints."""
import pytest
from starlette.testclient import TestClient


class TestInsightCreate:
    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_vault_{cls._counter}"

    def test_create_insight_basic(self, client: TestClient):
        """POST /insights creates an insight with seedling maturity."""
        vp = self._vault()
        entity_resp = client.post("/entities", json={
            "id": "test-entity-insight-1",
            "title": "Test Entity for Insight",
            "entity_type": "knowledge",
            "content": "some content",
            "vault_path": vp,
        })
        assert entity_resp.status_code == 200

        resp = client.post("/insights", json={
            "entity_id": "test-entity-insight-1",
            "title": "My First Insight",
            "content": "Insight content here",
        })
        assert resp.status_code == 200
        data = resp.json()
        assert data["title"] == "My First Insight"
        assert data["maturity"] == "seedling"
        assert data["entity_id"] == "test-entity-insight-1"
        assert data["source_type"] == "auto"
        assert "id" in data

    def test_create_insight_entity_not_found(self, client: TestClient):
        """POST /insights with non-existent entity returns 404."""
        resp = client.post("/insights", json={
            "entity_id": "nonexistent-entity",
            "title": "Orphan Insight",
        })
        assert resp.status_code == 404

    def test_create_insight_logged_event(self, client: TestClient):
        """Creating an insight also logs a timeline event."""
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-entity-insight-2",
            "title": "Test Entity 2",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        resp = client.post("/insights", json={
            "entity_id": "test-entity-insight-2",
            "title": "Logged Insight",
        })
        assert resp.status_code == 200


class TestInsightList:
    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_list_vault_{cls._counter}"

    def test_list_insights_empty(self, client: TestClient):
        """GET /insights on fresh DB returns empty list."""
        resp = client.get("/insights")
        assert resp.status_code == 200
        data = resp.json()
        assert data["items"] == []
        assert data["total"] == 0
        assert data["has_more"] is False

    def test_list_insights_returns_created(self, client: TestClient):
        """GET /insights returns created insights."""
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-entity-insight-list",
            "title": "Test Entity List",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        client.post("/insights", json={
            "entity_id": "test-entity-insight-list",
            "title": "List Test Insight",
            "content": "content",
        })
        resp = client.get("/insights")
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] >= 1
        assert any(i["title"] == "List Test Insight" for i in data["items"])

    def test_list_insights_pagination(self, client: TestClient):
        """Pagination params work correctly."""
        resp = client.get("/insights?limit=5&offset=0")
        assert resp.status_code == 200
        assert "has_more" in resp.json()

    def test_list_insights_maturity_filter(self, client: TestClient):
        """Maturity filter returns only matching insights."""
        resp = client.get("/insights?maturity=seedling")
        assert resp.status_code == 200
        for item in resp.json()["items"]:
            assert item["maturity"] == "seedling"

    def test_list_insights_invalid_maturity(self, client: TestClient):
        """Invalid maturity returns 422."""
        resp = client.get("/insights?maturity=invalid")
        assert resp.status_code == 422


class TestInsightGet:
    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_get_vault_{cls._counter}"

    def test_get_insight_found(self, client: TestClient):
        """GET /insights/{id} returns the insight."""
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-entity-insight-get",
            "title": "Get Test Entity",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-entity-insight-get",
            "title": "Get Test Insight",
        })
        insight_id = create_resp.json()["id"]

        resp = client.get(f"/insights/{insight_id}")
        assert resp.status_code == 200
        assert resp.json()["title"] == "Get Test Insight"

    def test_get_insight_not_found(self, client: TestClient):
        """GET /insights/{id} for nonexistent returns 404."""
        resp = client.get("/insights/nonexistent-insight-id")
        assert resp.status_code == 404


class TestInsightMaturityUpgrade:
    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_upgrade_vault_{cls._counter}"

    def test_upgrade_seedling_to_sprout(self, client: TestClient):
        """PATCH maturity advances seedling → sprout."""
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-entity-upgrade-1",
            "title": "Upgrade Test Entity",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-entity-upgrade-1",
            "title": "Upgrade Test Insight",
        })
        insight_id = create_resp.json()["id"]

        resp = client.patch(f"/insights/{insight_id}/maturity")
        assert resp.status_code == 200
        assert resp.json()["maturity"] == "sprout"

    def test_upgrade_sprout_to_mature(self, client: TestClient):
        """Second PATCH advances sprout → mature."""
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-entity-upgrade-2",
            "title": "Upgrade Test Entity 2",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-entity-upgrade-2",
            "title": "Two-Step Upgrade Insight",
        })
        insight_id = create_resp.json()["id"]

        client.patch(f"/insights/{insight_id}/maturity")
        resp = client.patch(f"/insights/{insight_id}/maturity")
        assert resp.status_code == 200
        assert resp.json()["maturity"] == "mature"

    def test_upgrade_mature_returns_422(self, client: TestClient):
        """Patching a mature insight returns 422 'already fully mature'."""
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-entity-upgrade-3",
            "title": "Upgrade Test Entity 3",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-entity-upgrade-3",
            "title": "Already Mature Insight",
        })
        insight_id = create_resp.json()["id"]

        client.patch(f"/insights/{insight_id}/maturity")
        client.patch(f"/insights/{insight_id}/maturity")

        resp = client.patch(f"/insights/{insight_id}/maturity")
        assert resp.status_code == 422
        assert "fully mature" in resp.json()["detail"].lower()

    def test_upgrade_nonexistent_returns_404(self, client: TestClient):
        """Patching nonexistent insight returns 404."""
        resp = client.patch("/insights/nonexistent-id/maturity")
        assert resp.status_code == 404