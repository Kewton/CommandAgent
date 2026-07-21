# Worker Sessions

## Issue #11

- Branch: `feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #12

- Branch: `feature/issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #13

- Branch: `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #14

- Branch: `feature/issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #15

- Branch: `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #16

- Branch: `feature/issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #17

- Branch: `feature/issue-17-brand-phase-3-decision-internal-protocol-identif`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-17-brand-phase-3-decision-internal-protocol-identif`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-17-brand-phase-3-decision-internal-protocol-identif Codex issue worker task for Issue #17

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

- Title: [brand] Phase 3 (decision): internal protocol identifiers still named anvil (data-anvil-*, .anvil metadata, anvil_tool_call, anvil_app)
- Objective: Anvil→CommandAgent リネームのPhase 3。**LLMとの動作契約・機械可読データに埋め込まれた "anvil" 識別子**をどう扱うかの**方針判断Issue**。実装前に本Issueでオプションを選択すること(実装Issueは判断後に別途切る)。

## Acceptance Criteria

- Apply approved decision: Adopt Option A. Preserve data-anvil-*, <anvil_tool_call>, anvil_app, .anvil/, JSON keys, event names, and schemas. Change only docs/mechanism-ledger.md to record this decision; make no production-code changes.

## Approved Decision

Adopt Option A. Preserve data-anvil-*, <anvil_tool_call>, anvil_app, .anvil/, JSON keys, event names, and schemas. Change only docs/mechanism-ledger.md to record this decision; make no production-code changes.
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/planner/profiles/nextjs/knowledge.toml
- src/planner/verify.rs
- src/planner/contract_attribute_repair.rs
- src/planner/state_binding_scan.rs
- src/planner/hook_attributes.rs
- src/planner/repair_targeting.rs
- src/minimal_loop/behavior_evidence.rs
- src/minimal_loop/interaction_probe.rs
- tests/corpus/apps/*/expectations.toml
- src/planner/profiles/data/step_policy.rs
- docs/mechanism-ledger.md
- manifest.toml
- knowledge.toml
- docs/dev-guardrails.md
- src/planner/runner.rs
- src/eval_events.rs
- src/planner/profiles/nextjs/manifest.toml
- tests/corpus/apps
- src/planner/runner/tests
- src/cli_panic_boundary.rs
- src/model_probe.rs
- src/tools/registry.rs
- src/tui/status.rs
- tests/corpus/apps/test0714_data_manifest_canonicalization/expectations.toml
- src/providers/xml_fallback.rs
- src/planner/profiles/python_cli.rs
- tests/corpus
- eval/suites/mvp-smoke.yaml
- docs/generality.md

## References

- なし

## Required Predecessors

- Issue #13: branch `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`, worktree `../CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Issue #15: branch `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`, worktree `../CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Issue #16: branch `feature/issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`, worktree `../CommandAgent-issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-17-brand-phase-3-decision-internal-protocol-identif
- Worktree: ../CommandAgent-issue-17-brand-phase-3-decision-internal-protocol-identif
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
