"""Application configuration — all env-driven, no magic constants."""
from pathlib import Path
from dotenv import load_dotenv
import os

load_dotenv()

# Vault
VAULT_PATH = Path(os.getenv("VAULT_PATH", "./vault"))

# Rust binary
RUST_BINARY_PATH = Path(os.getenv("RUST_BINARY_PATH", "./bin/compass_core"))

# Database
DB_PATH = Path(os.getenv("DB_PATH", "./data/compass.db"))
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
