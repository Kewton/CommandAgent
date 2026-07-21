# UAT Report

## Merge Gate

- Status: `pending`
- Message: missing UAT evidence for Issue #43 scenario 1

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #43: [ux][bug] Preserve accepted REPL goals and align live progress with the documented demo

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `REPLでEnter確定したコマンドを、処理開始前にスクロールバックへ再表示する。` を確認できる画面または実機操作を行う。
- 期待結果: REPLでEnter確定したコマンドを、処理開始前にスクロールバックへ再表示する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `少なくともcommand種別、Goal、明示profile/style/prompt-layoutを確認できる。` を確認できる画面または実機操作を行う。
- 期待結果: 少なくともcommand種別、Goal、明示profile/style/prompt-layoutを確認できる。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `日本語・CJK・長文・端末幅を超える折り返しでも消えない。` を確認できる画面または実機操作を行う。
- 期待結果: 日本語・CJK・長文・端末幅を超える折り返しでも消えない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `footer on/off、color/no-colorの両方で成立する。` を確認できる画面または実機操作を行う。
- 期待結果: footer on/off、color/no-colorの両方で成立する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `制御文字・bidi・端末escapeを既存のsanitize方針に従って無害化する。` を確認できる画面または実機操作を行う。
- 期待結果: 制御文字・bidi・端末escapeを既存のsanitize方針に従って無害化する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Command: /ultra-plan-run` を確認できる画面または実機操作を行う。
- 期待結果: Command: /ultra-plan-run
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Goal: あなたが考える最高に面白くかっこいいスペースインベーダーゲームを…` を確認できる画面または実機操作を行う。
- 期待結果: Goal: あなたが考える最高に面白くかっこいいスペースインベーダーゲームを…
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Profile: nextjs` を確認できる画面または実機操作を行う。
- 期待結果: Profile: nextjs
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Requested port: 3011` を確認できる画面または実機操作を行う。
- 期待結果: Requested port: 3011
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Run: 019f7fca-dc14-7241-bd51-36f0eba856ef` を確認できる画面または実機操作を行う。
- 期待結果: Run: 019f7fca-dc14-7241-bd51-36f0eba856ef
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `UltraPlan受付直後、総フェーズ数と最初のフェーズを表示する。` を確認できる画面または実機操作を行う。
- 期待結果: UltraPlan受付直後、総フェーズ数と最初のフェーズを表示する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `provider待機中も、現在のscope（planning / implementing / repairing）、phase `N/M`、step、経過時間を確認できる。` を確認できる画面または実機操作を行う。
- 期待結果: provider待機中も、現在のscope（planning / implementing / repairing）、phase `N/M`、step、経過時間を確認できる。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `固定フッターだけでなく、重要な遷移はスクロールバックbreadcrumbとして残す。` を確認できる画面または実機操作を行う。
- 期待結果: 固定フッターだけでなく、重要な遷移はスクロールバックbreadcrumbとして残す。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `空応答再試行・quality retry・interrupt requested等をユーザー向けに簡潔に表示する。` を確認できる画面または実機操作を行う。
- 期待結果: 空応答再試行・quality retry・interrupt requested等をユーザー向けに簡潔に表示する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/status` または同等の操作で、active Goal、Run ID、現在フェーズを再確認できる。` を確認できる画面または実機操作を行う。
- 期待結果: `/status` または同等の操作で、active Goal、Run ID、現在フェーズを再確認できる。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 16

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``--setup-interaction-probe` 成功時はsetup固有の短い結果を表示する。` を確認できる画面または実機操作を行う。
- 期待結果: `--setup-interaction-probe` 成功時はsetup固有の短い結果を表示する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 17

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `generic profileのassurance、`0/1 phases`、無関係なrelease/browser gate表を表示しない。` を確認できる画面または実機操作を行う。
- 期待結果: generic profileのassurance、`0/1 phases`、無関係なrelease/browser gate表を表示しない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 18

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `失敗時は失敗箇所とremediationを表示し、成功と失敗の終了コードを維持する。` を確認できる画面または実機操作を行う。
- 期待結果: 失敗時は失敗箇所とremediationを表示し、成功と失敗の終了コードを維持する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 19

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `他のdirect action (`--doctor`, `--runs`, completion/man generation等)も汎用coding task summaryの誤適用がないか監査する。` を確認できる画面または実機操作を行う。
- 期待結果: 他のdirect action (`--doctor`, `--runs`, completion/man generation等)も汎用coding task summaryの誤適用がないか監査する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 20

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `READMEで、`--ux-demo` がoffline scripted walkthroughであり、通常のprovider-backed runではないことをDemo直下に明記する。` を確認できる画面または実機操作を行う。
- 期待結果: READMEで、`--ux-demo` がoffline scripted walkthroughであり、通常のprovider-backed runではないことをDemo直下に明記する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 21

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `手作業のSVG抜粋と実際のターミナル録画を区別する。` を確認できる画面または実機操作を行う。
- 期待結果: 手作業のSVG抜粋と実際のターミナル録画を区別する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 22

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `最新バイナリを使った実際のREPL `/ultra-plan-run` の録画を追加するか、既存Demoを実録へ差し替える。` を確認できる画面または実機操作を行う。
- 期待結果: 最新バイナリを使った実際のREPL `/ultra-plan-run` の録画を追加するか、既存Demoを実録へ差し替える。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 23

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `録画には「指示受付」「Goal/profile/port」「phase/step進行」「長いprovider待機」「完了または回復」の実UXを含める。` を確認できる画面または実機操作を行う。
- 期待結果: 録画には「指示受付」「Goal/profile/port」「phase/step進行」「長いprovider待機」「完了または回復」の実UXを含める。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 24

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `README.md / README.ja.mdを同時更新し、doc-driftガードを通す。` を確認できる画面または実機操作を行う。
- 期待結果: README.md / README.ja.mdを同時更新し、doc-driftガードを通す。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 25

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Bの冒頭に追加: 「既存のpresentation / status_bus / footer機構の棚卸しを行い、再現環境で情報が不可視となる機序（ライター競合・scroll region・resize）をPTY screen-stateテストで特定する」。` を確認できる画面または実機操作を行う。
- 期待結果: Bの冒頭に追加: 「既存のpresentation / status_bus / footer機構の棚卸しを行い、再現環境で情報が不可視となる機序（ライター競合・scroll region・resize）をPTY screen-stateテストで特定する」。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 26

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `テスト要件: PTY回帰テストは `COMMANDAGENT_NO_SPINNER` / `COMMANDAGENT_NO_MARKDOWN` を**設定しない**（実UX合成）ケースを含める。` を確認できる画面または実機操作を行う。
- 期待結果: テスト要件: PTY回帰テストは `COMMANDAGENT_NO_SPINNER` / `COMMANDAGENT_NO_MARKDOWN` を**設定しない**（実UX合成）ケースを含める。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 27

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `C-4の監査結果を反映: 修正対象 = `--setup-interaction-probe` / `--model-probe` / `--plan-steps` / `--ultra-plan`、適用外（安全確認済み）= `--doctor` / `--runs` / `--ux-demo` / completions / man。` を確認できる画面または実機操作を行う。
- 期待結果: C-4の監査結果を反映: 修正対象 = `--setup-interaction-probe` / `--model-probe` / `--plan-steps` / `--ultra-plan`、適用外（安全確認済み）= `--doctor` / `--runs` / `--ux-demo` / completions / man。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 28

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `B-4は `documented_activity_ignore_reason` の分類見直し（`empty_response_recovered` 等の昇格）として実装する。` を確認できる画面または実機操作を行う。
- 期待結果: B-4は `documented_activity_ignore_reason` の分類見直し（`empty_response_recovered` 等の昇格）として実装する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 29

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `B-5は上記2-5の再定義を反映する。` を確認できる画面または実機操作を行う。
- 期待結果: B-5は上記2-5の再定義を反映する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

### Issue #44: [test][bug] PTY suite never runs via documented commands (#[ignore] + missing --include-ignored)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``just test-pty` が3テストを実際に実行する（出力が `0 passed` にならないこと、`3 passed` を確認）。` を確認できる画面または実機操作を行う。
- 期待結果: `just test-pty` が3テストを実際に実行する（出力が `0 passed` にならないこと、`3 passed` を確認）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``CONTRIBUTING.md` の該当コマンドが実際にテストを実行する形に更新されている。` を確認できる画面または実機操作を行う。
- 期待結果: `CONTRIBUTING.md` の該当コマンドが実際にテストを実行する形に更新されている。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``#[ignore]` の要否について方針をコード/ドキュメントのどちらかに一行で残す。` を確認できる画面または実機操作を行う。
- 期待結果: `#[ignore]` の要否について方針をコード/ドキュメントのどちらかに一行で残す。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `（任意）doc-drift guard（#22 の仕組み）でテスト起動コマンドの再乖離を検出できるなら追加する。` を確認できる画面または実機操作を行う。
- 期待結果: （任意）doc-drift guard（#22 の仕組み）でテスト起動コマンドの再乖離を検出できるなら追加する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `（任意）macOS/Linux runner での opt-in CI job を検討する。` を確認できる画面または実機操作を行う。
- 期待結果: （任意）macOS/Linux runner での opt-in CI job を検討する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

### Issue #45: [ux][bug] REPL failure output: render each failure once; stop framing typos and user interrupts as TASK FAILED

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/hepl` と入力: 候補提示を含む1〜3行の案内のみ。TASK FAILED ブロック・Terminal summary・`error:` の重複が一切出ない。` を確認できる画面または実機操作を行う。
- 期待結果: `/hepl` と入力: 候補提示を含む1〜3行の案内のみ。TASK FAILED ブロック・Terminal summary・`error:` の重複が一切出ない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `非slashの平文（日本語含む）を入力: 実行されず、/ultra-plan-run / /plan-run への誘導文のみ表示される。` を確認できる画面または実機操作を行う。
- 期待結果: 非slashの平文（日本語含む）を入力: 実行されず、/ultra-plan-run / /plan-run への誘導文のみ表示される。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `未知コマンド・自由文では `tui_command_start` / `tui_command_stop` イベントと summary 生成を行わない（コマンド実行前の入力エラー扱い）。既存イベントの名前・キー・スキーマは非破壊。` を確認できる画面または実機操作を行う。
- 期待結果: 未知コマンド・自由文では `tui_command_start` / `tui_command_stop` イベントと summary 生成を行わない（コマンド実行前の入力エラー扱い）。既存イベントの名前・キー・スキーマは非破壊。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `実行中に Esc/Ctrl-C: INTERRUPTED 表示が1回出て、再開手段が具体的に示される。「TASK FAILED」の文言が出ない。` を確認できる画面または実機操作を行う。
- 期待結果: 実行中に Esc/Ctrl-C: INTERRUPTED 表示が1回出て、再開手段が具体的に示される。「TASK FAILED」の文言が出ない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `実失敗（例: provider 接続不可）: 失敗表示が正確に1回、markdown renderer 経由で出る。` を確認できる画面または実機操作を行う。
- 期待結果: 実失敗（例: provider 接続不可）: 失敗表示が正確に1回、markdown renderer 経由で出る。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `入力エコー部は制御文字・bidi・端末escapeを既存sanitize方針で無害化する。` を確認できる画面または実機操作を行う。
- 期待結果: 入力エコー部は制御文字・bidi・端末escapeを既存sanitize方針で無害化する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `footer on/off、NO_COLOR の両方で成立する。` を確認できる画面または実機操作を行う。
- 期待結果: footer on/off、NO_COLOR の両方で成立する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/help` 文言を変更した場合、`tests/doc_drift.rs`（`render_help` を固定。94行・120行・128行参照）を更新して通す。` を確認できる画面または実機操作を行う。
- 期待結果: `/help` 文言を変更した場合、`tests/doc_drift.rs`（`render_help` を固定。94行・120行・128行参照）を更新して通す。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

### Issue #46: [ux] First-run onboarding: startup provider diagnostics, actionable error remediation, banner /help hint

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Ollama 停止状態で REPL 起動: host と是正手順を含む警告が出るが、プロンプトは表示され操作を継続できる。` を確認できる画面または実機操作を行う。
- 期待結果: Ollama 停止状態で REPL 起動: host と是正手順を含む警告が出るが、プロンプトは表示され操作を継続できる。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `起動遅延: サーバ到達時は体感ゼロ（tags 1回分）、不達時もタイムアウト上限（~2秒）以内。` を確認できる画面または実機操作を行う。
- 期待結果: 起動遅延: サーバ到達時は体感ゼロ（tags 1回分）、不達時もタイムアウト上限（~2秒）以内。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `モデル未取得で起動: `ollama pull <model>` を含む警告が出る。` を確認できる画面または実機操作を行う。
- 期待結果: モデル未取得で起動: `ollama pull <model>` を含む警告が出る。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `実行時の接続失敗・404・認証エラーそれぞれに是正ヒント行が付く（unit テストで文言を固定）。` を確認できる画面または実機操作を行う。
- 期待結果: 実行時の接続失敗・404・認証エラーそれぞれに是正ヒント行が付く（unit テストで文言を固定）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``--offline` 指定時・非TTY（`--prompt` 等）ではプローブを実行しない。` を確認できる画面または実機操作を行う。
- 期待結果: `--offline` 指定時・非TTY（`--prompt` 等）ではプローブを実行しない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `OPENAI/GEMINI キー未設定の起動失敗メッセージに設定手順と `--doctor` が含まれる。` を確認できる画面または実機操作を行う。
- 期待結果: OPENAI/GEMINI キー未設定の起動失敗メッセージに設定手順と `--doctor` が含まれる。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `バナーに `/help` / `/doctor` 導線行が追加される（`--ux-demo` のバナー描画・関連スナップショットも更新）。` を確認できる画面または実機操作を行う。
- 期待結果: バナーに `/help` / `/doctor` 導線行が追加される（`--ux-demo` のバナー描画・関連スナップショットも更新）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `既存イベントスキーマは非破壊（警告をイベント化する場合は additive な新イベントとする）。` を確認できる画面または実機操作を行う。
- 期待結果: 既存イベントスキーマは非破壊（警告をイベント化する場合は additive な新イベントとする）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

### Issue #47: [ux] Long-run awareness: terminal title progress and completion bell (OSC 2 / BEL)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `ultra 実行中の phase 遷移で `\x1b]2;<text>\x07` がタイトル用に出力される（PTY テストでバイト列を検証）。` を確認できる画面または実機操作を行う。
- 期待結果: ultra 実行中の phase 遷移で `\x1b]2;<text>\x07` がタイトル用に出力される（PTY テストでバイト列を検証）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `プロセス終了時にタイトルがクリアされる（空タイトルの OSC 2 出力）。` を確認できる画面または実機操作を行う。
- 期待結果: プロセス終了時にタイトルがクリアされる（空タイトルの OSC 2 出力）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `10秒以上かかったコマンドの完了時に BEL が1回出力され、短時間コマンドでは出力されない（時間は注入可能にして unit テスト）。` を確認できる画面または実機操作を行う。
- 期待結果: 10秒以上かかったコマンドの完了時に BEL が1回出力され、短時間コマンドでは出力されない（時間は注入可能にして unit テスト）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``COMMANDAGENT_NO_TERMINAL_TITLE=1` / `COMMANDAGENT_NO_BELL=1`（および `ANVIL_` prefix）でそれぞれ抑止される。` を確認できる画面または実機操作を行う。
- 期待結果: `COMMANDAGENT_NO_TERMINAL_TITLE=1` / `COMMANDAGENT_NO_BELL=1`（および `ANVIL_` prefix）でそれぞれ抑止される。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `非TTLで一切出力されない。`--footer off` でもタイトル/ベルは機能する（下記の注意参照）。` を確認できる画面または実機操作を行う。
- 期待結果: 非TTLで一切出力されない。`--footer off` でもタイトル/ベルは機能する（下記の注意参照）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `タイトル文字列は既存 sanitize 方針（制御文字・bidi 無害化）に従い、長さを常識的な上限（例: 120 bytes）で切る。` を確認できる画面または実機操作を行う。
- 期待結果: タイトル文字列は既存 sanitize 方針（制御文字・bidi 無害化）に従い、長さを常識的な上限（例: 120 bytes）で切る。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `既存イベントスキーマ・footer/spinner の挙動は非破壊。` を確認できる画面または実機操作を行う。
- 期待結果: 既存イベントスキーマ・footer/spinner の挙動は非破壊。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

### Issue #48: [ux][bug] Stop streaming raw planner JSON into the REPL scrollback

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``stream=on` で `/plan-steps <goal>` / `/ultra-plan-run <goal>` を実行しても、生 JSON（`{"goal":` 等）が stdout/stderr に現れない（PTY テストで検証）。` を確認できる画面または実機操作を行う。
- 期待結果: `stream=on` で `/plan-steps <goal>` / `/ultra-plan-run <goal>` を実行しても、生 JSON（`{"goal":` 等）が stdout/stderr に現れない（PTY テストで検証）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `planner ターン中のスピナー・breadcrumb・footer 表示は従来どおり。` を確認できる画面または実機操作を行う。
- 期待結果: planner ターン中のスピナー・breadcrumb・footer 表示は従来どおり。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `executor のストリーミング表示は不変。` を確認できる画面または実機操作を行う。
- 期待結果: executor のストリーミング表示は不変。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `planner ターンを Esc で中断した場合のストリーム後始末（spinner クリア等）にリグレッションが無い。` を確認できる画面または実機操作を行う。
- 期待結果: planner ターンを Esc で中断した場合のストリーム後始末（spinner クリア等）にリグレッションが無い。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `全イベント（名前・キー・値）が非破壊。` を確認できる画面または実機操作を行う。
- 期待結果: 全イベント（名前・キー・値）が非破壊。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``tests/tui_pty.rs` のストリーミングテストを新仕様（planner 生 JSON 不在＋spinner/footer cleanup の検証は維持）に更新する。` を確認できる画面または実機操作を行う。
- 期待結果: `tests/tui_pty.rs` のストリーミングテストを新仕様（planner 生 JSON 不在＋spinner/footer cleanup の検証は維持）に更新する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

### Issue #49: [ux][i18n][bug] Use display-width truncation for user-visible text (CJK currently gets ~1/3 the budget)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `日本語 Goal が Plan card で従来比約3倍（列幅120相当＝約60文字）表示される。ASCII の表示長は不変。` を確認できる画面または実機操作を行う。
- 期待結果: 日本語 Goal が Plan card で従来比約3倍（列幅120相当＝約60文字）表示される。ASCII の表示長は不変。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``display_width` / `char_display_width` / `display_width_ansi` を共有場所（例: `src/tui/terminal.rs` または `src/util.rs`）へ移し、footer と presentation が同一実装を使う。footer の既存挙動（ANSI エスケープを幅0として読み飛ばす等）は不変。` を確認できる画面または実機操作を行う。
- 期待結果: `display_width` / `char_display_width` / `display_width_ansi` を共有場所（例: `src/tui/terminal.rs` または `src/util.rs`）へ移し、footer と presentation が同一実装を使う。footer の既存挙動（ANSI エスケープを幅0として読み飛ばす等）は不変。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `表示用の新 API（例: `fit_display_width(value, cols, marker)`）を導入し、`presentation::fit` の全used箇所を置き換える。`excerpt_with_marker` は記録系用途に残す。` を確認できる画面または実機操作を行う。
- 期待結果: 表示用の新 API（例: `fit_display_width(value, cols, marker)`）を導入し、`presentation::fit` の全used箇所を置き換える。`excerpt_with_marker` は記録系用途に残す。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``events.jsonl` に書かれる値（`body_snippet` 等の長さ・内容）が変わらない（golden / conformance テスト非破壊）。` を確認できる画面または実機操作を行う。
- 期待結果: `events.jsonl` に書かれる値（`body_snippet` 等の長さ・内容）が変わらない（golden / conformance テスト非破壊）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `文字境界で panic しない（日本語・絵文字・結合文字・ANSI 込み文字列の unit テスト）。` を確認できる画面または実機操作を行う。
- 期待結果: 文字境界で panic しない（日本語・絵文字・結合文字・ANSI 込み文字列の unit テスト）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``input_queue::preview` / spinner label / `sanitize_command_excerpt` を監査し、表示系なら同 API へ寄せる（対象外と判断した場合は理由を PR に記載）。` を確認できる画面または実機操作を行う。
- 期待結果: `input_queue::preview` / spinner label / `sanitize_command_excerpt` を監査し、表示系なら同 API へ寄せる（対象外と判断した場合は理由を PR に記載）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

### Issue #50: [ux] Presentation consistency: unified elapsed-time format, ASCII glyph fallback, footer emphasis

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `spinner の経過が 61 秒以上で `1m01s` 形式になり、footer と同一関数を使う。` を確認できる画面または実機操作を行う。
- 期待結果: spinner の経過が 61 秒以上で `1m01s` 形式になり、footer と同一関数を使う。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``LC_ALL=C`（非UTF-8）で REPL / `--ux-demo` を実行しても、breadcrumb・バナー・footer に多バイト記号が出力されない。UTF-8 ロケールでは従来どおり。` を確認できる画面または実機操作を行う。
- 期待結果: `LC_ALL=C`（非UTF-8）で REPL / `--ux-demo` を実行しても、breadcrumb・バナー・footer に多バイト記号が出力されない。UTF-8 ロケールでは従来どおり。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `footer の一次情報行が非 dim、設定行が dim になる（`build_live_footer_lines` / `build_footer_line` の unit テスト更新）。` を確認できる画面または実機操作を行う。
- 期待結果: footer の一次情報行が非 dim、設定行が dim になる（`build_live_footer_lines` / `build_footer_line` の unit テスト更新）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``COMMANDAGENT_NO_SPINNER` / NO_COLOR 等の既存 env 挙動は不変。` を確認できる画面または実機操作を行う。
- 期待結果: `COMMANDAGENT_NO_SPINNER` / NO_COLOR 等の既存 env 挙動は不変。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``src/tui/ux_demo.rs` の scripted デモ（`scripted_demo_contains_full_visual_journey` テスト）と presentation 系スナップショットを更新し、`docs/assets/ux-demo.md` の手順で SVG/GIF 再生成が必要なら #43 の D 項（Demo 実録化）に委ねる旨を PR に明記。` を確認できる画面または実機操作を行う。
- 期待結果: `src/tui/ux_demo.rs` の scripted デモ（`scripted_demo_contains_full_visual_journey` テスト）と presentation 系スナップショットを更新し、`docs/assets/ux-demo.md` の手順で SVG/GIF 再生成が必要なら #43 の D 項（Demo 実録化）に委ねる旨を PR に明記。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `既存イベントスキーマ非破壊（記号はあくまで表示層。events.jsonl の値は変えない）。` を確認できる画面または実機操作を行う。
- 期待結果: 既存イベントスキーマ非破壊（記号はあくまで表示層。events.jsonl の値は変えない）。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

### Issue #51: [docs] Document REPL multi-line input continuation (/help + user guide)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/help` 出力に継続入力の説明が含まれる。` を確認できる画面または実機操作を行う。
- 期待結果: `/help` 出力に継続入力の説明が含まれる。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``docs/guide/en` / `docs/guide/ja` の両方に同内容の節があり、EN/JA パリティが保たれている。` を確認できる画面または実機操作を行う。
- 期待結果: `docs/guide/en` / `docs/guide/ja` の両方に同内容の節があり、EN/JA パリティが保たれている。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``tests/doc_drift.rs` は `render_help` の内容を固定している（`doc_drift.rs:94,120,128` 付近）。ヘルプ文言変更に合わせて drift 側の期待値・対応ドキュメントを更新し、テストを通す。` を確認できる画面または実機操作を行う。
- 期待結果: `tests/doc_drift.rs` は `render_help` の内容を固定している（`doc_drift.rs:94,120,128` 付近）。ヘルプ文言変更に合わせて drift 側の期待値・対応ドキュメントを更新し、テストを通す。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``cargo fmt --all -- --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test`` を確認できる画面または実機操作を行う。
- 期待結果: `cargo fmt --all -- --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test`
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
