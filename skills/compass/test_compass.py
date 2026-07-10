#!/usr/bin/env python3
"""
Compass skill 自动化测试：覆盖 CLI render 与参数解析。

运行：python3 test_compass.py
"""

import json
import os
import subprocess
import sys
import unittest

COMPASS_DIR = os.path.dirname(os.path.abspath(__file__))
COMPASS_SCRIPT = os.path.join(COMPASS_DIR, "compass")


def run(cmd):
    result = subprocess.run(
        cmd, capture_output=True, text=True, encoding="utf-8"
    )
    return result.stdout, result.stderr, result.returncode


class TestRenderCLI(unittest.TestCase):
    def test_render_search_list(self):
        raw = json.dumps(
            [
                {
                    "id": "know000001",
                    "title": "Nash Equilibrium",
                    "snippet": "core concept",
                    "layer": "knowledge",
                    "composite": 85.7,
                }
            ]
        )
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=search"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("Nash Equilibrium", out)
        self.assertIn("85.7", out)
        self.assertIn("[knowledge]", out)
        self.assertIn("core concept", out)

    def test_render_search_empty(self):
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", "raw=[]", "action=search"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("没有找到", out)

    def test_render_search_legacy_dict(self):
        raw = json.dumps(
            {
                "results": [
                    {"id": "know000001", "title": "Nash", "composite": 70.0, "layer": "case"}
                ]
            }
        )
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=search"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("Nash", out)
        self.assertIn("70.0", out)

    def test_render_top_list(self):
        raw = json.dumps(
            [{"id": "know000001", "title": "Nash", "layer": "knowledge", "composite": 85.7}]
        )
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=top"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("## Top 1", out)
        self.assertIn("85.7", out)

    def test_render_top_empty(self):
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", "raw=[]", "action=top"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("没有找到", out)

    def test_render_get(self):
        raw = json.dumps(
            {
                "id": "know000001",
                "title": "Nash Equilibrium",
                "layer": "knowledge",
                "status": "active",
                "score": {
                    "interest": 80.0,
                    "strategy": 90.0,
                    "consensus": 70.0,
                    "composite": 81.0,
                    "access_count": 5,
                },
                "refs": ["know000002"],
            }
        )
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=get"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("Nash Equilibrium", out)
        self.assertIn("knowledge", out)
        self.assertIn("active", out)
        self.assertIn("81.0", out)
        self.assertIn("[[know000002]]", out)

    def test_render_feed_list(self):
        raw = json.dumps([{"id": "know000001", "title": "Nash", "composite": 85.7}])
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=feed"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("今日焦点", out)
        self.assertIn("Nash", out)

    def test_render_feed_dict(self):
        raw = json.dumps(
            {
                "top_inbox": [{"id": "k1", "title": "A", "composite": 90.0}],
                "recently_updated": [{"id": "k2", "title": "B", "composite": 80.0}],
                "strategic": [{"id": "k3", "title": "C", "composite": 85.0}],
            }
        )
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=feed"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("今日焦点", out)
        self.assertIn("最近更新", out)
        self.assertIn("战略焦点", out)

    def test_render_context(self):
        raw = json.dumps(
            {
                "context": [
                    {
                        "id": "know000001",
                        "title": "Nash",
                        "content": "core concept",
                        "composite": 85.7,
                    }
                ],
                "reasoning": "recalled 1 entity",
            }
        )
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=context"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("Nash", out)
        self.assertIn("core concept", out)
        self.assertIn("recalled 1 entity", out)

    def test_render_context_empty(self):
        out, err, rc = run(
            [
                sys.executable,
                COMPASS_SCRIPT,
                "render",
                'raw={"context": []}',
                "action=context",
            ]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("没有找到", out)

    def test_render_create(self):
        raw = json.dumps(
            {"id": "know000002", "title": "New Note", "file_path": "Knowledge/know000002.md"}
        )
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=create"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("已创建", out)
        self.assertIn("New Note", out)

    def test_render_score(self):
        raw = json.dumps({"id": "know000001", "score": {"composite": 89.7}})
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=score"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("评分已更新", out)
        self.assertIn("89.7", out)

    def test_render_access(self):
        raw = json.dumps({"id": "know000001", "score": {"composite": 91.7}})
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", f"raw={raw}", "action=access"]
        )
        self.assertEqual(rc, 0, f"stderr: {err}")
        self.assertIn("访问已记录", out)
        self.assertIn("91.7", out)

    def test_render_unknown_action(self):
        out, err, rc = run(
            [sys.executable, COMPASS_SCRIPT, "render", 'raw={"id":"x"}', "action=unknown"]
        )
        self.assertEqual(rc, 1)
        self.assertIn("unknown action", err)

    def test_render_invalid_json(self):
        out, err, rc = run(
            [
                sys.executable,
                COMPASS_SCRIPT,
                "render",
                "raw=not json",
                "action=search",
            ]
        )
        self.assertEqual(rc, 1)
        self.assertIn("invalid JSON", err)

    def test_render_from_stdin(self):
        raw = json.dumps([{"id": "k1", "title": "T", "composite": 50.0}])
        result = subprocess.run(
            [sys.executable, COMPASS_SCRIPT, "render", "action=top"],
            input=raw,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        self.assertEqual(result.returncode, 0, f"stderr: {result.stderr}")
        self.assertIn("T", result.stdout)


class TestCLI(unittest.TestCase):
    def test_help_flag(self):
        out, err, rc = run([sys.executable, COMPASS_SCRIPT, "--help"])
        self.assertEqual(rc, 0)
        self.assertIn("Usage", out)

    def test_unknown_action(self):
        out, err, rc = run([sys.executable, COMPASS_SCRIPT, "foobar"])
        self.assertEqual(rc, 1)
        self.assertIn("unknown action", err)


if __name__ == "__main__":
    unittest.main(verbosity=2)
