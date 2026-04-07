"""Python ↔ Rust subprocess JSON-RPC client (async)."""
from __future__ import annotations

import asyncio
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from src import config as cfg


@dataclass
class ScoreResult:
    final_score: float
    decay_factor: float
    days_elapsed: float


@dataclass
class RefsResult:
    refs: list[str]


class RustClient:
    """Calls compass-core binary via JSON-RPC over stdin/stdout (async)."""

    def __init__(self, binary_path: Optional[Path] = None) -> None:
        self.binary_path = str(binary_path or cfg.RUST_BINARY_PATH)

    async def _call(self, method: str, params: dict) -> dict:
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1,
        }
        proc = await asyncio.create_subprocess_exec(
            self.binary_path,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, stderr = await asyncio.wait_for(
            proc.communicate(input=json.dumps(payload).encode()),
            timeout=10.0,
        )
        if proc.returncode != 0:
            raise RuntimeError(f"Rust binary error: {stderr.decode()}")
        response = json.loads(stdout)
        if "error" in response:
            raise RuntimeError(f"JSON-RPC error: {response['error']}")
        return response.get("result", {})

    async def compute_score(
        self,
        interest: float,
        strategy: float,
        consensus: float,
        last_boosted_at: str,
        interest_half_life_days: float = 30.0,
        strategy_half_life_days: float = 365.0,
        consensus_half_life_days: float = 60.0,
    ) -> ScoreResult:
        params = {
            "interest": interest,
            "strategy": strategy,
            "consensus": consensus,
            "last_boosted_at": last_boosted_at,
            "interest_half_life_days": interest_half_life_days,
            "strategy_half_life_days": strategy_half_life_days,
            "consensus_half_life_days": consensus_half_life_days,
        }
        result = await self._call("compute_score", params)
        return ScoreResult(
            final_score=result["final_score"],
            decay_factor=result["decay_factor"],
            days_elapsed=result["days_elapsed"],
        )

    async def parse_refs(self, content: str, current_id: Optional[str] = None) -> RefsResult:
        params = {"content": content, "current_entity_id": current_id}
        result = await self._call("parse_refs", params)
        return RefsResult(refs=result.get("refs", []))


# Singleton instance
rust_client = RustClient()
