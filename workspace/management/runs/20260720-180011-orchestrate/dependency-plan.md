# Dependency Plan

## Parallel Batches

- Batch 1: #43
- Batch 2: #44, #45, #49
- Batch 3: #46, #51
- Batch 4: #47, #48
- Batch 5: #50

## Merge Order

#43, #44, #45, #49, #46, #51, #47, #48, #50

## Issue Plans

### Issue #43

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Worktree: `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Suspected files: `src/tui/repl.rs, src/tui/footer.rs, tests/tui_pty.rs, README.md, docs/assets/ux-demo.md, src/tui/ux_demo.rs, src/tui/spinner.rs, src/minimal_loop/loop_run.rs, footer.rs, CONTRIBUTING.md, docs/assets/ux-demo.svg, src/tui/slash.rs, src/lib.rs, src/eval_events.rs, src/planner/ultra_plan_flow.rs, src/provider_call.rs, src/tui/banner.rs, src/minimal_loop/behavior_evidence.rs`
- References: `none`

### Issue #44

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Worktree: `../CommandAgent-issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Suspected files: `tests/tui_pty.rs, CONTRIBUTING.md, .github/workflows/ci.yml, release.yml, github/workflows/ci.yml, docs/codex-harness.md, docs/dev/generality.md, docs/dev/mechanism-ledger.md`
- References: `none`

### Issue #45

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Worktree: `../CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Suspected files: `status.rs, repl.rs, tests/doc_drift.rs, tests/tui_repl.rs, tests/tui_integration.rs, src/tui/slash.rs, src/tui/repl.rs, src/eval_events.rs, src/tui/status.rs, src/planner/ultra_plan_flow.rs, src/provider_call.rs, README.md`
- References: `none`

### Issue #46

- Classification: `strong-dependency`
- Dependency reason: depends on #43
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Worktree: `../CommandAgent-issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Suspected files: `src/doctor.rs, repl.rs, tests/tui_pty.rs, src/tui/repl.rs, src/providers/mod.rs, src/providers/ollama.rs, src/tui/banner.rs, docs/dev/generality.md`
- References: `none`

### Issue #47

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Worktree: `../CommandAgent-issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Suspected files: `src/tui/footer.rs, src/tui/spinner.rs, src/tui/status_bus.rs, src/env_compat.rs, footer.rs, repl.rs, Cargo.lock, README.md`
- References: `none`

### Issue #48

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-48-ux-bug-stop-streaming-raw-planner-json-into-the`
- Worktree: `../CommandAgent-issue-48-ux-bug-stop-streaming-raw-planner-json-into-the`
- Suspected files: `tests/tui_pty.rs, lib.rs, src/planner/runner.rs, src/tui/mod.rs, src/planner/ultra_plan_flow.rs, src/provider_call.rs, README.md, docs/guide/en/cli-reference.md`
- References: `none`

### Issue #49

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Worktree: `../CommandAgent-issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Suspected files: `src/tui/terminal.rs, src/util.rs, footer.rs, src/tui/presentation.rs, src/tui/footer.rs, Cargo.lock, docs/dev/profile-manifest.md, docs/guide/en/configuration.md`
- References: `none`

### Issue #50

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-50-ux-presentation-consistency-unified-elapsed-time`
- Worktree: `../CommandAgent-issue-50-ux-presentation-consistency-unified-elapsed-time`
- Suspected files: `src/tui/banner.rs, src/tui/ux_demo.rs, docs/assets/ux-demo.md, src/tui/spinner.rs, src/tui/footer.rs, src/tui/presentation.rs, src/tui/terminal.rs, src/tui/editor.rs, docs/dev/uat/scenarios.md`
- References: `none`

### Issue #51

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Dependency source: `inferred`
- Approved decision: none
- Branch: `feature/issue-51-docs-document-repl-multi-line-input-continuation`
- Worktree: `../CommandAgent-issue-51-docs-document-repl-multi-line-input-continuation`
- Suspected files: `src/tui/editor.rs, tests/doc_drift.rs, docs/guide, src/tui/slash.rs, docs/guide/en, docs/guide/ja, docs/README.md, docs/intent-skeleton.md`
- References: `none`

## Blocked Items

None at dry-run planning time.
