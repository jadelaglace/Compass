# Compass v3 Test Cases

> Version: v1.1 | Date: 2026-07-12 | Status: active acceptance baseline
>
> Upstream: [`ARCHITECTURE.md`](ARCHITECTURE.md). Downstream execution: [`PLAN.md`](PLAN.md). This document specifies behavior to preserve and extend; test implementation belongs with the relevant Rust or Skill module.

## 1. Test Rules

- All write-path tests use a temporary Vault and an isolated SQLite database. Real Vault data is never test input.
- Tests use a fixed clock whenever freshness or timestamps affect the result.
- API and Skill contracts are compatibility boundaries. A behavior change requires a PRD and test-case update before implementation.
- The frozen Web UI is smoke-tested only for retained compatibility. No new Web UI acceptance cases are added unless the freeze policy changes.

## 2. Product Acceptance Cases

| ID | Scenario | Expected result | Level |
|---|---|---|---|
| TC-D01 | Calculate a base score with default or valid custom weights | Base composite is deterministic; invalid weights are rejected | Domain unit |
| TC-D02 | Read an evergreen, decay, or expired entity at a fixed time | `effective_composite` uses the defined factor; base score remains unchanged | Domain unit |
| TC-D03 | Advance time without an explicit score/access/trigger action | No time-driven score value, history event, or Vault write is created | Domain/application |
| TC-V01 | Create an entity through the public API | Markdown and valid frontmatter are created; SQLite index can find it | Application/API E2E |
| TC-V02 | Update score, access, or accepted metadata | Only intended frontmatter fields change; index and history reconcile | Application/infrastructure |
| TC-V03 | Fail an index update after a successful Vault write | Vault remains authoritative; rebuild or watcher can restore index consistency | Application/integration |
| TC-I01 | Rebuild a Vault containing valid notes, templates, hidden paths, and duplicate IDs | Only eligible notes are indexed; duplicates are reported; rebuild is idempotent | Infrastructure integration |
| TC-I02 | Modify or delete a watched file | Incremental indexing produces the same projection as a rebuild | Integration |
| TC-Q01 | Request feed, top, search, graph, or agent context | Results use current effective score and stable public response fields | Application/API E2E |
| TC-Q02 | Query while a Vault read or result sort is slow | SQLite lock is released before file work or sorting; unrelated DB work can progress | Concurrency/integration |
| TC-S01 | Generate, accept, or reject tag and related suggestions | Candidate generation is read-only; acceptance is idempotent; reject writes no Vault content | Application/API E2E |
| TC-S02 | Generate a weekly report with fixed time zone and sparse data | Output is deterministic and handles empty/missing data | Application unit/E2E |
| TC-H01 | Access an API on localhost and on non-local bind settings | Unsafe exposure is rejected unless explicitly configured; optional bearer auth is enforced | API E2E |
| TC-K01 | Run each public Skill action against a Compass instance | Skill request, HTTP contract, Vault/SQLite effects, and render output remain compatible | Skill E2E |
| TC-W01 | Request the retained static Web entry point and `/graph` | Existing page and graph response remain reachable; no feature expansion is implied | HTTP smoke |
| TC-B01 | Back up an isolated dedicated Vault Git repository | Only Markdown and documented stable Obsidian settings are committed; the script emits a name/status diff and commit hash while preserving the working Markdown | PowerShell/Git integration |
| TC-B02 | Re-run backup with no eligible changes or a pre-existing staged change | No empty commit is created; pre-existing staging is refused and no backup commit is created | PowerShell/Git integration |
| TC-Y01 | Rebuild a Vault containing a primary Markdown note and a sync conflict copy with the same ID | Only the primary note is indexed; the conflict copy is retained but does not become a duplicate entity; after manual resolution, rebuild replaces the local projection | Rust integration |
| TC-Y02 | Receive a sync conflict-copy watcher event | The watcher requests a rebuild; neither the conflict copy nor a stale pre-rename projection remains indexed | Rust integration/manual two-device acceptance |

## 3. Architecture Regression Cases

| ID | Boundary | Acceptance |
|---|---|---|
| TC-A01 | Transport | Route handlers validate DTOs, call an application service, and serialize results; they do not parse Markdown or issue SQL directly |
| TC-A02 | Application | Services do not import Axum or SQLite row types and receive external capabilities through ports |
| TC-A03 | Infrastructure | `EntityRow` and other SQL mapping types remain private to the SQLite adapter; Vault parsing remains private to the Vault adapter |
| TC-A04 | Indexing | Rebuild and watcher changes share the same parse-to-projection path |
| TC-A05 | Concurrency | No database mutex guard spans file I/O, sorting, serialization, or `.await` |

Architecture cases may use compilation/privacy checks, focused source-boundary tests, and review checklists. They supplement, rather than replace, behavioral tests.

## 4. Required Verification Per Change

| Change scope | Minimum verification |
|---|---|
| Domain-only score or freshness change | Relevant domain cases plus full Rust test suite |
| Vault, SQLite, watcher, or indexing change | TC-V, TC-I, TC-A04/A05 coverage plus Rust tests |
| HTTP contract or auth change | Relevant TC-Q/TC-H cases plus Skill E2E when Skill consumes the endpoint |
| Skill change | Relevant renderer tests and TC-K01 E2E |
| Vault backup script or task change | TC-B01/B02 integration check plus `git diff --check` |
| Sync conflict handling or synchronization procedure change | TC-Y01/Y02 coverage plus manual two-device conflict/rebuild validation |
| Documentation-only change | Link check/review and `git diff --check` |

The current command baseline is:

```powershell
cd compass-core
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release

cd ..\skills\compass
python -m unittest -q test_compass.py
python -m unittest -q test_e2e.py
```
