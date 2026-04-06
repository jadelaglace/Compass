"""Application configuration — all env-driven, no magic constants."""
from pathlib import Path
from dotenv import load_dotenv
import os

load_dotenv()

# Base directory (where this file lives — compass-api/src/)
BASE_DIR = Path(__file__).parent.parent

# Vault — resolved relative to BASE_DIR if not absolute
_vault_env = os.getenv("VAULT_PATH", "")
if _vault_env:
    VAULT_PATH = Path(_vault_env) if Path(_vault_env).is_absolute() else (BASE_DIR / _vault_env)
else:
    VAULT_PATH = BASE_DIR / "vault"
VAULT_PATH = VAULT_PATH.resolve()

# Rust binary — resolved relative to BASE_DIR
_rust_env = os.getenv("RUST_BINARY_PATH", "")
if _rust_env:
    RUST_BINARY_PATH = Path(_rust_env) if Path(_rust_env).is_absolute() else (BASE_DIR / _rust_env)
else:
    RUST_BINARY_PATH = BASE_DIR / "bin" / "compass_core"
RUST_BINARY_PATH = RUST_BINARY_PATH.resolve()

# Database
_db_env = os.getenv("DB_PATH", "")
if _db_env:
    DB_PATH = Path(_db_env) if Path(_db_env).is_absolute() else (BASE_DIR / _db_env)
else:
    DB_PATH = BASE_DIR / "data" / "compass.db"
DB_PATH = DB_PATH.resolve()
DB_PATH.parent.mkdir(parents=True, exist_ok=True)

# FastAPI
HOST = os.getenv("HOST", "0.0.0.0")
PORT = int(os.getenv("PORT", "8000"))

# OpenAI / Anthropic (for Agent SDK, Phase 1 optional)
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")
ANTHROPIC_API_KEY = os.getenv("ANTHROPIC_API_KEY")

# Feishu (optional, for bot)
FEISHU_APP_ID = os.getenv("FEISHU_APP_ID")
FEISHU_APP_SECRET = os.getenv("FEISHU_APP_SECRET")

# ---- startup validation ----
_missing: list[str] = []
if not RUST_BINARY_PATH.exists():
    _missing.append(f"RUST_BINARY_PATH not found: {RUST_BINARY_PATH}")
if not VAULT_PATH.exists():
    _missing.append(f"VAULT_PATH not found: {VAULT_PATH}")

if _missing:
    raise RuntimeError(
        "Compass API startup errors:\n  " + "\n  ".join(_missing)
        + "\n\nSet RUST_BINARY_PATH and VAULT_PATH in .env or environment."
    )
