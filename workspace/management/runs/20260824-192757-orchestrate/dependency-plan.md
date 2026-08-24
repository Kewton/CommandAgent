# Dependency Plan

## Parallel Batches

- Batch 1: #371
- Batch 2: #372

## Merge Order

#371, #372

## Issue Plans

### Worktree row #371

- Issues: #371
- Classification: `independent`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-371-gui-extensions`
- Worktree: `../CommandAgent-issue-371-gui-extensions`
- Suspected files: `README.md, docs/guide/README.md, docs/guide/en/extensions.md, docs/user/gui-extensions.md, docs/user/gui-operations.md, CHANGELOG.md, Cargo.toml, docs/README.md`
- References: `none`

### Worktree row #372

- Issues: #372
- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #371
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-372-gui-extensions-draft-profile-api`
- Worktree: `../CommandAgent-issue-372-gui-extensions-draft-profile-api`
- Suspected files: `<extension-root>/profiles/<id>/manifest.toml, overlay.toml, src/planner/runner.rs, src/minimal_loop/loop_run.rs, README.md, docs/guide/README.md, docs/guide/en/extensions.md, docs/user/gui-extensions.md`
- References: `none`

## Blocked Items

None at dry-run planning time.
