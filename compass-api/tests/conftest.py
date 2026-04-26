"""Pytest fixtures — in-memory database and TestClient."""
from __future__ import annotations

import os
import tempfile
import pytest
from pathlib import Path
from unittest.mock import AsyncMock, patch, MagicMock

import pytest_asyncio
from fastapi.testclient import TestClient

# Ensure test paths exist before config module loads
TEST_VAULT = Path("/tmp/test_compass_vault")
TEST_BIN = Path("/tmp/test_compass_bin")
TEST_VAULT.mkdir(parents=True, exist_ok=True)
(TEST_BIN / "compass_core").touch()

os.environ["VAULT_PATH"] = str(TEST_VAULT)
os.environ["RUST_BINARY_PATH"] = str(TEST_BIN / "compass_core")

# Use a temp file for DB so init_db's mkdir works
_db_fd, _DB_PATH = tempfile.mkstemp(suffix=".db")
os.environ["DB_PATH"] = _DB_PATH


@pytest.fixture(scope="session")
def db_path():
    return Path(_DB_PATH)


@pytest_asyncio.fixture
async def db(db_path: Path):
    """Temp-file SQLite database with schema initialized."""
    from src.db.database import Database, init_db, set_db
    conn = await init_db(db_path)
    database = Database(db_path)
    database._conn = conn
    set_db(database)
    yield database
    await conn.close()


@pytest.fixture
def mock_db(db):
    """Alias for the db fixture — db already calls set_db."""
    yield db


@pytest.fixture
def client(mock_db):
    """FastAPI TestClient with in-memory DB and mocked rust client."""
    with patch("src.core.rust_client.rust_client") as mock_rust:
        mock_rust.compute_score = AsyncMock(return_value=MagicMock(
            final_score=5.0, decay_factor=0.95, days_elapsed=1.0,
        ))
        mock_rust.parse_refs = AsyncMock(return_value=MagicMock(refs=[]))

        from src.main import app
        with TestClient(app) as test_client:
            yield test_client
