# Worker Sessions

## Issue #43

- Branch: `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #44

- Branch: `feature/issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #45

- Branch: `feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #46

- Branch: `feature/issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #47

- Branch: `feature/issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `processing`
- Running: `True`
- Processing: `True`
- Worker message: worker is processing

## Issue #48

- Branch: `feature/issue-48-ux-bug-stop-streaming-raw-planner-json-into-the`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-48-ux-bug-stop-streaming-raw-planner-json-into-the`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `processing`
- Running: `True`
- Processing: `True`
- Worker message: worker is processing

## Issue #49

- Branch: `feature/issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #50

- Branch: `feature/issue-50-ux-presentation-consistency-unified-elapsed-time`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-50-ux-presentation-consistency-unified-elapsed-time`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `processing`
- Running: `True`
- Processing: `True`
- Worker message: worker is processing

## Issue #51

- Branch: `feature/issue-51-docs-document-repl-multi-line-input-continuation`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-51-docs-document-repl-multi-line-input-continuation`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-47-ux-long-run-awareness-terminal-title-progress-an Codex issue worker task for Issue #47

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

- Title: [ux] Long-run awareness: terminal title progress and completion bell (OSC 2 / BEL)
- Objective: CommandAgent の実 run は数十分に及ぶが、端末タイトルの更新も完了通知も無いため、ユーザーは前景タブでスピナーを見張り続けるしかない。OSC 2 による端末タイトルへの進捗表示と、コマンド完了時の BEL 通知を追加する。

## Acceptance Criteria

- ultra 実行中の phase 遷移で `\x1b]2;<text>\x07` がタイトル用に出力される（PTY テストでバイト列を検証）。
- プロセス終了時にタイトルがクリアされる（空タイトルの OSC 2 出力）。
- 10秒以上かかったコマンドの完了時に BEL が1回出力され、短時間コマンドでは出力されない（時間は注入可能にして unit テスト）。
- `COMMANDAGENT_NO_TERMINAL_TITLE=1` / `COMMANDAGENT_NO_BELL=1`（および `ANVIL_` prefix）でそれぞれ抑止される。
- 非TTLで一切出力されない。`--footer off` でもタイトル/ベルは機能する（下記の注意参照）。
- タイトル文字列は既存 sanitize 方針（制御文字・bidi 無害化）に従い、長さを常識的な上限（例: 120 bytes）で切る。
- 既存イベントスキーマ・footer/spinner の挙動は非破壊。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/footer.rs
- src/tui/spinner.rs
- src/tui/status_bus.rs
- src/env_compat.rs
- footer.rs
- repl.rs
- Cargo.lock
- README.md

## References

- なし

## Required Predecessors

- Issue #43: branch `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`, worktree `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Issue #45: branch `feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o`, worktree `../CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Issue #46: branch `feature/issue-46-ux-first-run-onboarding-startup-provider-diagnos`, worktree `../CommandAgent-issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Issue #49: branch `feature/issue-49-ux-i18n-bug-use-display-width-truncation-for-use`, worktree `../CommandAgent-issue-49-ux-i18n-bug-use-display-width-truncation-for-use`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-47-ux-long-run-awareness-terminal-title-progress-an
- Worktree: ../CommandAgent-issue-47-ux-long-run-awareness-terminal-title-progress-an
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-48-ux-bug-stop-streaming-raw-planner-json-into-the Codex issue worker task for Issue #48

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

- Title: [ux][bug] Stop streaming raw planner JSON into the REPL scrollback
- Objective: `stream=on` のとき、planner 呼び出し（UltraPlan / StepPlan 生成）の応答である**生の JSON 全文**がそのまま端末へストリーム表示される。機械形式の大量ノイズであると同時に、受理入力・Plan card・breadcrumb をスクロールバック上方へ押し流し、#43 の「何を依頼したか見えない」問題を悪化させる。planner scope のターンでは raw ストリーム表示を抑止する。

## Acceptance Criteria

- `stream=on` で `/plan-steps <goal>` / `/ultra-plan-run <goal>` を実行しても、生 JSON（`{"goal":` 等）が stdout/stderr に現れない（PTY テストで検証）。
- planner ターン中のスピナー・breadcrumb・footer 表示は従来どおり。
- executor のストリーミング表示は不変。
- planner ターンを Esc で中断した場合のストリーム後始末（spinner クリア等）にリグレッションが無い。
- 全イベント（名前・キー・値）が非破壊。
- `tests/tui_pty.rs` のストリーミングテストを新仕様（planner 生 JSON 不在＋spinner/footer cleanup の検証は維持）に更新する。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- tests/tui_pty.rs
- lib.rs
- src/planner/runner.rs
- src/tui/mod.rs
- src/planner/ultra_plan_flow.rs
- src/provider_call.rs
- README.md
- docs/guide/en/cli-reference.md

## References

- なし

## Required Predecessors

- Issue #43: branch `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`, worktree `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Issue #44: branch `feature/issue-44-test-bug-pty-suite-never-runs-via-documented-com`, worktree `../CommandAgent-issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Issue #45: branch `feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o`, worktree `../CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Issue #46: branch `feature/issue-46-ux-first-run-onboarding-startup-provider-diagnos`, worktree `../CommandAgent-issue-46-ux-first-run-onboarding-startup-provider-diagnos`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-48-ux-bug-stop-streaming-raw-planner-json-into-the
- Worktree: ../CommandAgent-issue-48-ux-bug-stop-streaming-raw-planner-json-into-the
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-50-ux-presentation-consistency-unified-elapsed-time Codex issue worker task for Issue #50

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

- Title: [ux] Presentation consistency: unified elapsed-time format, ASCII glyph fallback, footer emphasis
- Objective: 端末表示の細部に3つの不統一があり、まとめて解消する: (1) 経過時間の表記が spinner と footer で異なる、(2) breadcrumb・バナー等の Unicode 記号に非UTF-8ロケール向けフォールバックが無い（spinner だけ持っている）、(3) footer が dim 装飾のみで端末によっては読みにくい。

## Acceptance Criteria

- spinner の経過が 61 秒以上で `1m01s` 形式になり、footer と同一関数を使う。
- `LC_ALL=C`（非UTF-8）で REPL / `--ux-demo` を実行しても、breadcrumb・バナー・footer に多バイト記号が出力されない。UTF-8 ロケールでは従来どおり。
- footer の一次情報行が非 dim、設定行が dim になる（`build_live_footer_lines` / `build_footer_line` の unit テスト更新）。
- `COMMANDAGENT_NO_SPINNER` / NO_COLOR 等の既存 env 挙動は不変。
- `src/tui/ux_demo.rs` の scripted デモ（`scripted_demo_contains_full_visual_journey` テスト）と presentation 系スナップショットを更新し、`docs/assets/ux-demo.md` の手順で SVG/GIF 再生成が必要なら #43 の D 項（Demo 実録化）に委ねる旨を PR に明記。
- 既存イベントスキーマ非破壊（記号はあくまで表示層。events.jsonl の値は変えない）。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/banner.rs
- src/tui/ux_demo.rs
- docs/assets/ux-demo.md
- src/tui/spinner.rs
- src/tui/footer.rs
- src/tui/presentation.rs
- src/tui/terminal.rs
- src/tui/editor.rs
- docs/dev/uat/scenarios.md

## References

- なし

## Required Predecessors

- Issue #43: branch `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`, worktree `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Issue #46: branch `feature/issue-46-ux-first-run-onboarding-startup-provider-diagnos`, worktree `../CommandAgent-issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Issue #47: branch `feature/issue-47-ux-long-run-awareness-terminal-title-progress-an`, worktree `../CommandAgent-issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Issue #49: branch `feature/issue-49-ux-i18n-bug-use-display-width-truncation-for-use`, worktree `../CommandAgent-issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Issue #51: branch `feature/issue-51-docs-document-repl-multi-line-input-continuation`, worktree `../CommandAgent-issue-51-docs-document-repl-multi-line-input-continuation`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-50-ux-presentation-consistency-unified-elapsed-time
- Worktree: ../CommandAgent-issue-50-ux-presentation-consistency-unified-elapsed-time
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
