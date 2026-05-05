"""Integration tests for Insight-1/2/3 endpoints."""
from starlette.testclient import TestClient


class TestInsightCreate:
    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_vault_{cls._counter}"

    def test_create_insight_basic(self, client: TestClient):
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
        resp = client.post("/insights", json={
            "entity_id": "nonexistent-entity",
            "title": "Orphan Insight",
        })
        assert resp.status_code == 404

    def test_create_insight_logged_event(self, client: TestClient):
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
        resp = client.get("/insights")
        assert resp.status_code == 200
        data = resp.json()
        assert data["items"] == []
        assert data["total"] == 0
        assert data["has_more"] is False

    def test_list_insights_returns_created(self, client: TestClient):
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
        resp = client.get("/insights?limit=5&offset=0")
        assert resp.status_code == 200
        assert "has_more" in resp.json()

    def test_list_insights_maturity_filter(self, client: TestClient):
        resp = client.get("/insights?maturity=seedling")
        assert resp.status_code == 200
        for item in resp.json()["items"]:
            assert item["maturity"] == "seedling"

    def test_list_insights_invalid_maturity(self, client: TestClient):
        resp = client.get("/insights?maturity=invalid")
        assert resp.status_code == 422


class TestInsightGet:
    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_get_vault_{cls._counter}"

    def test_get_insight_found(self, client: TestClient):
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
        resp = client.get("/insights/nonexistent-insight-id")
        assert resp.status_code == 404


class TestInsightMaturityUpgrade:
    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_upgrade_vault_{cls._counter}"

    def test_upgrade_seedling_to_sprout(self, client: TestClient):
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
        resp = client.patch("/insights/nonexistent-id/maturity")
        assert resp.status_code == 404


class TestInsightEvolution:
    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_evolve_vault_{cls._counter}"

    def test_evolve_requires_mature_insight(self, client: TestClient):
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-evolve-1",
            "title": "Evolve Test Entity",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-evolve-1",
            "title": "Seedling Insight",
        })
        insight_id = create_resp.json()["id"]

        resp = client.get(f"/insights/{insight_id}/evolve")
        assert resp.status_code == 200
        data = resp.json()
        assert data["evolved"] is False
        assert data["detail"] == "Insight not yet mature"

    def test_evolve_entity_from_mature_insight(self, client: TestClient):
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-evolve-2",
            "title": "Evolve Test Entity 2",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-evolve-2",
            "title": "Evolve Insight 2",
        })
        insight_id = create_resp.json()["id"]

        client.patch(f"/insights/{insight_id}/maturity")
        client.patch(f"/insights/{insight_id}/maturity")

        resp = client.get(f"/insights/{insight_id}/evolve")
        assert resp.status_code == 200
        data = resp.json()
        assert data["evolved"] is True
        assert data["entity_maturity"] == "sprout"

    def test_evolve_twice_full_journey(self, client: TestClient):
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-evolve-3",
            "title": "Evolve Test Entity 3",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-evolve-3",
            "title": "Full Journey Insight",
        })
        insight_id = create_resp.json()["id"]

        client.patch(f"/insights/{insight_id}/maturity")
        client.patch(f"/insights/{insight_id}/maturity")

        resp1 = client.get(f"/insights/{insight_id}/evolve")
        assert resp1.json()["evolved"] is True
        assert resp1.json()["entity_maturity"] == "sprout"

        resp2 = client.get(f"/insights/{insight_id}/evolve")
        assert resp2.json()["evolved"] is True
        assert resp2.json()["entity_maturity"] == "mature"

        resp3 = client.get(f"/insights/{insight_id}/evolve")
        assert resp3.json()["evolved"] is False
        assert "already" in resp3.json()["detail"].lower()

    def test_evolve_insight_not_found(self, client: TestClient):
        resp = client.get("/insights/nonexistent-id/evolve")
        assert resp.status_code == 404


class TestInsightExport:
    """Tests for export endpoints: GET /insights?format=export, GET /insights/{id}/export, GET /insights/entity/{id}/export"""

    _counter = 0

    @classmethod
    def _vault(cls):
        cls._counter += 1
        return f"/tmp/insight_export_vault_{cls._counter}"

    def test_export_all_via_query_param_json(self, client: TestClient):
        """Export all insights via ?format=export query param (GET /insights with format=export)."""
        resp = client.get("/insights?format=export")
        assert resp.status_code == 200
        data = resp.json()
        assert data["format"] == "json"
        assert "items" in data
        assert "total" in data

    def test_export_all_via_query_param_markdown(self, client: TestClient):
        resp = client.get("/insights?format=markdown")
        assert resp.status_code == 200
        data = resp.json()
        assert data["format"] == "markdown"
        assert "content" in data
        assert "# Insights Export" in data["content"]

    def test_export_with_maturity_filter(self, client: TestClient):
        resp = client.get("/insights?format=export&maturity=seedling")
        assert resp.status_code == 200
        data = resp.json()
        assert data["format"] == "json"

    def test_export_single_insight_json(self, client: TestClient):
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-export-1",
            "title": "Export Test Entity",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-export-1",
            "title": "Export Test Insight",
            "content": "Some content for export",
        })
        insight_id = create_resp.json()["id"]

        resp = client.get(f"/insights/{insight_id}/export?format=json")
        assert resp.status_code == 200
        data = resp.json()
        assert data["format"] == "json"
        assert data["item"]["title"] == "Export Test Insight"

    def test_export_single_insight_markdown(self, client: TestClient):
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-export-2",
            "title": "Export Test Entity 2",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        create_resp = client.post("/insights", json={
            "entity_id": "test-export-2",
            "title": "Markdown Insight",
            "content": "Markdown content",
        })
        insight_id = create_resp.json()["id"]

        resp = client.get(f"/insights/{insight_id}/export?format=markdown")
        assert resp.status_code == 200
        data = resp.json()
        assert data["format"] == "markdown"
        assert "Markdown Insight" in data["content"]

    def test_export_entity_insights(self, client: TestClient):
        vp = self._vault()
        client.post("/entities", json={
            "id": "test-export-entity-1",
            "title": "Export Entity Test",
            "entity_type": "knowledge",
            "vault_path": vp,
        })
        client.post("/insights", json={
            "entity_id": "test-export-entity-1",
            "title": "Entity Insight 1",
        })
        client.post("/insights", json={
            "entity_id": "test-export-entity-1",
            "title": "Entity Insight 2",
        })

        resp = client.get("/insights/entity/test-export-entity-1/export?format=json")
        assert resp.status_code == 200
        data = resp.json()
        assert data["format"] == "json"
        assert data["entity_id"] == "test-export-entity-1"

    def test_export_invalid_format(self, client: TestClient):
        resp = client.get("/insights?format=yaml")
        assert resp.status_code == 422

    def test_export_entity_not_found(self, client: TestClient):
        resp = client.get("/insights/entity/nonexistent-entity/export")
        assert resp.status_code == 404