# Compass GitHub Development and Test Workflow

> Version: v1.0 | Date: 2026-07-11 | Status: active
>
> Upstream: [`PLAN.md`](PLAN.md) and [`TEST_CASES.md`](TEST_CASES.md). This workflow governs how an approved task becomes a reviewed, tested, merged change.

## 1. Before Coding

1. Create or select one GitHub Issue with a bounded outcome and a link to the relevant PLAN task.
2. Record the affected PRD requirement, architecture boundary, and `TEST_CASES.md` IDs in the Issue.
3. State any public HTTP, Skill, Vault, or schema contract change explicitly. These require the upstream documents to change first.
4. Create a focused branch named `codex/<issue>-<short-description>`.

## 2. Development Loop

1. Add or update the failing characterization/regression test for the target test case.
2. Implement the smallest change that makes the test pass while preserving the architecture boundary.
3. Refactor only after the relevant tests are green.
4. Keep generated files, real Vault data, credentials, and unrelated cleanup out of the branch.

## 3. Required Local Checks

Run the checks required by the affected test cases before opening a pull request:

```powershell
cd compass-core
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release

cd ..\skills\compass
python -m unittest -q test_compass.py
python -m unittest -q test_e2e.py
```

At present the repository has no tracked `.github` CI workflow. These local checks are therefore merge prerequisites, not a substitute for a future CI gate.

## 4. Pull Request and Merge

1. Open one pull request per bounded Issue.
2. The PR description links the Issue, affected document-chain entries, changed test-case IDs, and exact local check results.
3. Review checks behavior, public contracts, data ownership, lock scope, and unwanted scope expansion.
4. Resolve review findings, rerun affected checks, then merge only when the PR is review-ready and checks pass.
5. Close the Issue with the merged PR reference and update PLAN status or test evidence when the task is complete.

## 5. Change Controls

- Do not change public HTTP paths, Skill action payloads, frontmatter format, or SQLite migration behavior without a corresponding PRD, architecture, and test-case update.
- Do not add Web UI work while the freeze policy in `PRD_v3.0.md` and `PLAN.md` remains active.
- Do not claim GitHub Actions coverage until a repository workflow exists and runs these checks.
