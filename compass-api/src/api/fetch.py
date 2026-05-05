"""REST endpoint: URL content fetching and cleaning."""
import re
import uuid
from datetime import datetime, timezone
from typing import Annotated, Optional

import httpx
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel, HttpUrl

from src.db.database import Database, get_db
from src import config

router = APIRouter(prefix="/fetch", tags=["fetch"])


# ---- Models ----

class FetchRequest(BaseModel):
    url: HttpUrl


class FetchResponse(BaseModel):
    url: str
    title: Optional[str]
    raw_content: str
    content_type: str
    status_code: int
    fetched_at: str


class CleanRequest(BaseModel):
    raw_content: str
    source_url: Optional[str] = None


class CleanResponse(BaseModel):
    title: Optional[str]
    content: str
    summary: Optional[str] = None
    tags: list[str] = []
    source_url: Optional[str] = None


class SaveRequest(BaseModel):
    title: str
    content: str
    source_url: Optional[str] = None
    tags: list[str] = []
    category: str = "Inbox"


class SaveResponse(BaseModel):
    entity_id: str
    file_path: str
    title: str


# ---- Helpers ----

_USER_AGENT = "Compass-Fetch/2.1"
_TIMEOUT = 10.0
_NOISE_TAGS = {"nav", "footer", "aside", "header", "form", "script",
               "style", "noscript", "iframe", "svg"}


def _extract_title(raw_html: str) -> Optional[str]:
    for pattern in [
        r'<meta[^>]+property=["\']og:title["\'][^>]+content=["\']([^"\']+)["\']',
        r'<meta[^>]+content=["\']([^"\']+)["\'][^>]+property=["\']og:title["\']',
        r'<title[^>]*>([^<]+)</title>',
    ]:
        m = re.search(pattern, raw_html, re.IGNORECASE)
        if m:
            return m.group(1).strip()
    return None


def _extract_tags(raw_html: str) -> list[str]:
    for pattern in [
        r'<meta[^>]+name=["\']keywords["\'][^>]+content=["\']([^"\']+)["\']',
        r'<meta[^>]+content=["\']([^"\']+)["\'][^>]+name=["\']keywords["\']',
    ]:
        m = re.search(pattern, raw_html, re.IGNORECASE)
        if m:
            return [
                f"#{kw.strip().lower().replace(' ', '-')}"
                for kw in m.group(1).split(",")
                if kw.strip() and len(kw.strip()) > 2
            ][:5]
    return []


def _clean_html(raw_html: str) -> str:
    # Remove noise
    html = re.sub(r'<(script|style|noscript|noframes|noembed)[^>]*>.*?</\1>',
                  '', raw_html, flags=re.DOTALL | re.IGNORECASE)
    for tag in _NOISE_TAGS:
        html = re.sub(f'<{tag}[^>]*>.*?</{tag}>', '', html, flags=re.DOTALL | re.IGNORECASE)

    c = html
    c = re.sub(r'<img[^>]+src=["\']([^"\']+)["\'][^>]*>(?:</img>)?',
               lambda m: f'\n![img]({m.group(1)})\n', c, flags=re.IGNORECASE)
    c = re.sub(r'<a[^>]+href=["\']([^"\']+)["\'][^>]*>([^<]+)</a>',
               lambda m: f'[{m.group(2).strip()}]({m.group(1)})', c, flags=re.DOTALL)
    for i in range(1, 7):
        c = re.sub(rf'<h{i}[^>]*>(.*?)</h{i}>', rf'\n{"#"*i} \1\n',
                   c, flags=re.DOTALL | re.IGNORECASE)
    c = re.sub(r'<p[^>]*>(.*?)</p>', r'\n\1\n\n', c, flags=re.DOTALL | re.IGNORECASE)
    c = re.sub(r'<blockquote[^>]*>(.*?)</blockquote>', r'\n> \1\n', c, flags=re.DOTALL | re.IGNORECASE)
    c = re.sub(r'<pre><code[^>]*>(.*?)</code></pre>', r'\n```\n\1\n```\n', c, flags=re.DOTALL)
    c = re.sub(r'<code[^>]*>(.*?)</code>', r'`\1`', c, flags=re.DOTALL)
    c = re.sub(r'<li[^>]*>(.*?)</li>', r'\n- \1', c, flags=re.DOTALL | re.IGNORECASE)
    c = re.sub(r'<(ul|ol)[^>]*>', '\n', c, flags=re.IGNORECASE)
    c = re.sub(r'</(ul|ol)[^>]*>', '\n', c, flags=re.IGNORECASE)
    c = re.sub(r'<hr[^>]*>', '\n---\n', c, flags=re.IGNORECASE)
    for tag, wrap in [("strong", "**"), ("b", "**"), ("em", "*"), ("i", "*")]:
        c = re.sub(rf'<{tag}[^>]*>(.*?)</{tag}>',
                   lambda m: f'{wrap}{m.group(1)}{wrap}', c, flags=re.DOTALL)
    c = re.sub(r'<[^>]+>', '', c)
    c = c.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">")
    c = c.replace("&amp;", "&").replace("&quot;", '"').replace("&#39;", "'").replace("&apos;", "'")
    lines = [re.sub(r'\s+', ' ', line).strip() for line in c.splitlines()]
    return re.sub(r'\n{3,}', '\n\n', "\n".join(l for l in lines if l)).strip()


# ---- Endpoints ----

@router.post("", response_model=FetchResponse)
async def fetch_url(req: FetchRequest) -> FetchResponse:
    """Fetch raw HTML from URL (HTTP/HTTPS, 10s timeout, max 3 redirects)."""
    try:
        async with httpx.AsyncClient(
            timeout=httpx.Timeout(_TIMEOUT),
            follow_redirects=True,
            max_redirects=3,
            headers={"User-Agent": _USER_AGENT},
        ) as client:
            resp = await client.get(str(req.url))
    except httpx.TimeoutException:
        raise HTTPException(status_code=408, detail="Fetch timeout (10s)")
    except httpx.RequestError:
        raise HTTPException(status_code=400, detail="Request failed — check URL")

    if not (200 <= resp.status_code < 300):
        raise HTTPException(status_code=502, detail=f"Upstream returned {resp.status_code}")

    ct = resp.headers.get("content-type", "text/plain")
    title = _extract_title(resp.text) if "text/html" in ct.lower() else None
    return FetchResponse(
        url=str(req.url),
        title=title,
        raw_content=resp.text,
        content_type=ct,
        status_code=resp.status_code,
        fetched_at=datetime.now(tz=timezone.utc).isoformat(),
    )


@router.post("/clean", response_model=CleanResponse)
async def clean_content(req: CleanRequest) -> CleanResponse:
    """Strip HTML noise and convert to clean Markdown text."""
    return CleanResponse(
        title=_extract_title(req.raw_content),
        content=_clean_html(req.raw_content),
        summary=None,
        tags=_extract_tags(req.raw_content),
        source_url=req.source_url,
    )


@router.post("/save", response_model=SaveResponse)
async def save_content(
    req: SaveRequest,
    db: Annotated[Database, Depends(get_db)],
) -> SaveResponse:
    """Save cleaned content as a new Vault entity."""
    now = datetime.now(tz=timezone.utc).isoformat()
    entity_id = f"fetch-{uuid.uuid4().hex[:8]}"
    vault_path = f"{req.category}/{entity_id}.md"
    file_path = str(config.VAULT_PATH / vault_path)

    # Write markdown file
    config.VAULT_PATH.mkdir(parents=True, exist_ok=True)
    vault_file = config.VAULT_PATH / vault_path
    meta = f"---\nsource_url: {req.source_url or ''}\ntags: [{', '.join(req.tags)}]\n---\n"
    vault_file.write_text(f"{meta}# {req.title}\n\n{req.content}", encoding="utf-8")

    # Persist entity
    entity_data = {
        "id": entity_id, "file_path": file_path, "vault_path": vault_path,
        "title": req.title, "category": req.category,
        "created_at": now, "updated_at": now,
    }
    score_data = {
        "entity_id": entity_id, "interest": 5.0, "strategy": 5.0,
        "consensus": 0.0, "final_score": 5.0, "updated_at": now,
    }
    await db.create_entity_full(entity_data, score_data, [], "created", "fetch")
    return SaveResponse(entity_id=entity_id, file_path=file_path, title=req.title)