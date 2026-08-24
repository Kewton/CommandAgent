# Worker Sessions

## Issue #43

- Branch: `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `processing`
- Running: `True`
- Processing: `True`
- Worker message: worker is processing

## Issue #44

- Branch: `feature/issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `processing`
- Running: `True`
- Processing: `True`
- Worker message: worker is processing

## Issue #45

- Branch: `feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `started-but-idle`
- Running: `True`
- Processing: `False`
- Worker message: worker session is running but not processing

## Issue #46

- Branch: `feature/issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-46-ux-first-run-onboarding-startup-provider-diagnos`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `processing`
- Running: `True`
- Processing: `True`
- Worker message: worker is processing

## Issue #47

- Branch: `feature/issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-47-ux-long-run-awareness-terminal-title-progress-an`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 3 failed worker verification (#46: verification report is missing)

## Issue #48

- Branch: `feature/issue-48-ux-bug-stop-streaming-raw-planner-json-into-the`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-48-ux-bug-stop-streaming-raw-planner-json-into-the`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 3 failed worker verification (#46: verification report is missing)

## Issue #49

- Branch: `feature/issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-49-ux-i18n-bug-use-display-width-truncation-for-use`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `processing`
- Running: `True`
- Processing: `True`
- Worker message: worker is processing

## Issue #50

- Branch: `feature/issue-50-ux-presentation-consistency-unified-elapsed-time`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-50-ux-presentation-consistency-unified-elapsed-time`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 3 failed worker verification (#46: verification report is missing)

## Issue #51

- Branch: `feature/issue-51-docs-document-repl-multi-line-input-continuation`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-51-docs-document-repl-multi-line-input-continuation`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `processing`
- Running: `True`
- Processing: `True`
- Worker message: worker is processing

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
- `commandmatedev send commandagent-feature-issue-44-test-bug-pty-suite-never-runs-via-documented-com Codex issue worker task for Issue #44

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

- Title: [test][bug] PTY suite never runs via documented commands (#[ignore] + missing --include-ignored)
- Objective: `tests/tui_pty.rs` の3テストは全て `#[test]` + `#[ignore]` かつ `ANVIL_PTY_TESTS=1`（現行名 `COMMANDAGENT_PTY_TESTS`、`env_compat` により両対応）の環境変数ゲートで二重に守られている。しかし、文書化された実行コマンド（`justfile` の `test-pty` レシピ、および `CONTRIBUTING.md` の "Run the opt-in pseudo-terminal integration suite"）は `--include-ignored` を渡していないため、**テストが1件も実行されないまま exit 0 で成功終了する**。

## Acceptance Criteria

- `just test-pty` が3テストを実際に実行する（出力が `0 passed` にならないこと、`3 passed` を確認）。
- `CONTRIBUTING.md` の該当コマンドが実際にテストを実行する形に更新されている。
- `#[ignore]` の要否について方針をコード/ドキュメントのどちらかに一行で残す。
- （任意）doc-drift guard（#22 の仕組み）でテスト起動コマンドの再乖離を検出できるなら追加する。
- （任意）macOS/Linux runner での opt-in CI job を検討する。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- tests/tui_pty.rs
- CONTRIBUTING.md
- .github/workflows/ci.yml
- release.yml
- github/workflows/ci.yml
- docs/codex-harness.md
- docs/dev/generality.md
- docs/dev/mechanism-ledger.md

## References

- なし

## Required Predecessors

- Issue #43: branch `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`, worktree `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-44-test-bug-pty-suite-never-runs-via-documented-com
- Worktree: ../CommandAgent-issue-44-test-bug-pty-suite-never-runs-via-documented-com
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-45-ux-bug-repl-failure-output-render-each-failure-o Codex issue worker task for Issue #45

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

- Title: [ux][bug] REPL failure output: render each failure once; stop framing typos and user interrupts as TASK FAILED
- Objective: REPLの失敗系出力に3つの問題が重なっており、軽微な入力ミスや意図的な中断まで「タスク失敗」として大量出力される。失敗の種類（未知コマンド / 非slash自由文 / ユーザー割り込み / 実際の失敗）ごとに出口を分け、各失敗を**正確に1回**表示するようにする。

## Acceptance Criteria

- `/hepl` と入力: 候補提示を含む1〜3行の案内のみ。TASK FAILED ブロック・Terminal summary・`error:` の重複が一切出ない。
- 非slashの平文（日本語含む）を入力: 実行されず、/ultra-plan-run / /plan-run への誘導文のみ表示される。
- 未知コマンド・自由文では `tui_command_start` / `tui_command_stop` イベントと summary 生成を行わない（コマンド実行前の入力エラー扱い）。既存イベントの名前・キー・スキーマは非破壊。
- 実行中に Esc/Ctrl-C: INTERRUPTED 表示が1回出て、再開手段が具体的に示される。「TASK FAILED」の文言が出ない。
- 実失敗（例: provider 接続不可）: 失敗表示が正確に1回、markdown renderer 経由で出る。
- 入力エコー部は制御文字・bidi・端末escapeを既存sanitize方針で無害化する。
- footer on/off、NO_COLOR の両方で成立する。
- `/help` 文言を変更した場合、`tests/doc_drift.rs`（`render_help` を固定。94行・120行・128行参照）を更新して通す。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- status.rs
- repl.rs
- tests/doc_drift.rs
- tests/tui_repl.rs
- tests/tui_integration.rs
- src/tui/slash.rs
- src/tui/repl.rs
- src/eval_events.rs
- src/tui/status.rs
- src/planner/ultra_plan_flow.rs
- src/provider_call.rs
- README.md

## References

- なし

## Required Predecessors

- Issue #43: branch `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`, worktree `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o
- Worktree: ../CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-49-ux-i18n-bug-use-display-width-truncation-for-use Codex issue worker task for Issue #49

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

- Title: [ux][i18n][bug] Use display-width truncation for user-visible text (CJK currently gets ~1/3 the budget)
- Objective: ユーザー表示向けの文字列切り詰めがバイト長基準のため、日本語等の CJK テキストは ASCII の約 1/3 の長さで `...` 省略される。Plan card の Goal は「120」の予算に対して**日本語では約40文字**しか表示されない。表示系の切り詰めを表示列幅（display width）基準へ統一する。

## Acceptance Criteria

- 日本語 Goal が Plan card で従来比約3倍（列幅120相当＝約60文字）表示される。ASCII の表示長は不変。
- `display_width` / `char_display_width` / `display_width_ansi` を共有場所（例: `src/tui/terminal.rs` または `src/util.rs`）へ移し、footer と presentation が同一実装を使う。footer の既存挙動（ANSI エスケープを幅0として読み飛ばす等）は不変。
- 表示用の新 API（例: `fit_display_width(value, cols, marker)`）を導入し、`presentation::fit` の全used箇所を置き換える。`excerpt_with_marker` は記録系用途に残す。
- `events.jsonl` に書かれる値（`body_snippet` 等の長さ・内容）が変わらない（golden / conformance テスト非破壊）。
- 文字境界で panic しない（日本語・絵文字・結合文字・ANSI 込み文字列の unit テスト）。
- `input_queue::preview` / spinner label / `sanitize_command_excerpt` を監査し、表示系なら同 API へ寄せる（対象外と判断した場合は理由を PR に記載）。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/terminal.rs
- src/util.rs
- footer.rs
- src/tui/presentation.rs
- src/tui/footer.rs
- Cargo.lock
- docs/dev/profile-manifest.md
- docs/guide/en/configuration.md

## References

- なし

## Required Predecessors

- Issue #43: branch `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`, worktree `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-49-ux-i18n-bug-use-display-width-truncation-for-use
- Worktree: ../CommandAgent-issue-49-ux-i18n-bug-use-display-width-truncation-for-use
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-46-ux-first-run-onboarding-startup-provider-diagnos Codex issue worker task for Issue #46

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

- Title: [ux] First-run onboarding: startup provider diagnostics, actionable error remediation, banner /help hint
- Objective: REPL は起動時に provider への到達性を一切確認せず、バナーにも導線が無いため、初見ユーザー・環境不備ユーザーは「普通に起動したのに初回コマンドで生のHTTPエラーが出る」体験になる。起動時の軽量診断、providerエラーへの是正ヒント付与、バナーへの `/help` 導線を追加する。

## Acceptance Criteria

- Ollama 停止状態で REPL 起動: host と是正手順を含む警告が出るが、プロンプトは表示され操作を継続できる。
- 起動遅延: サーバ到達時は体感ゼロ（tags 1回分）、不達時もタイムアウト上限（~2秒）以内。
- モデル未取得で起動: `ollama pull <model>` を含む警告が出る。
- 実行時の接続失敗・404・認証エラーそれぞれに是正ヒント行が付く（unit テストで文言を固定）。
- `--offline` 指定時・非TTY（`--prompt` 等）ではプローブを実行しない。
- OPENAI/GEMINI キー未設定の起動失敗メッセージに設定手順と `--doctor` が含まれる。
- バナーに `/help` / `/doctor` 導線行が追加される（`--ux-demo` のバナー描画・関連スナップショットも更新）。
- 既存イベントスキーマは非破壊（警告をイベント化する場合は additive な新イベントとする）。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/doctor.rs
- repl.rs
- tests/tui_pty.rs
- src/tui/repl.rs
- src/providers/mod.rs
- src/providers/ollama.rs
- src/tui/banner.rs
- docs/dev/generality.md

## References

- なし

## Required Predecessors

- Issue #43: branch `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`, worktree `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Issue #44: branch `feature/issue-44-test-bug-pty-suite-never-runs-via-documented-com`, worktree `../CommandAgent-issue-44-test-bug-pty-suite-never-runs-via-documented-com`
- Issue #45: branch `feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o`, worktree `../CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-46-ux-first-run-onboarding-startup-provider-diagnos
- Worktree: ../CommandAgent-issue-46-ux-first-run-onboarding-startup-provider-diagnos
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-51-docs-document-repl-multi-line-input-continuation Codex issue worker task for Issue #51

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

- Title: [docs] Document REPL multi-line input continuation (/help + user guide)
- Objective: REPL の複数行入力（継続入力）は実装済みだが、`/help` にも利用者ガイドにも一切記載が無く、偶然以外に発見できない。`/help` と `docs/guide`（en/ja）に記載を追加する。コード変更は `/help` 文言のみの小型 Issue。

## Acceptance Criteria

- `/help` 出力に継続入力の説明が含まれる。
- `docs/guide/en` / `docs/guide/ja` の両方に同内容の節があり、EN/JA パリティが保たれている。
- `tests/doc_drift.rs` は `render_help` の内容を固定している（`doc_drift.rs:94,120,128` 付近）。ヘルプ文言変更に合わせて drift 側の期待値・対応ドキュメントを更新し、テストを通す。
- `cargo fmt --all -- --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test`

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/editor.rs
- tests/doc_drift.rs
- docs/guide
- src/tui/slash.rs
- docs/guide/en
- docs/guide/ja
- docs/README.md
- docs/intent-skeleton.md

## References

- なし

## Required Predecessors

- Issue #43: branch `feature/issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`, worktree `../CommandAgent-issue-43-ux-bug-preserve-accepted-repl-goals-and-align-li`
- Issue #45: branch `feature/issue-45-ux-bug-repl-failure-output-render-each-failure-o`, worktree `../CommandAgent-issue-45-ux-bug-repl-failure-output-render-each-failure-o`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-51-docs-document-repl-multi-line-input-continuation
- Worktree: ../CommandAgent-issue-51-docs-document-repl-multi-line-input-continuation
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
