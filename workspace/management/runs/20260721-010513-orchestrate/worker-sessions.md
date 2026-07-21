# Worker Sessions

## Issue #61

- Branch: `feature/issue-61-ux-bug-prevent-stair-step-repl-output-from-lf-on`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-61-ux-bug-prevent-stair-step-repl-output-from-lf-on`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: Error: Resource not found. Check the worktree ID.

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-61-ux-bug-prevent-stair-step-repl-output-from-lf-on Codex issue worker task for Issue #61

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

- Title: [ux][bug] Prevent stair-step REPL output from LF-only writes in raw mode
- Objective: macOS の対話REPLで長い `/ultra-plan-run` を確定すると、`Accepted command` カードの各行が右へ階段状にずれる。

## Acceptance Criteria

- raw mode中でも、受理カードの各論理行が意図した列から始まる。
- `Accepted command`、`- Input:`、`- Command:`、`- Goal:`、profile/style/layout/port/run IDが階段状にずれない。
- 長い日本語・CJK Goalの継続行は、prefixに対応した意図的なインデントだけを持つ。
- footer on/off、color/no-color、wide/narrow terminalで成立する。
- 受理済みGoal全文の保持、scrollback永続化、footer resize/cleanupを壊さない。
- raw mode中に同じLF-only経路を使うMarkdown stream、failure block、status/summary等の複数行出力も監査する。
- event名・JSON schema・`.anvil/` runtime namespaceを変更しない。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/repl.rs
- src/tui/interrupt.rs
- src/tui/command_receipt.rs
- src/tui/footer.rs
- tests/tui_pty.rs
- tests/corpus/apps/test0708_007/fixtures/events-exhaustion-pending-evidence.jsonl
- README.md
- docs/assets/ux-demo.md

## References

- なし

## Required Predecessors

- None

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-61-ux-bug-prevent-stair-step-repl-output-from-lf-on
- Worktree: ../CommandAgent-issue-61-ux-bug-prevent-stair-step-repl-output-from-lf-on
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
