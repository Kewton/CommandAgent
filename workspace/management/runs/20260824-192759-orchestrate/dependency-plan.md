# Dependency Plan

## Parallel Batches

- Batch 1: #370

## Merge Order

#370

## Issue Plans

### Worktree row #370

- Issues: #370
- Classification: `independent`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: Merge-recovery only in the existing feature/issue-370-gui-trial worktree; merge exact origin/develop 7abad1484fa29051a692f0b452b8158bb68808e2 with a normal merge commit; preserve Issue 369 deterministic completion-boundary assertions and Issue 370 four-route behavior; do not push, mutate PRs or Issues, use CommandMate, or alter historical evidence.
- Branch: `feature/issue-370-gui-trial`
- Worktree: `../CommandAgent-issue-370-gui-trial`
- Suspected files: `gui/components/trial-run.tsx, gui/components/trial-session-index.tsx, CHANGELOG.md, README.md, docs/README.md, docs/d3c-shell-design.md, docs/dev/mechanism-ledger.md, Cargo.toml`
- References: `none`

## Blocked Items

None at dry-run planning time.
