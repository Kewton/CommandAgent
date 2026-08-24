# UAT Report

## Merge Gate

- Status: `passed`
- Message: all 79 UAT scenarios passed with evidence

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #43: [ux][bug] Preserve accepted REPL goals and align live progress with the documented demo

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `REPLでEnter確定したコマンドを、処理開始前にスクロールバックへ再表示する。` を確認できる画面または実機操作を行う。
- 期待結果: REPLでEnter確定したコマンドを、処理開始前にスクロールバックへ再表示する。
- Actual: The accepted /ultra-plan-run command is written to scrollback before execution begins.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `少なくともcommand種別、Goal、明示profile/style/prompt-layoutを確認できる。` を確認できる画面または実機操作を行う。
- 期待結果: 少なくともcommand種別、Goal、明示profile/style/prompt-layoutを確認できる。
- Actual: The receipt exposes command, Goal, explicit profile, style, and prompt-layout fields.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `日本語・CJK・長文・端末幅を超える折り返しでも消えない。` を確認できる画面または実機操作を行う。
- 期待結果: 日本語・CJK・長文・端末幅を超える折り返しでも消えない。
- Actual: The PTY screen-state test preserved a long Japanese goal through terminal-width wrapping.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `footer on/off、color/no-colorの両方で成立する。` を確認できる画面または実機操作を行う。
- 期待結果: footer on/off、color/no-colorの両方で成立する。
- Actual: The PTY matrix passed with footer enabled/disabled and color enabled/disabled.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `制御文字・bidi・端末escapeを既存のsanitize方針に従って無害化する。` を確認できる画面または実機操作を行う。
- 期待結果: 制御文字・bidi・端末escapeを既存のsanitize方針に従って無害化する。
- Actual: Receipt text sanitization tests stripped terminal controls, bidi controls, and escapes.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Command: /ultra-plan-run` を確認できる画面または実機操作を行う。
- 期待結果: Command: /ultra-plan-run
- Actual: The receipt renders Command: /ultra-plan-run.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Goal: あなたが考える最高に面白くかっこいいスペースインベーダーゲームを…` を確認できる画面または実機操作を行う。
- 期待結果: Goal: あなたが考える最高に面白くかっこいいスペースインベーダーゲームを…
- Actual: The receipt renders the specified long Japanese Goal without losing it.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Profile: nextjs` を確認できる画面または実機操作を行う。
- 期待結果: Profile: nextjs
- Actual: The receipt renders Profile: nextjs.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Requested port: 3011` を確認できる画面または実機操作を行う。
- 期待結果: Requested port: 3011
- Actual: The receipt renders Requested port: 3011.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Run: 019f7fca-dc14-7241-bd51-36f0eba856ef` を確認できる画面または実機操作を行う。
- 期待結果: Run: 019f7fca-dc14-7241-bd51-36f0eba856ef
- Actual: Deterministic receipt/status tests render Run ID 019f7fca-dc14-7241-bd51-36f0eba856ef; the refreshed recording correctly uses its newly generated Run ID.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `UltraPlan受付直後、総フェーズ数と最初のフェーズを表示する。` を確認できる画面または実機操作を行う。
- 期待結果: UltraPlan受付直後、総フェーズ数と最初のフェーズを表示する。
- Actual: UltraPlan acceptance projection shows total phase count and the first phase.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `provider待機中も、現在のscope（planning / implementing / repairing）、phase `N/M`、step、経過時間を確認できる。` を確認できる画面または実機操作を行う。
- 期待結果: provider待機中も、現在のscope（planning / implementing / repairing）、phase `N/M`、step、経過時間を確認できる。
- Actual: Live status tests show planning/implementing/repairing scope, N/M phase, step, and elapsed time during provider waits.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `固定フッターだけでなく、重要な遷移はスクロールバックbreadcrumbとして残す。` を確認できる画面または実機操作を行う。
- 期待結果: 固定フッターだけでなく、重要な遷移はスクロールバックbreadcrumbとして残す。
- Actual: Important transitions remain as scrollback breadcrumbs in addition to the live footer.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `空応答再試行・quality retry・interrupt requested等をユーザー向けに簡潔に表示する。` を確認できる画面または実機操作を行う。
- 期待結果: 空応答再試行・quality retry・interrupt requested等をユーザー向けに簡潔に表示する。
- Actual: Empty-response retry, quality retry, and interrupt-requested transitions are promoted to concise user-visible activity.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/status` または同等の操作で、active Goal、Run ID、現在フェーズを再確認できる。` を確認できる画面または実機操作を行う。
- 期待結果: `/status` または同等の操作で、active Goal、Run ID、現在フェーズを再確認できる。
- Actual: The status card reports active Goal, Run ID, and current phase.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 16

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``--setup-interaction-probe` 成功時はsetup固有の短い結果を表示する。` を確認できる画面または実機操作を行う。
- 期待結果: `--setup-interaction-probe` 成功時はsetup固有の短い結果を表示する。
- Actual: The setup interaction probe renders its short setup-specific result.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 17

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `generic profileのassurance、`0/1 phases`、無関係なrelease/browser gate表を表示しない。` を確認できる画面または実機操作を行う。
- 期待結果: generic profileのassurance、`0/1 phases`、無関係なrelease/browser gate表を表示しない。
- Actual: Direct-action tests confirm setup output omits generic assurance, phase counts, and unrelated gate tables.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 18

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `失敗時は失敗箇所とremediationを表示し、成功と失敗の終了コードを維持する。` を確認できる画面または実機操作を行う。
- 期待結果: 失敗時は失敗箇所とremediationを表示し、成功と失敗の終了コードを維持する。
- Actual: Probe failure tests retain failure exit status and render the failure location plus remediation.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 19

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `他のdirect action (`--doctor`, `--runs`, completion/man generation等)も汎用coding task summaryの誤適用がないか監査する。` を確認できる画面または実機操作を行う。
- 期待結果: 他のdirect action (`--doctor`, `--runs`, completion/man generation等)も汎用coding task summaryの誤適用がないか監査する。
- Actual: The direct-action audit confirms doctor, runs, UX demo, completions, and man output do not receive the generic coding-task summary.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 20

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `READMEで、`--ux-demo` がoffline scripted walkthroughであり、通常のprovider-backed runではないことをDemo直下に明記する。` を確認できる画面または実機操作を行う。
- 期待結果: READMEで、`--ux-demo` がoffline scripted walkthroughであり、通常のprovider-backed runではないことをDemo直下に明記する。
- Actual: README and README.ja identify --ux-demo as an offline scripted walkthrough rather than a provider-backed run.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 21

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `手作業のSVG抜粋と実際のターミナル録画を区別する。` を確認できる画面または実機操作を行う。
- 期待結果: 手作業のSVG抜粋と実際のターミナル録画を区別する。
- Actual: The bilingual README text distinguishes hand-authored SVG excerpts from real terminal recordings.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 22

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `最新バイナリを使った実際のREPL `/ultra-plan-run` の録画を追加するか、既存Demoを実録へ差し替える。` を確認できる画面または実機操作を行う。
- 期待結果: 最新バイナリを使った実際のREPL `/ultra-plan-run` の録画を追加するか、既存Demoを実録へ差し替える。
- Actual: A current real REPL /ultra-plan-run recording is present at docs/assets/repl-ultra-plan-run.rec.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 23

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `録画には「指示受付」「Goal/profile/port」「phase/step進行」「長いprovider待機」「完了または回復」の実UXを含める。` を確認できる画面または実機操作を行う。
- 期待結果: 録画には「指示受付」「Goal/profile/port」「phase/step進行」「長いprovider待機」「完了または回復」の実UXを含める。
- Actual: The recording contains command acceptance, Goal/profile/port, phase and step progress, a long provider wait, and recovery/completion context.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 24

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `README.md / README.ja.mdを同時更新し、doc-driftガードを通す。` を確認できる画面または実機操作を行う。
- 期待結果: README.md / README.ja.mdを同時更新し、doc-driftガードを通す。
- Actual: README.md and README.ja were updated together and all documentation-drift assertions passed.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 25

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Bの冒頭に追加: 「既存のpresentation / status_bus / footer機構の棚卸しを行い、再現環境で情報が不可視となる機序（ライター競合・scroll region・resize）をPTY screen-stateテストで特定する」。` を確認できる画面または実機操作を行う。
- 期待結果: Bの冒頭に追加: 「既存のpresentation / status_bus / footer機構の棚卸しを行い、再現環境で情報が不可視となる機序（ライター競合・scroll region・resize）をPTY screen-stateテストで特定する」。
- Actual: The implementation audit used PTY screen state to cover scroll-region, resize, footer, and presentation interaction.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 26

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `テスト要件: PTY回帰テストは `COMMANDAGENT_NO_SPINNER` / `COMMANDAGENT_NO_MARKDOWN` を**設定しない**（実UX合成）ケースを含める。` を確認できる画面または実機操作を行う。
- 期待結果: テスト要件: PTY回帰テストは `COMMANDAGENT_NO_SPINNER` / `COMMANDAGENT_NO_MARKDOWN` を**設定しない**（実UX合成）ケースを含める。
- Actual: The long-goal PTY case leaves spinner and Markdown enabled, exercising the composed real UX.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 27

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `C-4の監査結果を反映: 修正対象 = `--setup-interaction-probe` / `--model-probe` / `--plan-steps` / `--ultra-plan`、適用外（安全確認済み）= `--doctor` / `--runs` / `--ux-demo` / completions / man。` を確認できる画面または実機操作を行う。
- 期待結果: C-4の監査結果を反映: 修正対象 = `--setup-interaction-probe` / `--model-probe` / `--plan-steps` / `--ultra-plan`、適用外（安全確認済み）= `--doctor` / `--runs` / `--ux-demo` / completions / man。
- Actual: The direct-action audit fixed execution probes and verified doctor, runs, UX demo, completions, and man as safe exclusions.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 28

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `B-4は `documented_activity_ignore_reason` の分類見直し（`empty_response_recovered` 等の昇格）として実装する。` を確認できる画面または実機操作を行う。
- 期待結果: B-4は `documented_activity_ignore_reason` の分類見直し（`empty_response_recovered` 等の昇格）として実装する。
- Actual: Activity projection tests promote empty_response_recovered and related notable normalization instead of silently ignoring it.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

#### Scenario 29

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `B-5は上記2-5の再定義を反映する。` を確認できる画面または実機操作を行う。
- 期待結果: B-5は上記2-5の再定義を反映する。
- Actual: The final PTY and presentation matrix covers the redefined receipt, status, activity, and visibility requirements.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored (7 passed); cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed); source/recording audit.
- Result: passed

### Issue #44: [test][bug] PTY suite never runs via documented commands (#[ignore] + missing --include-ignored)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``just test-pty` が3テストを実際に実行する（出力が `0 passed` にならないこと、`3 passed` を確認）。` を確認できる画面または実機操作を行う。
- 期待結果: `just test-pty` が3テストを実際に実行する（出力が `0 passed` にならないこと、`3 passed` を確認）。
- Actual: The documented PTY recipe executed seven ignored PTY tests on the combined candidate; it did not report 0 passed.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: exact documented cargo PTY recipe (7 passed); cargo test --test doc_drift (7 passed); justfile and CONTRIBUTING.md command audit.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``CONTRIBUTING.md` の該当コマンドが実際にテストを実行する形に更新されている。` を確認できる画面または実機操作を行う。
- 期待結果: `CONTRIBUTING.md` の該当コマンドが実際にテストを実行する形に更新されている。
- Actual: CONTRIBUTING.md contains the executable ANVIL_PTY_TESTS=1 cargo test --test tui_pty -- --include-ignored command.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: exact documented cargo PTY recipe (7 passed); cargo test --test doc_drift (7 passed); justfile and CONTRIBUTING.md command audit.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``#[ignore]` の要否について方針をコード/ドキュメントのどちらかに一行で残す。` を確認できる画面または実機操作を行う。
- 期待結果: `#[ignore]` の要否について方針をコード/ドキュメントのどちらかに一行で残す。
- Actual: CONTRIBUTING.md documents why both the environment opt-in and libtest ignored-test flag are required.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: exact documented cargo PTY recipe (7 passed); cargo test --test doc_drift (7 passed); justfile and CONTRIBUTING.md command audit.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `（任意）doc-drift guard（#22 の仕組み）でテスト起動コマンドの再乖離を検出できるなら追加する。` を確認できる画面または実機操作を行う。
- 期待結果: （任意）doc-drift guard（#22 の仕組み）でテスト起動コマンドの再乖離を検出できるなら追加する。
- Actual: A documentation-drift test now rejects a recipe that omits --include-ignored or diverges from CONTRIBUTING.md.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: exact documented cargo PTY recipe (7 passed); cargo test --test doc_drift (7 passed); justfile and CONTRIBUTING.md command audit.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `（任意）macOS/Linux runner での opt-in CI job を検討する。` を確認できる画面または実機操作を行う。
- 期待結果: （任意）macOS/Linux runner での opt-in CI job を検討する。
- Actual: The optional CI-runner expansion was considered; PTY remains an explicit local opt-in because hosted terminal support is not guaranteed.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: exact documented cargo PTY recipe (7 passed); cargo test --test doc_drift (7 passed); justfile and CONTRIBUTING.md command audit.
- Result: passed

### Issue #45: [ux][bug] REPL failure output: render each failure once; stop framing typos and user interrupts as TASK FAILED

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/hepl` と入力: 候補提示を含む1〜3行の案内のみ。TASK FAILED ブロック・Terminal summary・`error:` の重複が一切出ない。` を確認できる画面または実機操作を行う。
- 期待結果: `/hepl` と入力: 候補提示を含む1〜3行の案内のみ。TASK FAILED ブロック・Terminal summary・`error:` の重複が一切出ない。
- Actual: The typo /hepl produces only concise suggestion/help guidance, with no TASK FAILED block, terminal summary, or duplicate error prefix.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed).
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `非slashの平文（日本語含む）を入力: 実行されず、/ultra-plan-run / /plan-run への誘導文のみ表示される。` を確認できる画面または実機操作を行う。
- 期待結果: 非slashの平文（日本語含む）を入力: 実行されず、/ultra-plan-run / /plan-run への誘導文のみ表示される。
- Actual: Plain Japanese and other non-slash text is not executed and points only to /ultra-plan-run and /plan-run entry points.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed).
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `未知コマンド・自由文では `tui_command_start` / `tui_command_stop` イベントと summary 生成を行わない（コマンド実行前の入力エラー扱い）。既存イベントの名前・キー・スキーマは非破壊。` を確認できる画面または実機操作を行う。
- 期待結果: 未知コマンド・自由文では `tui_command_start` / `tui_command_stop` イベントと summary 生成を行わない（コマンド実行前の入力エラー扱い）。既存イベントの名前・キー・スキーマは非破壊。
- Actual: Invalid pre-command input emits neither tui_command_start/stop events nor a generated summary; existing event schemas remain unchanged.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed).
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `実行中に Esc/Ctrl-C: INTERRUPTED 表示が1回出て、再開手段が具体的に示される。「TASK FAILED」の文言が出ない。` を確認できる画面または実機操作を行う。
- 期待結果: 実行中に Esc/Ctrl-C: INTERRUPTED 表示が1回出て、再開手段が具体的に示される。「TASK FAILED」の文言が出ない。
- Actual: Interrupt output is one distinct INTERRUPTED block with concrete resume/rerun guidance and no TASK FAILED wording.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed).
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `実失敗（例: provider 接続不可）: 失敗表示が正確に1回、markdown renderer 経由で出る。` を確認できる画面または実機操作を行う。
- 期待結果: 実失敗（例: provider 接続不可）: 失敗表示が正確に1回、markdown renderer 経由で出る。
- Actual: A provider failure renders exactly one failure block through the Markdown renderer.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed).
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `入力エコー部は制御文字・bidi・端末escapeを既存sanitize方針で無害化する。` を確認できる画面または実機操作を行う。
- 期待結果: 入力エコー部は制御文字・bidi・端末escapeを既存sanitize方針で無害化する。
- Actual: Echoed invalid input is sanitized for terminal controls, bidi controls, and escape sequences.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed).
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `footer on/off、NO_COLOR の両方で成立する。` を確認できる画面または実機操作を行う。
- 期待結果: footer on/off、NO_COLOR の両方で成立する。
- Actual: Failure and input-guidance behavior passed footer on/off and color/no-color coverage in the combined TUI suites.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed).
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/help` 文言を変更した場合、`tests/doc_drift.rs`（`render_help` を固定。94行・120行・128行参照）を更新して通す。` を確認できる画面または実機操作を行う。
- 期待結果: `/help` 文言を変更した場合、`tests/doc_drift.rs`（`render_help` を固定。94行・120行・128行参照）を更新して通す。
- Actual: The rendered help contract and documentation-drift suite pass with the updated wording.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); cargo test --test tui_integration (22 passed, 1 ignored); cargo test --test doc_drift (7 passed).
- Result: passed

### Issue #46: [ux] First-run onboarding: startup provider diagnostics, actionable error remediation, banner /help hint

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Ollama 停止状態で REPL 起動: host と是正手順を含む警告が出るが、プロンプトは表示され操作を継続できる。` を確認できる画面または実機操作を行う。
- 期待結果: Ollama 停止状態で REPL 起動: host と是正手順を含む警告が出るが、プロンプトは表示され操作を継続できる。
- Actual: With Ollama unreachable, the PTY shows host/remediation guidance and still reaches a usable prompt.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY onboarding cases passed; cargo test providers:: (52 passed); cargo test --test provider_onboarding (1 passed); LC_ALL=C UX demo passed.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `起動遅延: サーバ到達時は体感ゼロ（tags 1回分）、不達時もタイムアウト上限（~2秒）以内。` を確認できる画面または実機操作を行う。
- 期待結果: 起動遅延: サーバ到達時は体感ゼロ（tags 1回分）、不達時もタイムアウト上限（~2秒）以内。
- Actual: Startup tests perform one tags request when reachable and bound the unreachable probe to approximately two seconds.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY onboarding cases passed; cargo test providers:: (52 passed); cargo test --test provider_onboarding (1 passed); LC_ALL=C UX demo passed.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `モデル未取得で起動: `ollama pull <model>` を含む警告が出る。` を確認できる画面または実機操作を行う。
- 期待結果: モデル未取得で起動: `ollama pull <model>` を含む警告が出る。
- Actual: With the configured Ollama model absent, the PTY warning includes ollama pull <model> and leaves the prompt usable.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY onboarding cases passed; cargo test providers:: (52 passed); cargo test --test provider_onboarding (1 passed); LC_ALL=C UX demo passed.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `実行時の接続失敗・404・認証エラーそれぞれに是正ヒント行が付く（unit テストで文言を固定）。` を確認できる画面または実機操作を行う。
- 期待結果: 実行時の接続失敗・404・認証エラーそれぞれに是正ヒント行が付く（unit テストで文言を固定）。
- Actual: Connection, not-found/404, and authentication failures each have fixed actionable remediation lines.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY onboarding cases passed; cargo test providers:: (52 passed); cargo test --test provider_onboarding (1 passed); LC_ALL=C UX demo passed.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``--offline` 指定時・非TTY（`--prompt` 等）ではプローブを実行しない。` を確認できる画面または実機操作を行う。
- 期待結果: `--offline` 指定時・非TTY（`--prompt` 等）ではプローブを実行しない。
- Actual: Offline and non-interactive actions skip the startup provider probe.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY onboarding cases passed; cargo test providers:: (52 passed); cargo test --test provider_onboarding (1 passed); LC_ALL=C UX demo passed.
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `OPENAI/GEMINI キー未設定の起動失敗メッセージに設定手順と `--doctor` が含まれる。` を確認できる画面または実機操作を行う。
- 期待結果: OPENAI/GEMINI キー未設定の起動失敗メッセージに設定手順と `--doctor` が含まれる。
- Actual: Missing OpenAI and Gemini keys report setup steps and direct the user to --doctor.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY onboarding cases passed; cargo test providers:: (52 passed); cargo test --test provider_onboarding (1 passed); LC_ALL=C UX demo passed.
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `バナーに `/help` / `/doctor` 導線行が追加される（`--ux-demo` のバナー描画・関連スナップショットも更新）。` を確認できる画面または実機操作を行う。
- 期待結果: バナーに `/help` / `/doctor` 導線行が追加される（`--ux-demo` のバナー描画・関連スナップショットも更新）。
- Actual: The banner and scripted UX demo include the /help and /doctor guidance line.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY onboarding cases passed; cargo test providers:: (52 passed); cargo test --test provider_onboarding (1 passed); LC_ALL=C UX demo passed.
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `既存イベントスキーマは非破壊（警告をイベント化する場合は additive な新イベントとする）。` を確認できる画面または実機操作を行う。
- 期待結果: 既存イベントスキーマは非破壊（警告をイベント化する場合は additive な新イベントとする）。
- Actual: The onboarding change passes the event/integration suite without changing existing event names or fields.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY onboarding cases passed; cargo test providers:: (52 passed); cargo test --test provider_onboarding (1 passed); LC_ALL=C UX demo passed.
- Result: passed

### Issue #47: [ux] Long-run awareness: terminal title progress and completion bell (OSC 2 / BEL)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `ultra 実行中の phase 遷移で `\x1b]2;<text>\x07` がタイトル用に出力される（PTY テストでバイト列を検証）。` を確認できる画面または実機操作を行う。
- 期待結果: ultra 実行中の phase 遷移で `\x1b]2;<text>\x07` がタイトル用に出力される（PTY テストでバイト列を検証）。
- Actual: Phase-start notification tests emit the exact OSC 2 terminal-title byte sequence.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed), including terminal_notifications exact-byte, timing, environment, sanitization tests; PTY and full suites passed.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `プロセス終了時にタイトルがクリアされる（空タイトルの OSC 2 出力）。` を確認できる画面または実機操作を行う。
- 期待結果: プロセス終了時にタイトルがクリアされる（空タイトルの OSC 2 出力）。
- Actual: Command completion clears the title with one empty OSC 2 sequence.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed), including terminal_notifications exact-byte, timing, environment, sanitization tests; PTY and full suites passed.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `10秒以上かかったコマンドの完了時に BEL が1回出力され、短時間コマンドでは出力されない（時間は注入可能にして unit テスト）。` を確認できる画面または実機操作を行う。
- 期待結果: 10秒以上かかったコマンドの完了時に BEL が1回出力され、短時間コマンドでは出力されない（時間は注入可能にして unit テスト）。
- Actual: Injected-time tests emit one BEL at the ten-second threshold and none for short commands.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed), including terminal_notifications exact-byte, timing, environment, sanitization tests; PTY and full suites passed.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``COMMANDAGENT_NO_TERMINAL_TITLE=1` / `COMMANDAGENT_NO_BELL=1`（および `ANVIL_` prefix）でそれぞれ抑止される。` を確認できる画面または実機操作を行う。
- 期待結果: `COMMANDAGENT_NO_TERMINAL_TITLE=1` / `COMMANDAGENT_NO_BELL=1`（および `ANVIL_` prefix）でそれぞれ抑止される。
- Actual: Current COMMANDAGENT and legacy ANVIL disable variables independently suppress title and bell output.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed), including terminal_notifications exact-byte, timing, environment, sanitization tests; PTY and full suites passed.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `非TTLで一切出力されない。`--footer off` でもタイトル/ベルは機能する（下記の注意参照）。` を確認できる画面または実機操作を行う。
- 期待結果: 非TTLで一切出力されない。`--footer off` でもタイトル/ベルは機能する（下記の注意参照）。
- Actual: Non-TTY output is suppressed while terminal notifications remain independent of footer visibility.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed), including terminal_notifications exact-byte, timing, environment, sanitization tests; PTY and full suites passed.
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `タイトル文字列は既存 sanitize 方針（制御文字・bidi 無害化）に従い、長さを常識的な上限（例: 120 bytes）で切る。` を確認できる画面または実機操作を行う。
- 期待結果: タイトル文字列は既存 sanitize 方針（制御文字・bidi 無害化）に従い、長さを常識的な上限（例: 120 bytes）で切る。
- Actual: Title text strips controls and bidi markers and is capped safely at the configured UTF-8 byte limit.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed), including terminal_notifications exact-byte, timing, environment, sanitization tests; PTY and full suites passed.
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `既存イベントスキーマ・footer/spinner の挙動は非破壊。` を確認できる画面または実機操作を行う。
- 期待結果: 既存イベントスキーマ・footer/spinner の挙動は非破壊。
- Actual: The full TUI, PTY, and integration suites preserve event schemas and footer/spinner behavior.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed), including terminal_notifications exact-byte, timing, environment, sanitization tests; PTY and full suites passed.
- Result: passed

### Issue #48: [ux][bug] Stop streaming raw planner JSON into the REPL scrollback

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``stream=on` で `/plan-steps <goal>` / `/ultra-plan-run <goal>` を実行しても、生 JSON（`{"goal":` 等）が stdout/stderr に現れない（PTY テストで検証）。` を確認できる画面または実機操作を行う。
- 期待結果: `stream=on` で `/plan-steps <goal>` / `/ultra-plan-run <goal>` を実行しても、生 JSON（`{"goal":` 等）が stdout/stderr に現れない（PTY テストで検証）。
- Actual: With streaming enabled, PTY planner runs contain no raw planner JSON in stdout or stderr.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY planner stream and interrupt cases passed; cargo test provider_call::tests:: (14 passed); full integration suite passed.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `planner ターン中のスピナー・breadcrumb・footer 表示は従来どおり。` を確認できる画面または実機操作を行う。
- 期待結果: planner ターン中のスピナー・breadcrumb・footer 表示は従来どおり。
- Actual: The planner-stream PTY assertion retains spinner, breadcrumb, and footer progress.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY planner stream and interrupt cases passed; cargo test provider_call::tests:: (14 passed); full integration suite passed.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `executor のストリーミング表示は不変。` を確認できる画面または実機操作を行う。
- 期待結果: executor のストリーミング表示は不変。
- Actual: Provider-call tests confirm executor streaming behavior is unchanged.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY planner stream and interrupt cases passed; cargo test provider_call::tests:: (14 passed); full integration suite passed.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `planner ターンを Esc で中断した場合のストリーム後始末（spinner クリア等）にリグレッションが無い。` を確認できる画面または実機操作を行う。
- 期待結果: planner ターンを Esc で中断した場合のストリーム後始末（spinner クリア等）にリグレッションが無い。
- Actual: Interrupting a planner turn clears spinner/footer state without leaking raw stream content.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY planner stream and interrupt cases passed; cargo test provider_call::tests:: (14 passed); full integration suite passed.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `全イベント（名前・キー・値）が非破壊。` を確認できる画面または実機操作を行う。
- 期待結果: 全イベント（名前・キー・値）が非破壊。
- Actual: Provider-call and integration event assertions pass without changing event names, keys, or values.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY planner stream and interrupt cases passed; cargo test provider_call::tests:: (14 passed); full integration suite passed.
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``tests/tui_pty.rs` のストリーミングテストを新仕様（planner 生 JSON 不在＋spinner/footer cleanup の検証は維持）に更新する。` を確認できる画面または実機操作を行う。
- 期待結果: `tests/tui_pty.rs` のストリーミングテストを新仕様（planner 生 JSON 不在＋spinner/footer cleanup の検証は維持）に更新する。
- Actual: The revised PTY streaming cases assert both raw-JSON absence and spinner/footer cleanup.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: PTY planner stream and interrupt cases passed; cargo test provider_call::tests:: (14 passed); full integration suite passed.
- Result: passed

### Issue #49: [ux][i18n][bug] Use display-width truncation for user-visible text (CJK currently gets ~1/3 the budget)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `日本語 Goal が Plan card で従来比約3倍（列幅120相当＝約60文字）表示される。ASCII の表示長は不変。` を確認できる画面または実機操作を行う。
- 期待結果: 日本語 Goal が Plan card で従来比約3倍（列幅120相当＝約60文字）表示される。ASCII の表示長は不変。
- Actual: The plan card grants Japanese goals 120 display columns (about 60 wide characters) while preserving the ASCII budget.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); full cargo test passed (1,574 library tests plus integrations); source audit of shared fit_display_width call sites.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``display_width` / `char_display_width` / `display_width_ansi` を共有場所（例: `src/tui/terminal.rs` または `src/util.rs`）へ移し、footer と presentation が同一実装を使う。footer の既存挙動（ANSI エスケープを幅0として読み飛ばす等）は不変。` を確認できる画面または実機操作を行う。
- 期待結果: `display_width` / `char_display_width` / `display_width_ansi` を共有場所（例: `src/tui/terminal.rs` または `src/util.rs`）へ移し、footer と presentation が同一実装を使う。footer の既存挙動（ANSI エスケープを幅0として読み飛ばす等）は不変。
- Actual: Shared util display-width functions are used by footer and presentation, including ANSI-zero-width behavior.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); full cargo test passed (1,574 library tests plus integrations); source audit of shared fit_display_width call sites.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `表示用の新 API（例: `fit_display_width(value, cols, marker)`）を導入し、`presentation::fit` の全used箇所を置き換える。`excerpt_with_marker` は記録系用途に残す。` を確認できる画面または実機操作を行う。
- 期待結果: 表示用の新 API（例: `fit_display_width(value, cols, marker)`）を導入し、`presentation::fit` の全used箇所を置き換える。`excerpt_with_marker` は記録系用途に残す。
- Actual: presentation::fit uses fit_display_width, while record-oriented excerpt_with_marker remains available for stored values.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); full cargo test passed (1,574 library tests plus integrations); source audit of shared fit_display_width call sites.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``events.jsonl` に書かれる値（`body_snippet` 等の長さ・内容）が変わらない（golden / conformance テスト非破壊）。` を確認できる画面または実機操作を行う。
- 期待結果: `events.jsonl` に書かれる値（`body_snippet` 等の長さ・内容）が変わらない（golden / conformance テスト非破壊）。
- Actual: Event body-snippet and conformance assertions pass unchanged.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); full cargo test passed (1,574 library tests plus integrations); source audit of shared fit_display_width call sites.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `文字境界で panic しない（日本語・絵文字・結合文字・ANSI 込み文字列の unit テスト）。` を確認できる画面または実機操作を行う。
- 期待結果: 文字境界で panic しない（日本語・絵文字・結合文字・ANSI 込み文字列の unit テスト）。
- Actual: Unit tests cover Japanese, emoji, combining marks, ANSI input, and boundary-safe truncation without panics.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); full cargo test passed (1,574 library tests plus integrations); source audit of shared fit_display_width call sites.
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``input_queue::preview` / spinner label / `sanitize_command_excerpt` を監査し、表示系なら同 API へ寄せる（対象外と判断した場合は理由を PR に記載）。` を確認できる画面または実機操作を行う。
- 期待結果: `input_queue::preview` / spinner label / `sanitize_command_excerpt` を監査し、表示系なら同 API へ寄せる（対象外と判断した場合は理由を PR に記載）。
- Actual: Input-queue preview and command excerpts use the shared display-width API; spinner labels use sanitization because they are not width-budgeted.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); full cargo test passed (1,574 library tests plus integrations); source audit of shared fit_display_width call sites.
- Result: passed

### Issue #50: [ux] Presentation consistency: unified elapsed-time format, ASCII glyph fallback, footer emphasis

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `spinner の経過が 61 秒以上で `1m01s` 形式になり、footer と同一関数を使う。` を確認できる画面または実機操作を行う。
- 期待結果: spinner の経過が 61 秒以上で `1m01s` 形式になり、footer と同一関数を使う。
- Actual: Spinner elapsed time uses the shared formatter and renders 61 seconds as 1m01s, matching the footer.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); LC_ALL=C COMMANDAGENT_UX_DEMO_FAST=1 cargo run --quiet -- --cwd /tmp --ux-demo passed with ASCII-only output; full suite passed.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``LC_ALL=C`（非UTF-8）で REPL / `--ux-demo` を実行しても、breadcrumb・バナー・footer に多バイト記号が出力されない。UTF-8 ロケールでは従来どおり。` を確認できる画面または実機操作を行う。
- 期待結果: `LC_ALL=C`（非UTF-8）で REPL / `--ux-demo` を実行しても、breadcrumb・バナー・footer に多バイト記号が出力されない。UTF-8 ロケールでは従来どおり。
- Actual: The LC_ALL=C scripted demo completed with entirely ASCII presentation output; UTF-8 locale tests preserve Unicode glyphs.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); LC_ALL=C COMMANDAGENT_UX_DEMO_FAST=1 cargo run --quiet -- --cwd /tmp --ux-demo passed with ASCII-only output; full suite passed.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `footer の一次情報行が非 dim、設定行が dim になる（`build_live_footer_lines` / `build_footer_line` の unit テスト更新）。` を確認できる画面または実機操作を行う。
- 期待結果: footer の一次情報行が非 dim、設定行が dim になる（`build_live_footer_lines` / `build_footer_line` の unit テスト更新）。
- Actual: Footer tests show the primary status line non-dim and the settings line dim.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); LC_ALL=C COMMANDAGENT_UX_DEMO_FAST=1 cargo run --quiet -- --cwd /tmp --ux-demo passed with ASCII-only output; full suite passed.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``COMMANDAGENT_NO_SPINNER` / NO_COLOR 等の既存 env 挙動は不変。` を確認できる画面または実機操作を行う。
- 期待結果: `COMMANDAGENT_NO_SPINNER` / NO_COLOR 等の既存 env 挙動は不変。
- Actual: Spinner-disable and NO_COLOR/environment behavior remains covered and unchanged.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); LC_ALL=C COMMANDAGENT_UX_DEMO_FAST=1 cargo run --quiet -- --cwd /tmp --ux-demo passed with ASCII-only output; full suite passed.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``src/tui/ux_demo.rs` の scripted デモ（`scripted_demo_contains_full_visual_journey` テスト）と presentation 系スナップショットを更新し、`docs/assets/ux-demo.md` の手順で SVG/GIF 再生成が必要なら #43 の D 項（Demo 実録化）に委ねる旨を PR に明記。` を確認できる画面または実機操作を行う。
- 期待結果: `src/tui/ux_demo.rs` の scripted デモ（`scripted_demo_contains_full_visual_journey` テスト）と presentation 系スナップショットを更新し、`docs/assets/ux-demo.md` の手順で SVG/GIF 再生成が必要なら #43 の D 項（Demo 実録化）に委ねる旨を PR に明記。
- Actual: The scripted visual-journey tests and demo documentation pass; recording regeneration is explicitly delegated to Issue 43's real-recording work.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); LC_ALL=C COMMANDAGENT_UX_DEMO_FAST=1 cargo run --quiet -- --cwd /tmp --ux-demo passed with ASCII-only output; full suite passed.
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `既存イベントスキーマ非破壊（記号はあくまで表示層。events.jsonl の値は変えない）。` を確認できる画面または実機操作を行う。
- 期待結果: 既存イベントスキーマ非破壊（記号はあくまで表示層。events.jsonl の値は変えない）。
- Actual: Presentation-only glyph changes leave events.jsonl values unchanged, confirmed by the full conformance suite.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test tui:: (145 passed); LC_ALL=C COMMANDAGENT_UX_DEMO_FAST=1 cargo run --quiet -- --cwd /tmp --ux-demo passed with ASCII-only output; full suite passed.
- Result: passed

### Issue #51: [docs] Document REPL multi-line input continuation (/help + user guide)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/help` 出力に継続入力の説明が含まれる。` を確認できる画面または実機操作を行う。
- 期待結果: `/help` 出力に継続入力の説明が含まれる。
- Actual: Rendered /help includes multi-line continuation instructions.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test --test doc_drift (7 passed); cargo fmt --all -- --check passed; cargo clippy --all-targets -- -D warnings passed; cargo test passed.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``docs/guide/en` / `docs/guide/ja` の両方に同内容の節があり、EN/JA パリティが保たれている。` を確認できる画面または実機操作を行う。
- 期待結果: `docs/guide/en` / `docs/guide/ja` の両方に同内容の節があり、EN/JA パリティが保たれている。
- Actual: English and Japanese user guides contain matching continuation sections and retain heading/file parity.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test --test doc_drift (7 passed); cargo fmt --all -- --check passed; cargo clippy --all-targets -- -D warnings passed; cargo test passed.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``tests/doc_drift.rs` は `render_help` の内容を固定している（`doc_drift.rs:94,120,128` 付近）。ヘルプ文言変更に合わせて drift 側の期待値・対応ドキュメントを更新し、テストを通す。` を確認できる画面または実機操作を行う。
- 期待結果: `tests/doc_drift.rs` は `render_help` の内容を固定している（`doc_drift.rs:94,120,128` 付近）。ヘルプ文言変更に合わせて drift 側の期待値・対応ドキュメントを更新し、テストを通す。
- Actual: Documentation-drift assertions bind rendered help to both bilingual guide updates.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test --test doc_drift (7 passed); cargo fmt --all -- --check passed; cargo clippy --all-targets -- -D warnings passed; cargo test passed.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``cargo fmt --all -- --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test`` を確認できる画面または実機操作を行う。
- 期待結果: `cargo fmt --all -- --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test`
- Actual: cargo fmt --all -- --check, strict all-target clippy, and the complete cargo test suite all passed on the combined candidate.
- Evidence: Candidate add657dcc150517f837c4f84892dd870861d6b65: cargo test --test doc_drift (7 passed); cargo fmt --all -- --check passed; cargo clippy --all-targets -- -D warnings passed; cargo test passed.
- Result: passed

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
