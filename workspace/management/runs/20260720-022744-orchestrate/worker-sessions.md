# Worker Sessions

## Issue #31

- Branch: `feature/issue-31-add-a-clean-release-build-that-leaves-only-the-e`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-31-add-a-clean-release-build-that-leaves-only-the-e`
- Status: `created`
- Message: worktree created
- Worker status: `planned`
- Running: `None`
- Processing: `None`
- Worker message: dry-run: CommandMate dispatch skipped

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-31-add-a-clean-release-build-that-leaves-only-the-e Codex issue worker task for Issue #31

If `$codex-issue-worker` is available in this worktree, follow that skill.
If it is not available, treat this message as the full worker instruction.

## Required Workflow

1. Read the Issue summary, acceptance criteria, approved decision, suspected files, and references.
2. Write a short design note before editing.
3. Implement the smallest coherent change that satisfies the Issue.
4. Add or update focused tests where appropriate.
5. Run focused verification, and broader checks if shared contracts are touched.
6. Write `dev-reports/issue-<number>/design.md`, `implementation-summary.md`, and `verification.md`.
7. In `verification.md`, record the exact line "- Status: `passed`" only when every required check passed, followed by one "- `<command>`: `passed`" entry per check. Use `blocked` when any required check fails or cannot run.
8. Commit the work with a clear Issue-scoped commit message.
9. Report blockers only if implementation cannot safely proceed.

## Issue Summary

- Title: Add a clean release build that leaves only the executable
- Objective: `cargo build --release` currently leaves Cargo link-time artifacts and stale hashed variants under `target/release/deps`.

## Acceptance Criteria

- A documented repository command produces an optimized `commandagent` executable.
- The published executable reports the expected commit/version provenance through `--version`.
- After a successful clean release build, `target/release/deps` is absent or contains no generated libraries.
- `commandagentdev --version` succeeds after the build.
- An induced build or verification failure preserves the previously published executable.
- Temporary build artifacts are removed after both success and failure.
- Ordinary `cargo build`, `cargo test`, and development caching semantics remain unchanged.
- Focused automated tests cover success, failure preservation, cleanup, and launcher-path compatibility.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass.

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/lib.rs
- src/main.rs
- build.rs
- docs/codex-harness.md
- docs/mechanism-ledger.md
- docs/model-probe.md
- docs/uat/scenarios.md
- scripts/eval_lib/report.py

## References

- なし

## Required Predecessors

- None

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-31-add-a-clean-release-build-that-leaves-only-the-e
- Worktree: ../CommandAgent-issue-31-add-a-clean-release-build-that-leaves-only-the-e
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
