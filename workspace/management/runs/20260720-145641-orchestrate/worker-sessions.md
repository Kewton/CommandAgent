# Worker Sessions

## Issue #43

- Branch: `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: Error: Resource not found. Check the worktree ID.

## Issue #44

- Branch: `feature/issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed dispatch (#43: verification report is missing)

## Issue #45

- Branch: `feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed dispatch (#43: verification report is missing)

## Issue #46

- Branch: `feature/issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed dispatch (#43: verification report is missing)

## Issue #47

- Branch: `feature/issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed dispatch (#43: verification report is missing)

## Issue #48

- Branch: `feature/issue-48-ux-bug-stop-streaming-raw-planner-json-into-the`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-48-ux-bug-stop-streaming-raw-planner-json-into-the`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed dispatch (#43: verification report is missing)

## Issue #49

- Branch: `feature/issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed dispatch (#43: verification report is missing)

## Issue #50

- Branch: `feature/issue-50-ux-presentation-consistency-unified-elapsed-time`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-50-ux-presentation-consistency-unified-elapsed-time`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed dispatch (#43: verification report is missing)

## Issue #51

- Branch: `feature/issue-51-docs-document-repl-multi-line-input-continuation`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-51-docs-document-repl-multi-line-input-continuation`
- Status: `created`
- Message: worktree created
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed dispatch (#43: verification report is missing)

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li Codex issue worker task for Issue #43

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

- Title: [ux][bug] Preserve accepted REPL goals and align live progress with the documented demo
- Objective: 固定フッター有効のREPLで長い `/ultra-plan-run` 指示を確定すると、指示は正しく受理・実行されているにもかかわらず、確定したコマンドとGoalが端末の可視領域・スクロールバック上で確認できなくなる。

## Acceptance Criteria

- REPLでEnter確定したコマンドを、処理開始前にスクロールバックへ再表示する。
- 少なくともcommand種別、Goal、明示profile/style/prompt-layoutを確認できる。
- 日本語・CJK・長文・端末幅を超える折り返しでも消えない。
- footer on/off、color/no-colorの両方で成立する。
- 制御文字・bidi・端末escapeを既存のsanitize方針に従って無害化する。
- Command: /ultra-plan-run
- Goal: あなたが考える最高に面白くかっこいいスペースインベーダーゲームを…
- Profile: nextjs
- Requested port: 3011
- Run: 019f7fca-dc14-7241-bd51-36f0eba856ef
- UltraPlan受付直後、総フェーズ数と最初のフェーズを表示する。
- provider待機中も、現在のscope（planning / implementing / repairing）、phase `N/M`、step、経過時間を確認できる。
- 固定フッターだけでなく、重要な遷移はスクロールバックbreadcrumbとして残す。
- 空応答再試行・quality retry・interrupt requested等をユーザー向けに簡潔に表示する。
- `/status` または同等の操作で、active Goal、Run ID、現在フェーズを再確認できる。
- `--setup-interaction-probe` 成功時はsetup固有の短い結果を表示する。
- generic profileのassurance、`0/1 phases`、無関係なrelease/browser gate表を表示しない。
- 失敗時は失敗箇所とremediationを表示し、成功と失敗の終了コードを維持する。
- 他のdirect action (`--doctor`, `--runs`, completion/man generation等)も汎用coding task summaryの誤適用がないか監査する。
- READMEで、`--ux-demo` がoffline scripted walkthroughであり、通常のprovider-backed runではないことをDemo直下に明記する。
- 手作業のSVG抜粋と実際のターミナル録画を区別する。
- 最新バイナリを使った実際のREPL `/ultra-plan-run` の録画を追加するか、既存Demoを実録へ差し替える。
- 録画には「指示受付」「Goal/profile/port」「phase/step進行」「長いprovider待機」「完了または回復」の実UXを含める。
- README.md / README.ja.mdを同時更新し、doc-driftガードを通す。
- Bの冒頭に追加: 「既存のpresentation / status_bus / footer機構の棚卸しを行い、再現環境で情報が不可視となる機序（ライター競合・scroll region・resize）をPTY screen-stateテストで特定する」。
- テスト要件: PTY回帰テストは `COMMANDAGENT_NO_SPINNER` / `COMMANDAGENT_NO_MARKDOWN` を**設定しない**（実UX合成）ケースを含める。
- C-4の監査結果を反映: 修正対象 = `--setup-interaction-probe` / `--model-probe` / `--plan-steps` / `--ultra-plan`、適用外（安全確認済み）= `--doctor` / `--runs` / `--ux-demo` / completions / man。
- B-4は `documented_activity_ignore_reason` の分類見直し（`empty_response_recovered` 等の昇格）として実装する。
- B-5は上記2-5の再定義を反映する。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/repl.rs
- src/tui/footer.rs
- tests/tui_pty.rs
- README.md
- docs/assets/ux-demo.md
- src/tui/ux_demo.rs
- src/tui/spinner.rs
- src/minimal_loop/loop_run.rs
- footer.rs
- CONTRIBUTING.md
- docs/assets/ux-demo.svg
- src/tui/slash.rs
- src/lib.rs
- src/eval_events.rs
- src/planner/ultra_plan_flow.rs
- src/provider_call.rs
- src/tui/banner.rs
- src/minimal_loop/behavior_evidence.rs

## References

- なし

## Required Predecessors

- None

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li
- Worktree: ../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
