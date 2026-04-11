"""FileWatcher — monitors vault directory, keeps Compass API in sync.

Events (create / modify / delete) are coalesced with a 500 ms debounce window
so rapid file saves bundle into a single API call.

Startup behaviour:
  1. Walk the entire vault and POST / PUT every *.md file found.
     This catches files modified while the watcher was not running.
  2. Register the watchdog observer and enter the event loop.

Entity ID mapping
-----------------
Entity IDs are derived from the vault-relative file path by stripping the
extension and replacing path separators with hyphens:

  Inbox/daily-2026-04-06.md  →  inbox-daily-2026-04-06
  Projects/compass.md        →  projects-compass

This must match what users write in [[wiki-links]] so that FileWatcher and
the Rust reference parser agree on IDs.

Frontmatter expected fields
---------------------------
  title       (str)  — entity title; defaults to sanitized filename
  category    (str)  — defaults to "Inbox"
  interest    (float) — 0-10 scale, default 5.0
  strategy    (float) — 0-10 scale, default 5.0
  consensus   (float) — 0-10 scale, default 0.0
  tags        (list[str]) — stored in metadata
  Any additional fields are stored verbatim in metadata.
"""
from __future__ import annotations

import asyncio
import logging

logger = logging.getLogger(__name__)
import re
import sys
import threading
import time
from pathlib import Path
from typing import Optional

import frontmatter
import httpx
from watchdog.observers import Observer
from watchdog.events import (
    FileSystemEvent,
    FileSystemEventHandler,
    FileCreatedEvent,
    FileModifiedEvent,
    FileDeletedEvent,
    FileMovedEvent,
)

from src import config

# ---- entity ID helpers ----

_STRIP_EXT_RE = re.compile(r"\.(md|MD|markdown|MARKDOWN)$")
_MULTI_DASH_RE = re.compile(r"-+")


def vault_path_to_entity_id(vault_path: str) -> str:
    """Stable entity ID derived from vault-relative path.

    Examples:
      Inbox/daily-2026-04-06.md  →  inbox-daily-2026-04-06
      Projects/compass_v2.md     →  projects-compass_v2
    """
    # Remove file extension
    stem = _STRIP_EXT_RE.sub("", vault_path)
    # Replace / and \ with -
    stem = stem.replace("/", "-").replace("\\", "-")
    # Collapse multiple dashes
    stem = _MULTI_DASH_RE.sub("-", stem)
    # Strip leading/trailing dashes
    stem = stem.strip("-")
    return stem.lower()


# ---- frontmatter parsing ----

class ParsedFile:
    """Decoded frontmatter + body from a vault markdown file."""

    def __init__(
        self,
        vault_path: str,
        entity_id: str,
        title: str,
        category: str,
        interest: float,
        strategy: float,
        consensus: float,
        content: Optional[str],
        metadata: dict,
    ):
        self.vault_path = vault_path
        self.entity_id = entity_id
        self.title = title
        self.category = category
        self.interest = interest
        self.strategy = strategy
        self.consensus = consensus
        self.content = content  # markdown body for ref extraction
        self.metadata = metadata

    def to_api_payload(self) -> dict:
        """Build the POST / PUT request body."""
        return {
            "id": self.entity_id,
            "title": self.title,
            "category": self.category,
            "vault_path": self.vault_path,
            "interest": self.interest,
            "strategy": self.strategy,
            "consensus": self.consensus,
            "content": self.content,
            "metadata": self.metadata,
        }


def parse_markdown_file(path: Path, vault_path: str) -> Optional[ParsedFile]:
    """Parse a markdown file, extracting frontmatter and body.

    Returns None if the file cannot be read or has no frontmatter.
    Uses frontmatter.load() which accepts a filename string.
    """
    try:
        # frontmatter.load() accepts a filename string (or file object)
        post = frontmatter.load(str(path))
    except Exception as exc:
        return None

    metadata: dict = dict(post.metadata)  # pyfrontmatter Post stores metadata
    body = post.content or ""

    entity_id = vault_path_to_entity_id(vault_path)

    title = str(metadata.pop("title", ""))
    if not title:
        # Fallback to filename without extension
        title = _STRIP_EXT_RE.sub("", Path(vault_path).name)

    category = str(metadata.pop("category", "Inbox"))
    interest = float(metadata.pop("interest", 5.0))
    strategy = float(metadata.pop("strategy", 5.0))
    consensus = float(metadata.pop("consensus", 0.0))

    # Store remaining keys in metadata
    extra = dict(metadata)

    return ParsedFile(
        vault_path=vault_path,
        entity_id=entity_id,
        title=title,
        category=category,
        interest=interest,
        strategy=strategy,
        consensus=consensus,
        content=body,
        metadata=extra,
    )


# ---- API client ----

_API_CLIENT: Optional[httpx.AsyncClient] = None


def get_client() -> httpx.AsyncClient:
    global _API_CLIENT
    if _API_CLIENT is None:
        _API_CLIENT = httpx.AsyncClient(
            base_url=f"http://{config.HOST}:{config.PORT}",
            timeout=10.0,
        )
    return _API_CLIENT


async def close_client() -> None:
    global _API_CLIENT
    if _API_CLIENT:
        await _API_CLIENT.aclose()
        _API_CLIENT = None


async def api_upsert(parsed: ParsedFile) -> None:
    """Create or update an entity via POST (server-side upsert handles both cases)."""
    client = get_client()
    payload = parsed.to_api_payload()
    # POST /entities with an id is idempotent — the server's ON CONFLICT DO
    # UPDATE handles the create-or-update semantics.  No client-side PUT→POST
    # fallback needed, eliminating the race window between the two calls.
    resp = await client.post("/entities", json=payload)
    resp.raise_for_status()


async def api_delete(entity_id: str) -> None:
    """DELETE an entity. 404 is treated as success (already gone)."""
    client = get_client()
    resp = await client.delete(f"/entities/{entity_id}")
    if resp.status_code != 404:
        resp.raise_for_status()


# ---- coalescing event queue ----

class EventQueue:
    """Coalescing event buffer.

    Multiple watchdog events for the same path within `delay` seconds
    are collapsed to the most recent event type.
    EventHandler writes to it (from watchdog thread);
    the poll loop reads from it (from asyncio event loop).

    Uses threading.Lock because watchdog calls push() from its own thread,
    not from the asyncio event loop.
    """

    def __init__(self, delay: float = 0.5) -> None:
        self.delay = delay
        self._events: dict[str, tuple[str, float]] = {}  # path → (type, timestamp)
        self._lock = threading.Lock()

    def push(self, path: str, event_type: str) -> None:
        """Called from the watchdog thread — safe to call from any thread."""
        with self._lock:
            self._events[path] = (event_type, time.monotonic())

    async def drain(self) -> list[tuple[str, str]]:
        """Return and clear all events older than self.delay seconds.

        threading.Lock.acquire() is blocking, so we run it in a thread pool
        to avoid blocking the asyncio event loop.
        """
        loop = asyncio.get_running_loop()
        await loop.run_in_executor(None, self._lock.acquire)
        try:
            now = time.monotonic()
            out: list[tuple[str, str]] = []
            for path, (etype, ts) in list(self._events.items()):
                if now - ts >= self.delay:
                    out.append((path, etype))
                    del self._events[path]
            return out
        finally:
            self._lock.release()


# ---- watchdog event handler ----

_QUEUE: Optional[EventQueue] = None


def set_queue(q: EventQueue) -> None:
    global _QUEUE
    _QUEUE = q


def _enqueue(path: str, event_type: str) -> None:
    if _QUEUE is not None:
        _QUEUE.push(path, event_type)


class VaultHandler(FileSystemEventHandler):
    """Watches the vault root and enqueues events for any *.md change."""

    def __init__(self, vault_path: Path) -> None:
        self.vault_path = vault_path.resolve()

    def _md_path(self, path: str) -> Optional[str]:
        p = Path(path).resolve()
        try:
            p.relative_to(self.vault_path)
        except ValueError:
            return None
        if p.suffix.lower() in (".md", ".markdown"):
            return str(p.relative_to(self.vault_path))
        return None

    def on_created(self, event: FileCreatedEvent) -> None:
        vp = self._md_path(event.src_path)
        if vp:
            _enqueue(vp, "create")

    def on_modified(self, event: FileSystemEvent) -> None:
        if isinstance(event, FileModifiedEvent):
            vp = self._md_path(event.src_path)
            if vp:
                _enqueue(vp, "update")

    def on_deleted(self, event: FileSystemEvent) -> None:
        if isinstance(event, FileDeletedEvent):
            vp = self._md_path(event.src_path)
            if vp:
                _enqueue(vp, "delete")

    def on_moved(self, event: FileSystemEvent) -> None:
        # A move is delete + create from our perspective
        if isinstance(event, FileMovedEvent):
            old_vp = self._md_path(event.src_path)
            new_vp = self._md_path(event.dest_path)
            if old_vp:
                _enqueue(old_vp, "delete")
            if new_vp:
                _enqueue(new_vp, "create")


# ---- vault scan ----

async def scan_vault() -> list[ParsedFile]:
    """Walk the entire vault and parse every markdown file."""
    vault = config.VAULT_PATH
    results: list[ParsedFile] = []
    if not vault.exists():
        logger.warning(f"[FileWatcher] Vault not found: {vault}")
        return results
    for path in vault.rglob("*.md"):
        vp = str(path.relative_to(vault))
        parsed = parse_markdown_file(path, vp)
        if parsed:
            results.append(parsed)
    for path in vault.rglob("*.markdown"):
        vp = str(path.relative_to(vault))
        parsed = parse_markdown_file(path, vp)
        if parsed:
            results.append(parsed)
    return results


async def full_sync() -> None:
    """Full vault scan: upsert every file. Used on startup."""
    files = await scan_vault()
    logger.info(f"[FileWatcher] Full sync: {len(files)} files found")
    for parsed in files:
        try:
            await api_upsert(parsed)
            logger.info(f"  [synced]   {parsed.vault_path} → {parsed.entity_id}")
        except Exception as exc:
            logger.error(f"  [error]    {parsed.vault_path}: {exc}")


# ---- main loop ----

async def process_events(queue: EventQueue) -> None:
    """Poll the coalescing queue and dispatch to the API."""
    while True:
        await asyncio.sleep(queue.delay)
        events = await queue.drain()
        for vault_path, etype in events:
            entity_id = vault_path_to_entity_id(vault_path)
            if etype == "delete":
                logger.info(f"[FileWatcher] delete {vault_path}")
                try:
                    await api_delete(entity_id)
                except Exception as exc:
                    logger.error(f"  [error] delete {entity_id}: {exc}")
            else:
                # create or update — re-parse the file and upsert
                full_path = config.VAULT_PATH / vault_path
                if not full_path.exists():
                    logger.info(f"[FileWatcher] skip (gone) {vault_path}")
                    continue
                parsed = parse_markdown_file(full_path, vault_path)
                if not parsed:
                    logger.warning(f"[FileWatcher] skip (parse error) {vault_path}")
                    continue
                logger.info(f"[FileWatcher] {etype} {vault_path} → {entity_id}")
                try:
                    await api_upsert(parsed)
                except Exception as exc:
                    logger.error(f"  [error] upsert {entity_id}: {exc}")


def run_watcher() -> None:
    """Entry point: start the watchdog observer (blocking)."""
    vault = config.VAULT_PATH
    if not vault.exists():
        raise RuntimeError(f"VAULT_PATH does not exist: {vault}")

    queue = EventQueue(delay=0.5)
    set_queue(queue)

    handler = VaultHandler(vault)
    observer = Observer()
    observer.schedule(handler, str(vault), recursive=True)
    observer.start()
    logger.info(f"[FileWatcher] Watching {vault} (recursive)")

    # Run the async event processor in the event loop
    try:
        asyncio.run(process_events(queue))
    finally:
        observer.stop()
        observer.join()
        asyncio.run(close_client())


if __name__ == "__main__":
    run_watcher()
