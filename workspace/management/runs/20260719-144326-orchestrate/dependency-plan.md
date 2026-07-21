# Dependency Plan

## Parallel Batches

- Batch 1: #11, #14
- Batch 2: #12, #16
- Batch 3: #13
- Batch 4: #17
- Batch 5: #15

## Merge Order

#11, #14, #12, #16, #13, #17, #15

## Issue Plans

### Issue #11

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Branch: `feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Worktree: `../CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Suspected files: `src/tui/markdown.rs, src/tui/markdown/table.rs, docs/dev-guardrails.md, src/tui/footer.rs, src/tui/terminal.rs, src/completion_metadata/data.rs, src/completion_metadata/intent.rs, src/runs.rs`
- References: `none`

### Issue #12

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Branch: `feature/issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Worktree: `../CommandAgent-issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Suspected files: `src/providers/openai.rs, src/providers/gemini.rs, Cargo.toml, src/tui/spinner.rs, src/tui/footer.rs, src/tui/interrupt.rs, src/providers/xml_fallback.rs, src/providers/streaming.rs, src/planner/runner.rs, src/minimal_loop/loop_run.rs, docs/dev-guardrails.md, src/providers/ollama.rs, src/tui/repl.rs, src/tui/slash.rs, src/tui/mod.rs, src/tui/markdown.rs, tests/tui_integration.rs`
- References: `none`

### Issue #13

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Branch: `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Worktree: `../CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Suspected files: `src/tui/footer.rs, src/tui/banner.rs, tests/tui_pty.rs, src/lib.rs, src/tui/interrupt.rs, src/planner/runner.rs, src/tui/editor.rs, src/completion_metadata/data.rs`
- References: `none`

### Issue #14

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Branch: `feature/issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Worktree: `../CommandAgent-issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Suspected files: `src/tui/interrupt.rs, src/tui/input_queue.rs, docs/dev-guardrails.md, tests/tui_integration.rs, src/tui/repl.rs, src/tui/mod.rs, src/planner/assurance.rs, src/planner/final_acceptance.rs`
- References: `none`

### Issue #15

- Classification: `strong-dependency`
- Dependency reason: depends on #12, #16, #17
- Branch: `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Worktree: `../CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Suspected files: `docs/mechanism-ledger.md, tests/corpus_regression.rs, README.md, .anvil/config.toml, docs/generality.md, docs/perf-notes.md, docs/uat-corpus.md, docs/uat/scenarios.md, eval/README.md, src/tui/banner.rs, src/tui/repl.rs, tests/tui_pty.rs, src/planner/runner.rs, src/minimal_loop/interaction_probe.rs, docs/migration, workspace/management/runs, src/tui, src/repl.rs, anvil/config.toml, src/time_profile.rs`
- References: `none`

### Issue #16

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Branch: `feature/issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`
- Worktree: `../CommandAgent-issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`
- Suspected files: `src/env_compat.rs, docs/dev-guardrails.md, src/tui/terminal.rs, tests/tui_pty.rs, tests/live_provider.rs, eval/README.md, docs/uat/scenarios.md, .github/workflows/ci.yml, .anvil/config.toml, ~/.anvil/config.toml, README.md, docs/mechanism-ledger.md, src/config.rs, src/tui/footer.rs, src/tui/interrupt.rs, src/tui/markdown.rs, src/tui/spinner.rs, src/eval_events.rs, src/minimal_loop/completion.rs, src/planner/runner.rs, src/minimal_loop/interaction_probe.rs, src/minimal_loop/loop_run.rs, src/tui/ux_demo.rs, src/build_info.rs, scripts/bench.sh, src/state.rs, src/tui/status.rs, github/workflows/ci.yml, anvil/config.toml, src/time_profile.rs`
- References: `none`

### Issue #17

- Classification: `weak-conflict`
- Dependency reason: shared implementation file risk
- Branch: `feature/issue-17-brand-phase-3-decision-internal-protocol-identif`
- Worktree: `../CommandAgent-issue-17-brand-phase-3-decision-internal-protocol-identif`
- Suspected files: `src/planner/profiles/nextjs/knowledge.toml, src/planner/verify.rs, src/planner/contract_attribute_repair.rs, src/planner/state_binding_scan.rs, src/planner/hook_attributes.rs, src/planner/repair_targeting.rs, src/minimal_loop/behavior_evidence.rs, src/minimal_loop/interaction_probe.rs, tests/corpus/apps/*/expectations.toml, src/planner/profiles/data/step_policy.rs, docs/mechanism-ledger.md, manifest.toml, knowledge.toml, docs/dev-guardrails.md, src/planner/runner.rs, src/eval_events.rs, src/planner/profiles/nextjs/manifest.toml, tests/corpus/apps, src/planner/runner/tests, src/cli_panic_boundary.rs, src/model_probe.rs, src/tools/registry.rs, src/tui/status.rs, tests/corpus/apps/test0714_data_manifest_canonicalization/expectations.toml, src/providers/xml_fallback.rs, src/planner/profiles/python_cli.rs, tests/corpus, eval/suites/mvp-smoke.yaml, src/time_profile.rs`
- References: `none`

## Blocked Items

None at dry-run planning time.
