# Dependency Plan

## Parallel Batches

- Batch 1: #375
- Batch 2: #370
- Batch 3: #374
- Batch 4: #376
- Batch 5: #377

## Merge Order

#375, #370, #374, #376, #377

## Issue Plans

### Worktree row #375

- Issues: #375
- Classification: `weak-conflict`
- Dependency reason: explicitly has no dependencies
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-375-trial-events-stepplan`
- Worktree: `../CommandAgent-issue-375-trial-events-stepplan`
- Suspected files: `src/planner/step_plan.rs, src/planner/runner/phase/step_plan_execution.rs, summary.md, src/planner/runner.rs, src/minimal_loop/loop_run.rs, phase/step_plan_execution.rs, src/eval_events, tests/corpus/apps, CHANGELOG.md`
- References: `none`

### Worktree row #370

- Issues: #370
- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #375
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-370-gui-trial`
- Worktree: `../CommandAgent-issue-370-gui-trial`
- Suspected files: `gui/components/trial-run.tsx, gui/components/trial-session-index.tsx, CHANGELOG.md, README.md, docs/README.md, docs/d3c-shell-design.md, docs/dev/mechanism-ledger.md, Cargo.toml`
- References: `none`

### Worktree row #374

- Issues: #374
- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #370
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-374-gui-trial`
- Worktree: `../CommandAgent-issue-374-gui-trial`
- Suspected files: `summary.md, src/bin/gui_server/session_paths.rs, delegate.rs, tests/gui_server.rs, CHANGELOG.md, README.md, docs/README.md, docs/d3c-shell-design.md`
- References: `none`

### Worktree row #376

- Issues: #376
- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #374
- Dependency source: `explicit`
- Approved decision: none
- Branch: `feature/issue-376-gui-trial-plan`
- Worktree: `../CommandAgent-issue-376-gui-trial-plan`
- Suspected files: `gui/components/trial-gate-two.tsx, CHANGELOG.md, README.md, docs/README.md, docs/d3c-shell-design.md, docs/dev/mechanism-ledger.md, docs/dev/e5f-phase-state-machine.md, docs/dev/generality.md`
- References: `none`

### Worktree row #377

- Issues: #377
- Classification: `strong-dependency`
- Dependency reason: explicitly depends on #376
- Dependency source: `explicit`
- Approved decision: Propagate exact finalized dependency heads 375=1acfc81aa0ba7d7f338db4013d94df95e0d7d779, 370=9e8e178b97b49c78411ad9d2ba1783168227cdd9, 374=839f9c335a4af780a72433130e452ad984b87c3e, 376=810dd041a20cf73b03c1979cc66015d26cf65a6e into the existing feature/issue-377-gui-trial-failed worktree; preserve Issue 377 behavior; verify and commit only; do not push, mutate PRs or Issues, dispatch workers, or start/stop CommandMate.
- Branch: `feature/issue-377-gui-trial-failed`
- Worktree: `../CommandAgent-issue-377-gui-trial-failed`
- Suspected files: `src/bin/gui_server/session_diagnostics.rs, summary.md, CHANGELOG.md, README.md, docs/README.md, docs/d3c-shell-design.md, docs/dev/mechanism-ledger.md, Cargo.toml`
- References: `none`

## Blocked Items

None at dry-run planning time.
