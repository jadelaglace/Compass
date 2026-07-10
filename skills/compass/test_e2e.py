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
import urllib.request

COMPASS_DIR = os.path.dirname(os.path.abspath(__file__))
COMPASS_SCRIPT = os.path.join(COMPASS_DIR, "compass")
REPO_ROOT = os.path.dirname(os.path.dirname(COMPASS_DIR))
COMPASS_CORE = os.path.join(REPO_ROOT, "compass-core")

# Windows 上尽量使用编译后的二进制；否则 cargo run
DEFAULT_BINARY = os.path.join(COMPASS_CORE, "target", "release", "compass.exe")
if not os.path.exists(DEFAULT_BINARY):
    DEFAULT_BINARY = os.path.join(COMPASS_CORE, "target", "debug", "compass.exe")


def wait_for_server(url, timeout=30):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1) as resp:
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

        if not wait_for_server("http://localhost:18080/health", timeout=60):
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
                    with urllib.request.urlopen(
                        f"http://localhost:18080/entities/{entity_id}", timeout=1
                    ):
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
        self.assertIn("81.0", rendered)

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
