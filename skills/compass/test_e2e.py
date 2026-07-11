#!/usr/bin/env python3
"""
Compass skill + compass-core HTTP API 端到端测试。

运行前确保已在 compass-core 编译好二进制：
    cd compass-core && cargo build --release

运行：python3 test_e2e.py
"""

import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.parse
import urllib.request

COMPASS_DIR = os.path.dirname(os.path.abspath(__file__))
COMPASS_SCRIPT = os.path.join(COMPASS_DIR, "compass")
REPO_ROOT = os.path.dirname(os.path.dirname(COMPASS_DIR))
COMPASS_CORE = os.path.join(REPO_ROOT, "compass-core")

# Windows 上尽量使用编译后的二进制；否则 cargo run
DEFAULT_BINARY = os.path.join(COMPASS_CORE, "target", "release", "compass.exe")
if not os.path.exists(DEFAULT_BINARY):
    DEFAULT_BINARY = os.path.join(COMPASS_CORE, "target", "debug", "compass.exe")
TEST_API_TOKEN = "compass-e2e-token"


def wait_for_server(url, timeout=30, token=None):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            request = urllib.request.Request(url)
            if token:
                request.add_header("Authorization", f"Bearer {token}")
            with urllib.request.urlopen(request, timeout=1) as resp:
                if resp.status == 200:
                    return True
        except Exception:
            time.sleep(0.2)
    return False


def compass_cli(*args, env=None):
    """调用 skill CLI，返回 (stdout, stderr, rc)。"""
    cmd = [sys.executable, COMPASS_SCRIPT] + list(args)
    result = subprocess.run(
        cmd, capture_output=True, text=True, encoding="utf-8", env=env
    )
    return result.stdout, result.stderr, result.returncode


def compass_render_stdin(raw, action, env):
    result = subprocess.run(
        [sys.executable, COMPASS_SCRIPT, "render", f"action={action}"],
        input=raw,
        capture_output=True,
        text=True,
        encoding="utf-8",
        env=env,
    )
    return result.stdout, result.stderr, result.returncode


def http_json(base_url, path, method="GET", payload=None, timeout=5):
    def decode(body):
        if not body:
            return None
        try:
            return json.loads(body)
        except json.JSONDecodeError:
            return body

    data = None if payload is None else json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(
        f"{base_url}{path}", data=data, method=method
    )
    request.add_header("Accept", "application/json")
    token = os.environ.get("COMPASS_API_TOKEN", "").strip()
    if token:
        request.add_header("Authorization", f"Bearer {token}")
    if data is not None:
        request.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            body = response.read().decode("utf-8")
            return response.status, decode(body)
    except urllib.error.HTTPError as error:
        try:
            body = error.read().decode("utf-8")
            return error.code, decode(body)
        finally:
            error.close()


class TestE2ESkillAgainstApi(unittest.TestCase):
    server_proc = None
    tmpdir = None
    env = None
    vault = None

    @classmethod
    def setUpClass(cls):
        cls.tmpdir = tempfile.mkdtemp(prefix="compass-e2e-")
        vault = os.path.join(cls.tmpdir, "vault")
        cls.vault = vault
        os.makedirs(vault)

        # 准备几个测试笔记
        cls.note_a = os.path.join(vault, "know-000001.md")
        with open(cls.note_a, "w", encoding="utf-8") as f:
            f.write(
                "---\n"
                "id: know-000001\n"
                "title: Nash Equilibrium\n"
                "layer: knowledge\n"
                "status: active\n"
                "score:\n"
                "  interest: 80.0\n"
                "  strategy: 90.0\n"
                "  consensus: 70.0\n"
                "  composite: 81.0\n"
                "  updated_at: '2026-07-09T00:00:00Z'\n"
                "  last_boosted_at: '2026-07-09T00:00:00Z'\n"
                "  access_count: 0\n"
                "---\n"
                "Nash equilibrium is a core concept in game theory.\n"
            )

        cls.note_b = os.path.join(vault, "dir-000001.md")
        with open(cls.note_b, "w", encoding="utf-8") as f:
            f.write(
                "---\n"
                "id: dir-000001\n"
                "title: Strategic Direction\n"
                "layer: direction\n"
                "status: active\n"
                "score:\n"
                "  interest: 95.0\n"
                "  strategy: 95.0\n"
                "  consensus: 95.0\n"
                "  composite: 95.0\n"
                "  updated_at: '2026-07-09T00:00:00Z'\n"
                "  last_boosted_at: '2026-07-09T00:00:00Z'\n"
                "  access_count: 0\n"
                "---\n"
                "This direction guides all strategic decisions.\n"
            )

        # 临时配置（TOML 字符串用正斜杠避免反斜杠转义问题）
        cfg_path = os.path.join(cls.tmpdir, "compass.toml")
        vault_fwd = vault.replace("\\", "/")
        with open(cfg_path, "w", encoding="utf-8") as f:
            f.write(
                f'vault_path = "{vault_fwd}"\n'
                "port = 18080\n"
                f'auth_token = "{TEST_API_TOKEN}"\n'
                "\n[weights]\n"
                "interest = 0.40\n"
                "strategy = 0.35\n"
                "consensus = 0.25\n"
                "\n[decay]\n"
                "daily_rate = 0.98\n"
                "floor = 0.5\n"
                "boost_protection_days = 3\n"
                "direction_layer_factor = 0.5\n"
            )

        cls.env = os.environ.copy()
        cls.env["COMPASS_CONFIG"] = cfg_path
        cls.env["COMPASS_API_URL"] = "http://localhost:18080"
        cls.env["COMPASS_API_TOKEN"] = TEST_API_TOKEN
        cls.previous_test_token = os.environ.get("COMPASS_API_TOKEN")
        os.environ["COMPASS_API_TOKEN"] = TEST_API_TOKEN

        # 启动服务器
        if os.path.exists(DEFAULT_BINARY):
            cmd = [DEFAULT_BINARY]
            cls.server_cwd = COMPASS_CORE
        else:
            cmd = ["cargo", "run", "--quiet"]
            cls.server_cwd = COMPASS_CORE

        cls.server_proc = subprocess.Popen(
            cmd,
            cwd=cls.server_cwd,
            env=cls.env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
        )

        if not wait_for_server("http://localhost:18080/health", timeout=60, token=TEST_API_TOKEN):
            cls._kill_server()
            raise RuntimeError("compass server failed to start")

    def setUp(self):
        self.created_files = []

    def tearDown(self):
        for path in self.created_files:
            entity_id = os.path.splitext(os.path.basename(path))[0]
            try:
                os.remove(path)
            except FileNotFoundError:
                continue

            # FileWatcher is asynchronous; wait until SQLite no longer exposes
            # the entity before the next test starts.
            deadline = time.time() + 5
            while time.time() < deadline:
                try:
                    request = urllib.request.Request(
                        f"http://localhost:18080/entities/{entity_id}"
                    )
                    request.add_header("Authorization", f"Bearer {TEST_API_TOKEN}")
                    with urllib.request.urlopen(request, timeout=1):
                        pass
                except urllib.error.HTTPError as exc:
                    is_absent = exc.code == 404
                    exc.close()
                    if is_absent:
                        break
                except urllib.error.URLError:
                    break
                time.sleep(0.1)

    @classmethod
    def tearDownClass(cls):
        cls._kill_server()
        if cls.previous_test_token is None:
            os.environ.pop("COMPASS_API_TOKEN", None)
        else:
            os.environ["COMPASS_API_TOKEN"] = cls.previous_test_token
        if cls.tmpdir and os.path.exists(cls.tmpdir):
            shutil.rmtree(cls.tmpdir, ignore_errors=True)

    @classmethod
    def _kill_server(cls):
        if cls.server_proc and cls.server_proc.poll() is None:
            if sys.platform == "win32":
                cls.server_proc.terminate()
                try:
                    cls.server_proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    cls.server_proc.kill()
            else:
                cls.server_proc.send_signal(signal.SIGTERM)
                try:
                    cls.server_proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    cls.server_proc.kill()

    # ---- action + render 两阶段闭环 ----

    def test_health(self):
        out, err, rc = compass_cli("render", 'raw={"status":"ok"}', "action=create", env=self.env)
        self.assertEqual(rc, 0)

    def test_search_and_render(self):
        raw, err, rc = compass_cli("search", "q=Nash", "limit=5", env=self.env)
        self.assertEqual(rc, 0, f"search failed: {err}")
        data = json.loads(raw)
        self.assertEqual(len(data), 1)
        self.assertEqual(data[0]["id"], "know-000001")
        self.assertIn("composite", data[0])
        self.assertIn("layer", data[0])

        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=search", env=self.env
        )
        self.assertEqual(rc, 0, f"render failed: {err}")
        self.assertIn("Nash Equilibrium", rendered)
        self.assertIn(f"{data[0]['composite']:.1f}", rendered)

    def test_top_and_render(self):
        raw, err, rc = compass_cli("top", "limit=5", env=self.env)
        self.assertEqual(rc, 0, f"top failed: {err}")
        data = json.loads(raw)
        self.assertGreaterEqual(len(data), 2)
        # 默认按 composite 降序，direction 95 应在第一
        self.assertEqual(data[0]["id"], "dir-000001")

        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=top", env=self.env
        )
        self.assertEqual(rc, 0, f"render failed: {err}")
        self.assertIn("Strategic Direction", rendered)

    def test_top_layer_filter(self):
        raw, err, rc = compass_cli("top", "limit=5", "layer=knowledge", env=self.env)
        self.assertEqual(rc, 0, f"top layer failed: {err}")
        data = json.loads(raw)
        self.assertEqual(len(data), 1)
        self.assertEqual(data[0]["id"], "know-000001")

    def test_top_category_alias(self):
        raw, err, rc = compass_cli("top", "limit=5", "category=direction", env=self.env)
        self.assertEqual(rc, 0, f"top category alias failed: {err}")
        data = json.loads(raw)
        self.assertEqual(len(data), 1)
        self.assertEqual(data[0]["id"], "dir-000001")

    def test_get_and_render(self):
        raw, err, rc = compass_cli("get", "id=know-000001", env=self.env)
        self.assertEqual(rc, 0, f"get failed: {err}")
        data = json.loads(raw)
        self.assertEqual(data["id"], "know-000001")
        self.assertEqual(data["score"]["composite"], 81.0)

        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=get", env=self.env
        )
        self.assertEqual(rc, 0, f"render failed: {err}")
        self.assertIn("Nash Equilibrium", rendered)
        self.assertIn("81.0", rendered)

    def test_feed_and_render(self):
        raw, err, rc = compass_cli("feed", "limit=5", env=self.env)
        self.assertEqual(rc, 0, f"feed failed: {err}")
        data = json.loads(raw)
        self.assertGreaterEqual(len(data), 2)

        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=feed", env=self.env
        )
        self.assertEqual(rc, 0, f"render failed: {err}")
        self.assertIn("今日焦点", rendered)

    def test_feed_all_modes(self):
        expected_first = {
            "explore": "dir-000001",
            "strategic": "dir-000001",
        }
        for mode in ("explore", "consolidate", "strategic"):
            raw, err, rc = compass_cli("feed", "limit=5", f"mode={mode}", env=self.env)
            self.assertEqual(rc, 0, f"feed {mode} failed: {err}")
            data = json.loads(raw)
            self.assertGreaterEqual(len(data), 2)
            if mode in expected_first:
                self.assertEqual(data[0]["id"], expected_first[mode])

            rendered, err, rc = compass_cli(
                "render", f"raw={raw}", "action=feed", env=self.env
            )
            self.assertEqual(rc, 0, f"render feed {mode} failed: {err}")
            self.assertIn("今日焦点", rendered)

    def test_context_and_render(self):
        raw, err, rc = compass_cli(
            "context", "task=game theory", "top_k=2", env=self.env
        )
        self.assertEqual(rc, 0, f"context failed: {err}")
        data = json.loads(raw)
        self.assertGreaterEqual(len(data["context"]), 1)

        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=context", env=self.env
        )
        self.assertEqual(rc, 0, f"render failed: {err}")
        self.assertIn("Nash", rendered)

    def test_phase4_actions_vault_sqlite_and_render(self):
        # Tag suggestions are generated by the API, then accept writes only the
        # tag metadata back to the real temporary Vault.
        raw, err, rc = compass_cli(
            "create",
            "title=Tag Source",
            "layer=knowledge",
            "content=game theory Nash equilibrium repeated concepts",
            env=self.env,
        )
        self.assertEqual(rc, 0, f"tag source create failed: {err}")
        tag_source = json.loads(raw)
        tag_source_path = os.path.join(self.vault, tag_source["file_path"])
        self.created_files.append(tag_source_path)

        raw, err, rc = compass_cli("tags", f"id={tag_source['id']}", env=self.env)
        self.assertEqual(rc, 0, f"tag suggestions failed: {err}")
        tag_data = json.loads(raw)
        self.assertGreaterEqual(len(tag_data["suggestions"]), 2)
        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=tags", env=self.env
        )
        self.assertEqual(rc, 0, f"tag render failed: {err}")
        self.assertIn("标签建议", rendered)

        accepted_id = tag_data["suggestions"][0]["suggestion_id"]
        rejected_id = tag_data["suggestions"][1]["suggestion_id"]
        raw, err, rc = compass_cli("accept_tag", f"suggestion_id={accepted_id}", env=self.env)
        self.assertEqual(rc, 0, f"tag accept failed: {err}")
        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=accept_tag", env=self.env
        )
        self.assertEqual(rc, 0)
        self.assertIn("建议已接受", rendered)
        raw, err, rc = compass_cli(
            "accept_tag", f"suggestion_id={accepted_id}", env=self.env
        )
        self.assertEqual(rc, 0, f"repeated tag accept failed: {err}")
        self.assertEqual(json.loads(raw)["status"], "accepted")
        _, err, rc = compass_cli(
            "reject_tag", f"suggestion_id={accepted_id}", env=self.env
        )
        self.assertEqual(rc, 1)
        self.assertIn("409", err)
        with open(tag_source_path, encoding="utf-8") as f:
            updated_note = f.read()
        self.assertIn("tags:", updated_note)

        raw, err, rc = compass_cli("reject_tag", f"id={rejected_id}", env=self.env)
        self.assertEqual(rc, 0, f"tag reject failed: {err}")
        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=reject_tag", env=self.env
        )
        self.assertEqual(rc, 0)
        self.assertIn("建议已拒绝", rendered)

        # A temporary source note exercises the related recommendation write path.
        raw, err, rc = compass_cli(
            "create",
            "title=Related Source",
            "layer=knowledge",
            "content=game theory Nash equilibrium shared concepts",
            env=self.env,
        )
        self.assertEqual(rc, 0, f"source create failed: {err}")
        source = json.loads(raw)
        source_id = source["id"]
        source_path = os.path.join(self.vault, source["file_path"])
        self.created_files.append(source_path)

        raw, err, rc = compass_cli("related", f"id={source_id}", "limit=10", env=self.env)
        self.assertEqual(rc, 0, f"related failed: {err}")
        related_data = json.loads(raw)
        target = next(
            item for item in related_data["suggestions"] if item["id"] == "know-000001"
        )
        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=related", env=self.env
        )
        self.assertEqual(rc, 0, f"related render failed: {err}")
        self.assertIn("Nash Equilibrium", rendered)

        raw, err, rc = compass_cli(
            "accept_related", f"suggestion_id={target['suggestion_id']}", env=self.env
        )
        self.assertEqual(rc, 0, f"related accept failed: {err}")
        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=accept_related", env=self.env
        )
        self.assertEqual(rc, 0)
        self.assertIn("建议已接受", rendered)
        raw, err, rc = compass_cli(
            "accept_related", f"suggestion_id={target['suggestion_id']}", env=self.env
        )
        self.assertEqual(rc, 0, f"repeated related accept failed: {err}")
        self.assertEqual(json.loads(raw)["status"], "accepted")
        _, err, rc = compass_cli(
            "reject_related", f"suggestion_id={target['suggestion_id']}", env=self.env
        )
        self.assertEqual(rc, 1)
        self.assertIn("409", err)
        with open(source_path, encoding="utf-8") as f:
            self.assertIn("[[know-000001]]", f.read())

        raw, err, rc = compass_cli(
            "weekly",
            "from=2026-01-01",
            "to=2027-01-01",
            "tz=Asia/Shanghai",
            env=self.env,
        )
        self.assertEqual(rc, 0, f"weekly report failed: {err}")
        report = json.loads(raw)
        self.assertEqual(report["tz"], "Asia/Shanghai")
        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=weekly", env=self.env
        )
        self.assertEqual(rc, 0, f"weekly render failed: {err}")
        self.assertIn("周报", rendered)

    def test_phase4_http_contract_boundaries(self):
        base_url = self.env["COMPASS_API_URL"]

        status, _ = http_json(base_url, "/feed?mode=invalid")
        self.assertEqual(status, 422)
        status, _ = http_json(
            base_url, "/agent/context", method="POST", payload={"task": ""}
        )
        self.assertEqual(status, 422)
        status, _ = http_json(
            base_url, "/agent/context", method="POST", payload={}
        )
        self.assertEqual(status, 422)

        status, _ = http_json(
            base_url,
            "/entities/missing/tag-suggestions",
            method="POST",
            payload={},
        )
        self.assertEqual(status, 404)
        status, _ = http_json(
            base_url,
            "/tag-suggestions/missing/accept",
            method="POST",
        )
        self.assertEqual(status, 404)

        status, lexical = http_json(
            base_url,
            "/entities/know-000001/tag-suggestions",
            method="POST",
            payload={},
        )
        self.assertEqual(status, 200)
        candidate = {
            "tag": "decision-science",
            "confidence": 0.8,
            "reason": "agent candidate",
            "source": "agent",
            "content_hash": lexical["content_hash"],
        }
        status, _ = http_json(
            base_url,
            "/entities/know-000001/tag-suggestions",
            method="POST",
            payload={"candidates": [candidate]},
        )
        self.assertEqual(status, 422)
        candidate["algorithm_version"] = "agent-v1"
        candidates = [
            {**candidate, "tag": f"candidate-{index}"}
            for index in range(21)
        ]
        status, _ = http_json(
            base_url,
            "/entities/know-000001/tag-suggestions",
            method="POST",
            payload={"candidates": candidates},
        )
        self.assertEqual(status, 422)

        status, related = http_json(
            base_url, "/entities/know-000001/related?limit=5"
        )
        self.assertEqual(status, 200)
        self.assertTrue(related["suggestions"])
        self.assertTrue(related["suggestions"][0]["reasons"])
        self.assertNotIn(
            "know-000001",
            {item["id"] for item in related["suggestions"]},
        )
        status, _ = http_json(base_url, "/entities/missing/related")
        self.assertEqual(status, 404)

        valid_weekly = (
            "/reports/weekly?from=2026-07-01&to=2026-07-12"
            "&tz=Asia%2FShanghai"
        )
        status, first = http_json(base_url, valid_weekly)
        self.assertEqual(status, 200)
        status, second = http_json(base_url, valid_weekly)
        self.assertEqual(status, 200)
        self.assertEqual(first, second)
        for query in (
            "from=2026-07-01&to=2026-07-12",
            "from=2026-07-01&to=2026-07-12&tz=Bogus%2FZone",
            "from=notdate&to=2026-07-12&tz=Asia%2FShanghai",
            "from=2026-07-12&to=2026-07-01&tz=Asia%2FShanghai",
        ):
            status, _ = http_json(base_url, f"/reports/weekly?{query}")
            self.assertEqual(status, 422)

    def test_runtime_stability_under_mixed_http_load(self):
        raw, err, rc = compass_cli(
            "create",
            "title=Runtime Stability",
            "layer=knowledge",
            "content=中文评分引擎 mixed load regression",
            env=self.env,
        )
        self.assertEqual(rc, 0, f"create failed: {err}")
        entity = json.loads(raw)
        self.created_files.append(os.path.join(self.vault, entity["file_path"]))
        base_url = self.env["COMPASS_API_URL"]

        # More than 40 mixed TCP requests, including successful writes and
        # intentionally rejected requests, exceed the original failure report
        # while keeping the regular E2E suite practical on Windows.
        for iteration in range(10):
            status, health = http_json(base_url, "/health")
            self.assertEqual((status, health["status"]), (200, "ok"))

            status, results = http_json(
                base_url,
                "/search?q=" + urllib.parse.quote("评分"),
            )
            self.assertEqual(status, 200)
            self.assertTrue(any(item["id"] == entity["id"] for item in results))

            status, _ = http_json(base_url, "/feed?mode=explore&limit=5")
            self.assertEqual(status, 200)

            status, _ = http_json(
                base_url,
                "/agent/context",
                method="POST",
                payload={"task": "mixed load", "top_k": 3},
            )
            self.assertEqual(status, 200)

            if iteration % 5 == 0:
                status, _ = http_json(base_url, "/feed?mode=invalid")
                self.assertEqual(status, 422)
                status, _ = http_json(
                    base_url,
                    f"/entities/{entity['id']}/access",
                    method="PATCH",
                    payload={"depth": "read"},
                )
                self.assertEqual(status, 200)

        self.assertIsNone(self.server_proc.poll(), "Compass exited during mixed load")
        status, health = http_json(base_url, "/health")
        self.assertEqual((status, health["status"]), (200, "ok"))

    def test_create_and_score_and_access(self):
        # create
        raw, err, rc = compass_cli(
            "create",
            "title=Test Note",
            "layer=knowledge",
            "content=body content",
            "interest=70",
            "strategy=80",
            "consensus=60",
            env=self.env,
        )
        self.assertEqual(rc, 0, f"create failed: {err}")
        data = json.loads(raw)
        self.assertIn("id", data)
        new_id = data["id"]
        self.created_files.append(os.path.join(self.vault, data["file_path"]))
        self.assertAlmostEqual(data["composite"], 71.0, places=6)

        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=create", env=self.env
        )
        self.assertEqual(rc, 0)
        self.assertIn("Test Note", rendered)

        # score
        raw, err, rc = compass_cli(
            "score", f"id={new_id}", "interest=95", env=self.env
        )
        self.assertEqual(rc, 0, f"score failed: {err}")
        data = json.loads(raw)
        self.assertAlmostEqual(data["score"]["composite"], 81.0, places=6)

        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=score", env=self.env
        )
        self.assertEqual(rc, 0)
        self.assertIn("81.0", rendered)

        # access
        raw, err, rc = compass_cli(
            "access", f"id={new_id}", "depth=study", env=self.env
        )
        self.assertEqual(rc, 0, f"access failed: {err}")
        data = json.loads(raw)
        self.assertAlmostEqual(data["score"]["interest"], 98.0, places=6)
        self.assertEqual(data["score"]["access_count"], 1)

        rendered, err, rc = compass_cli(
            "render", f"raw={raw}", "action=access", env=self.env
        )
        self.assertEqual(rc, 0)
        self.assertIn("访问已记录", rendered)

    def test_access_depths_and_invalid_depth(self):
        raw, err, rc = compass_cli(
            "create",
            "title=Depth Test",
            "layer=knowledge",
            "content=access depth coverage",
            "interest=10",
            "strategy=20",
            "consensus=30",
            env=self.env,
        )
        self.assertEqual(rc, 0, f"create failed: {err}")
        data = json.loads(raw)
        entity_id = data["id"]
        self.created_files.append(os.path.join(self.vault, data["file_path"]))

        expected = {
            "glance": (10.0, 20.0, 30.1, 1),
            "read": (11.0, 20.0, 30.6, 2),
            "study": (14.0, 20.0, 31.6, 3),
            "apply": (16.0, 25.0, 33.6, 4),
        }
        for depth, (interest, strategy, consensus, count) in expected.items():
            raw, err, rc = compass_cli(
                "access", f"id={entity_id}", f"depth={depth}", env=self.env
            )
            self.assertEqual(rc, 0, f"access {depth} failed: {err}")
            score = json.loads(raw)["score"]
            self.assertAlmostEqual(score["interest"], interest, places=6)
            self.assertAlmostEqual(score["strategy"], strategy, places=6)
            self.assertAlmostEqual(score["consensus"], consensus, places=6)
            self.assertEqual(score["access_count"], count)

        _, err, rc = compass_cli(
            "access", f"id={entity_id}", "depth=invalid", env=self.env
        )
        self.assertEqual(rc, 1)
        self.assertIn("invalid", err.lower())

    def test_render_stdin(self):
        raw = json.dumps(
            [{"id": "know-000001", "title": "Nash", "composite": 81.0}],
            ensure_ascii=False,
        )
        rendered, err, rc = compass_render_stdin(raw, "search", self.env)
        self.assertEqual(rc, 0, f"stdin render failed: {err}")
        self.assertIn("Nash", rendered)
        self.assertIn("81.0", rendered)

    def test_get_not_found(self):
        out, err, rc = compass_cli("get", "id=nonexistent", env=self.env)
        self.assertEqual(rc, 1)
        self.assertIn("not found", err.lower())

    def test_search_no_match(self):
        raw, err, rc = compass_cli("search", "q=xyzabc123", env=self.env)
        self.assertEqual(rc, 0)
        data = json.loads(raw)
        self.assertEqual(data, [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
