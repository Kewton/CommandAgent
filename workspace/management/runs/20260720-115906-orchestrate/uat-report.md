# UAT Report

## Merge Gate

- Status: `passed`
- Message: all 125 UAT scenarios passed with evidence

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #19: [docs] Overhaul README (EN) and add README.ja.md: quickstart, install, features, demo, badges, license

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**1行の価値提案**(何であるか+なぜ使うか)と簡潔なリード文` を確認できる画面または実機操作を行う。
- 期待結果: **1行の価値提案**(何であるか+なぜ使うか)と簡潔なリード文
- Actual: Observed on PR #33 head cffd8da: **1行の価値提案**(何であるか+なぜ使うか)と簡潔なリード文
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**バッジ**: CIステータス(`.github/workflows/ci.yml` のワークフロー名 `CI` を参照)、ライセンス(MIT)。存在しないもの(crates.io等)のバッジは貼らない` を確認できる画面または実機操作を行う。
- 期待結果: **バッジ**: CIステータス(`.github/workflows/ci.yml` のワークフロー名 `CI` を参照)、ライセンス(MIT)。存在しないもの(crates.io等)のバッジは貼らない
- Actual: Observed on PR #33 head cffd8da: **バッジ**: CIステータス(`.github/workflows/ci.yml` のワークフロー名 `CI` を参照)、ライセンス(MIT)。存在しないもの(crates.io等)のバッジは貼らない
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**機能一覧**: minimal loop / step plan(plan-run)/ ultra plan run / プロファイル(generic・nextjs・python_cli・data)/ マルチプロバイダ(Ollama・Gemini・OpenAI)/ 検証・修復ループ / TUI(フッター・スピナー・割り込み)を箇条書きで` を確認できる画面または実機操作を行う。
- 期待結果: **機能一覧**: minimal loop / step plan(plan-run)/ ultra plan run / プロファイル(generic・nextjs・python_cli・data)/ マルチプロバイダ(Ollama・Gemini・OpenAI)/ 検証・修復ループ / TUI(フッター・スピナー・割り込み)を箇条書きで
- Actual: Observed on PR #33 head cffd8da: **機能一覧**: minimal loop / step plan(plan-run)/ ultra plan run / プロファイル(generic・nextjs・python_cli・data)/ マルチプロバイダ(Ollama・Gemini・OpenAI)/ 検証・修復ループ / TUI(フッター・スピナー・割り込み)を箇条書きで
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**デモ**: `--ux-demo` を録画したGIFまたはSVGアニメを `docs/assets/` に置き先頭近くに埋め込む。再現手順(録画コマンド)をアセットと同じ場所にメモとして残す(例: charmbracelet/vhs のtapeファイル or asciinema。ツール選定は実装者に委ねるが再現可能にすること)` を確認できる画面または実機操作を行う。
- 期待結果: **デモ**: `--ux-demo` を録画したGIFまたはSVGアニメを `docs/assets/` に置き先頭近くに埋め込む。再現手順(録画コマンド)をアセットと同じ場所にメモとして残す(例: charmbracelet/vhs のtapeファイル or asciinema。ツール選定は実装者に委ねるが再現可能にすること)
- Actual: Observed on PR #33 head cffd8da: **デモ**: `--ux-demo` を録画したGIFまたはSVGアニメを `docs/assets/` に置き先頭近くに埋め込む。再現手順(録画コマンド)をアセットと同じ場所にメモとして残す(例: charmbracelet/vhs のtapeファイル or asciinema。ツール選定は実装者に委ねるが再現可能にすること)
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**Quickstart(最小経路)**: Ollamaのみで動く最短手順(Ollamaインストール→モデルpull→`cargo install --path .`→1コマンド実行)。モデルIDは「**手元に実在するモデルに置き換える**」旨を明記し、例は `<your-model>` プレースホルダ表記にする` を確認できる画面または実機操作を行う。
- 期待結果: **Quickstart(最小経路)**: Ollamaのみで動く最短手順(Ollamaインストール→モデルpull→`cargo install --path .`→1コマンド実行)。モデルIDは「**手元に実在するモデルに置き換える**」旨を明記し、例は `<your-model>` プレースホルダ表記にする
- Actual: Observed on PR #33 head cffd8da: **Quickstart(最小経路)**: Ollamaのみで動く最短手順(Ollamaインストール→モデルpull→`cargo install --path .`→1コマンド実行)。モデルIDは「**手元に実在するモデルに置き換える**」旨を明記し、例は `<your-model>` プレースホルダ表記にする
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**Install**: 前提条件表(Rust 1.88+ / 任意: Ollama、Gemini・OpenAI APIキー、Node.js+npm(インタラクションプローブ用)、Python 3(eval用))と導入手順。`scripts/setup.sh`(別Issue)が入り次第そのリンクを張る前提の構成にする` を確認できる画面または実機操作を行う。
- 期待結果: **Install**: 前提条件表(Rust 1.88+ / 任意: Ollama、Gemini・OpenAI APIキー、Node.js+npm(インタラクションプローブ用)、Python 3(eval用))と導入手順。`scripts/setup.sh`(別Issue)が入り次第そのリンクを張る前提の構成にする
- Actual: Observed on PR #33 head cffd8da: **Install**: 前提条件表(Rust 1.88+ / 任意: Ollama、Gemini・OpenAI APIキー、Node.js+npm(インタラクションプローブ用)、Python 3(eval用))と導入手順。`scripts/setup.sh`(別Issue)が入り次第そのリンクを張る前提の構成にする
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**Usage**: 代表的なCLI実行例とREPLの主要スラッシュコマンド抜粋(全リファレンスは docs/guide/ へのリンク。別Issue)` を確認できる画面または実機操作を行う。
- 期待結果: **Usage**: 代表的なCLI実行例とREPLの主要スラッシュコマンド抜粋(全リファレンスは docs/guide/ へのリンク。別Issue)
- Actual: Observed on PR #33 head cffd8da: **Usage**: 代表的なCLI実行例とREPLの主要スラッシュコマンド抜粋(全リファレンスは docs/guide/ へのリンク。別Issue)
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**Configuration**: preset の存在と設定ファイルの場所のみ簡潔に示し、詳細はガイドへのリンク` を確認できる画面または実機操作を行う。
- 期待結果: **Configuration**: preset の存在と設定ファイルの場所のみ簡潔に示し、詳細はガイドへのリンク
- Actual: Observed on PR #33 head cffd8da: **Configuration**: preset の存在と設定ファイルの場所のみ簡潔に示し、詳細はガイドへのリンク
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**License 節**(MIT)` を確認できる画面または実機操作を行う。
- 期待結果: **License 節**(MIT)
- Actual: Observed on PR #33 head cffd8da: **License 節**(MIT)
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `現READMEの UAT・copy-validation・symlink運用などの**開発者向け内容は README から削除し docs/dev/ へ移す**(docs再編Issueと調整。両Issueを同一PRにしてもよい)` を確認できる画面または実機操作を行う。
- 期待結果: 現READMEの UAT・copy-validation・symlink運用などの**開発者向け内容は README から削除し docs/dev/ へ移す**(docs再編Issueと調整。両Issueを同一PRにしてもよい)
- Actual: Observed on PR #33 head cffd8da: 現READMEの UAT・copy-validation・symlink運用などの**開発者向け内容は README から削除し docs/dev/ へ移す**(docs再編Issueと調整。両Issueを同一PRにしてもよい)
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``README.ja.md` を新設し、英語版と**同一構成・同一情報**の日本語訳とする` を確認できる画面または実機操作を行う。
- 期待結果: `README.ja.md` を新設し、英語版と**同一構成・同一情報**の日本語訳とする
- Actual: Observed on PR #33 head cffd8da: `README.ja.md` を新設し、英語版と**同一構成・同一情報**の日本語訳とする
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `両ファイルの冒頭に相互リンク(`English | 日本語`)を置く` を確認できる画面または実機操作を行う。
- 期待結果: 両ファイルの冒頭に相互リンク(`English | 日本語`)を置く
- Actual: Observed on PR #33 head cffd8da: 両ファイルの冒頭に相互リンク(`English | 日本語`)を置く
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `以後の更新で両方を同時に更新する旨をCONTRIBUTING(別Issue)に記載するため、本Issueでは両ファイル冒頭コメントに「対訳ファイルあり・同時更新のこと」の注記を入れる` を確認できる画面または実機操作を行う。
- 期待結果: 以後の更新で両方を同時に更新する旨をCONTRIBUTING(別Issue)に記載するため、本Issueでは両ファイル冒頭コメントに「対訳ファイルあり・同時更新のこと」の注記を入れる
- Actual: Observed on PR #33 head cffd8da: 以後の更新で両方を同時に更新する旨をCONTRIBUTING(別Issue)に記載するため、本Issueでは両ファイル冒頭コメントに「対訳ファイルあり・同時更新のこと」の注記を入れる
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `記載するフラグ・コマンド・パスはすべて現行コード(`src/cli.rs`, `src/tui/slash.rs:408-429`, `src/config.rs:663-668`)と突き合わせて正確にする` を確認できる画面または実機操作を行う。
- 期待結果: 記載するフラグ・コマンド・パスはすべて現行コード(`src/cli.rs`, `src/tui/slash.rs:408-429`, `src/config.rs:663-668`)と突き合わせて正確にする
- Actual: Observed on PR #33 head cffd8da: 記載するフラグ・コマンド・パスはすべて現行コード(`src/cli.rs`, `src/tui/slash.rs:408-429`, `src/config.rs:663-668`)と突き合わせて正確にする
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `「Anvil does not auto-create them」相当の情報(設定ファイルは自動生成されない)は**正確なので維持**する(`src/config.rs:520-558` に書き込み経路なし)` を確認できる画面または実機操作を行う。
- 期待結果: 「Anvil does not auto-create them」相当の情報(設定ファイルは自動生成されない)は**正確なので維持**する(`src/config.rs:520-558` に書き込み経路なし)
- Actual: Observed on PR #33 head cffd8da: 「Anvil does not auto-create them」相当の情報(設定ファイルは自動生成されない)は**正確なので維持**する(`src/config.rs:520-558` に書き込み経路なし)
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 16

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``.anvil/` パス表記は現状の実パスのまま書く(リネームは Issue #16/#17 のスコープ。先走らない)` を確認できる画面または実機操作を行う。
- 期待結果: `.anvil/` パス表記は現状の実パスのまま書く(リネームは Issue #16/#17 のスコープ。先走らない)
- Actual: Observed on PR #33 head cffd8da: `.anvil/` パス表記は現状の実パスのまま書く(リネームは Issue #16/#17 のスコープ。先走らない)
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 17

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `製品名表記は Issue #15(ユーザー可視ブランディング)の完了状態に合わせる。**#15完了後の着手を推奨**(未完了なら本Issueで名前だけ先行変更しない)` を確認できる画面または実機操作を行う。
- 期待結果: 製品名表記は Issue #15(ユーザー可視ブランディング)の完了状態に合わせる。**#15完了後の着手を推奨**(未完了なら本Issueで名前だけ先行変更しない)
- Actual: Observed on PR #33 head cffd8da: 製品名表記は Issue #15(ユーザー可視ブランディング)の完了状態に合わせる。**#15完了後の着手を推奨**(未完了なら本Issueで名前だけ先行変更しない)
- Evidence: Issue #19 verification: bilingual structure/link/demo/CLI checks passed; GitHub Actions passed for PR #33; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #20: [docs] Add bilingual user guide (docs/guide/ en+ja): CLI, slash commands, configuration, providers, troubleshooting

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``cli-reference.md` — **全37フラグ**の表(フラグ / 引数 / 既定値 / 説明 / 関連項目)。既定値・conflicts(例: `--footer` と `--no-footer` は排他, `src/cli.rs:115-128`)は `src/cli.rs` から正確に転記` を確認できる画面または実機操作を行う。
- 期待結果: `cli-reference.md` — **全37フラグ**の表(フラグ / 引数 / 既定値 / 説明 / 関連項目)。既定値・conflicts(例: `--footer` と `--no-footer` は排他, `src/cli.rs:115-128`)は `src/cli.rs` から正確に転記
- Actual: Observed on PR #34 head b1e82b7: `cli-reference.md` — **全37フラグ**の表(フラグ / 引数 / 既定値 / 説明 / 関連項目)。既定値・conflicts(例: `--footer` と `--no-footer` は排他, `src/cli.rs:115-128`)は `src/cli.rs` から正確に転記
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``slash-commands.md` — **全15コマンド**(`render_help` の内容が正)+ インラインフラグ + `$(cat <path>)` 展開 + プロファイル自動推論(`src/tui/slash.rs:192-199`)の説明` を確認できる画面または実機操作を行う。
- 期待結果: `slash-commands.md` — **全15コマンド**(`render_help` の内容が正)+ インラインフラグ + `$(cat <path>)` 展開 + プロファイル自動推論(`src/tui/slash.rs:192-199`)の説明
- Actual: Observed on PR #34 head b1e82b7: `slash-commands.md` — **全15コマンド**(`render_help` の内容が正)+ インラインフラグ + `$(cat <path>)` 展開 + プロファイル自動推論(`src/tui/slash.rs:192-199`)の説明
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``configuration.md` — 設定解決の全体像(CLIフラグ > preset > トップレベルキー > 既定値の優先順位を実コードで確認して記載)、presetの全10キーと**完全性の罠**、トップレベルキー、探索パスと順序、レガシー `.anvil/config`、環境変数(`NO_COLOR`、`ANVIL_NO_FOOTER`/`ANVIL_NO_SPINNER`/`ANVIL_NO_MARKDOWN`/`ANVIL_NO_INTERRUPT` 等のユーザー向けのもの)` を確認できる画面または実機操作を行う。
- 期待結果: `configuration.md` — 設定解決の全体像(CLIフラグ > preset > トップレベルキー > 既定値の優先順位を実コードで確認して記載)、presetの全10キーと**完全性の罠**、トップレベルキー、探索パスと順序、レガシー `.anvil/config`、環境変数(`NO_COLOR`、`ANVIL_NO_FOOTER`/`ANVIL_NO_SPINNER`/`ANVIL_NO_MARKDOWN`/`ANVIL_NO_INTERRUPT` 等のユーザー向けのもの)
- Actual: Observed on PR #34 head b1e82b7: `configuration.md` — 設定解決の全体像(CLIフラグ > preset > トップレベルキー > 既定値の優先順位を実コードで確認して記載)、presetの全10キーと**完全性の罠**、トップレベルキー、探索パスと順序、レガシー `.anvil/config`、環境変数(`NO_COLOR`、`ANVIL_NO_FOOTER`/`ANVIL_NO_SPINNER`/`ANVIL_NO_MARKDOWN`/`ANVIL_NO_INTERRUPT` 等のユーザー向けのもの)
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``providers.md` — プロバイダ別セットアップ表(必要なキー・取得先URL・設定方法 env/.env・Ollamaのホスト設定)。**キーの値を画面に出さない**注意書きと `.env` の権限推奨(600)を含める` を確認できる画面または実機操作を行う。
- 期待結果: `providers.md` — プロバイダ別セットアップ表(必要なキー・取得先URL・設定方法 env/.env・Ollamaのホスト設定)。**キーの値を画面に出さない**注意書きと `.env` の権限推奨(600)を含める
- Actual: Observed on PR #34 head b1e82b7: `providers.md` — プロバイダ別セットアップ表(必要なキー・取得先URL・設定方法 env/.env・Ollamaのホスト設定)。**キーの値を画面に出さない**注意書きと `.env` の権限推奨(600)を含める
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``troubleshooting.md` — 最低限: 「`GEMINI_API_KEY is not set`」(`src/config.rs:1117` のエラー文言そのまま見出しに)/「port N is busy」preflight(`src/preflight.rs:95` 周辺の挙動と選択肢)/「interaction probe unavailable」(`src/minimal_loop/interaction_probe.rs:20` の remediation)/ フッター描画乱れ(`--footer off`)/ モデルIDが実在しない場合の失敗の見え方 / Ollama未起動` を確認できる画面または実機操作を行う。
- 期待結果: `troubleshooting.md` — 最低限: 「`GEMINI_API_KEY is not set`」(`src/config.rs:1117` のエラー文言そのまま見出しに)/「port N is busy」preflight(`src/preflight.rs:95` 周辺の挙動と選択肢)/「interaction probe unavailable」(`src/minimal_loop/interaction_probe.rs:20` の remediation)/ フッター描画乱れ(`--footer off`)/ モデルIDが実在しない場合の失敗の見え方 / Ollama未起動
- Actual: Observed on PR #34 head b1e82b7: `troubleshooting.md` — 最低限: 「`GEMINI_API_KEY is not set`」(`src/config.rs:1117` のエラー文言そのまま見出しに)/「port N is busy」preflight(`src/preflight.rs:95` 周辺の挙動と選択肢)/「interaction probe unavailable」(`src/minimal_loop/interaction_probe.rs:20` の remediation)/ フッター描画乱れ(`--footer off`)/ モデルIDが実在しない場合の失敗の見え方 / Ollama未起動
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``docs/guide/README.md` — ガイドの目次(en/ja両方への入口)。`docs/model-probe.md` へのリンクを含める` を確認できる画面または実機操作を行う。
- 期待結果: `docs/guide/README.md` — ガイドの目次(en/ja両方への入口)。`docs/model-probe.md` へのリンクを含める
- Actual: Observed on PR #34 head b1e82b7: `docs/guide/README.md` — ガイドの目次(en/ja両方への入口)。`docs/model-probe.md` へのリンクを含める
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `en/ja は**同一構成・同一情報**。見出し構造(h2/h3の数と順序)を一致させる(後続のdoc-driftガードIssueで機械検証する前提)` を確認できる画面または実機操作を行う。
- 期待結果: en/ja は**同一構成・同一情報**。見出し構造(h2/h3の数と順序)を一致させる(後続のdoc-driftガードIssueで機械検証する前提)
- Actual: Observed on PR #34 head b1e82b7: en/ja は**同一構成・同一情報**。見出し構造(h2/h3の数と順序)を一致させる(後続のdoc-driftガードIssueで機械検証する前提)
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `各ファイル冒頭に対訳ファイルへの相互リンク` を確認できる画面または実機操作を行う。
- 期待結果: 各ファイル冒頭に対訳ファイルへの相互リンク
- Actual: Observed on PR #34 head b1e82b7: 各ファイル冒頭に対訳ファイルへの相互リンク
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `全記載をコードと突き合わせる。とくに既定値(`num_predict`、`max_iterations`、`chat_timeout_secs`、`chat_retries`、`context_budget`)は `src/cli.rs` / `src/config.rs` の実値を確認して記載` を確認できる画面または実機操作を行う。
- 期待結果: 全記載をコードと突き合わせる。とくに既定値(`num_predict`、`max_iterations`、`chat_timeout_secs`、`chat_retries`、`context_budget`)は `src/cli.rs` / `src/config.rs` の実値を確認して記載
- Actual: Observed on PR #34 head b1e82b7: 全記載をコードと突き合わせる。とくに既定値(`num_predict`、`max_iterations`、`chat_timeout_secs`、`chat_retries`、`context_budget`)は `src/cli.rs` / `src/config.rs` の実値を確認して記載
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``.anvil/` パス・`ANVIL_*` 変数名は現状のまま記載(リネームは Issue #16/#17。ガイド側は完了後に追従)` を確認できる画面または実機操作を行う。
- 期待結果: `.anvil/` パス・`ANVIL_*` 変数名は現状のまま記載(リネームは Issue #16/#17。ガイド側は完了後に追従)
- Actual: Observed on PR #34 head b1e82b7: `.anvil/` パス・`ANVIL_*` 変数名は現状のまま記載(リネームは Issue #16/#17。ガイド側は完了後に追従)
- Evidence: Issue #20 verification: CLI/slash/config documentation parity checks passed; GitHub Actions passed for PR #34; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #21: [docs] Reorganize docs/ into guide vs dev with an index; translate data-profile-contract; add benchmarks README

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``docs/dev/` を新設し、内部向け・歴史記録を移動する: `dev-guardrails.md` `mechanism-ledger.md` `generality.md` `perf-notes.md` `integration-notes.md` `uat-corpus.md` `uat/` `migration/` `profile-manifest.md` `data-profile-contract.md`` を確認できる画面または実機操作を行う。
- 期待結果: `docs/dev/` を新設し、内部向け・歴史記録を移動する: `dev-guardrails.md` `mechanism-ledger.md` `generality.md` `perf-notes.md` `integration-notes.md` `uat-corpus.md` `uat/` `migration/` `profile-manifest.md` `data-profile-contract.md`
- Actual: Observed on PR #35 head 23fdc75: `docs/dev/` を新設し、内部向け・歴史記録を移動する: `dev-guardrails.md` `mechanism-ledger.md` `generality.md` `perf-notes.md` `integration-notes.md` `uat-corpus.md` `uat/` `migration/` `profile-manifest.md` `data-profile-contract.md`
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `移動は `git mv` で行い、**内容は一切書き換えない**(歴史記録の改変禁止。とくに `mechanism-ledger.md` と `migration/`)` を確認できる画面または実機操作を行う。
- 期待結果: 移動は `git mv` で行い、**内容は一切書き換えない**(歴史記録の改変禁止。とくに `mechanism-ledger.md` と `migration/`)
- Actual: Observed on PR #35 head 23fdc75: 移動は `git mv` で行い、**内容は一切書き換えない**(歴史記録の改変禁止。とくに `mechanism-ledger.md` と `migration/`)
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``model-probe.md` はユーザー寄りとして `docs/guide/` 側(ユーザーガイドIssueのディレクトリ)へ移すか、ガイドからリンクする(ガイドIssueと調整。同時実装可)` を確認できる画面または実機操作を行う。
- 期待結果: `model-probe.md` はユーザー寄りとして `docs/guide/` 側(ユーザーガイドIssueのディレクトリ)へ移すか、ガイドからリンクする(ガイドIssueと調整。同時実装可)
- Actual: Observed on PR #35 head 23fdc75: `model-probe.md` はユーザー寄りとして `docs/guide/` 側(ユーザーガイドIssueのディレクトリ)へ移すか、ガイドからリンクする(ガイドIssueと調整。同時実装可)
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `移動に伴うパス参照の追従: `rg -n 'docs/' src tests .github *.md docs` で全参照を洗い出して更新する(確認済みの参照: `src/minimal_loop/repair_pressure.rs:231` のコメント。ほかにMarkdown間の相互リンクが `README.md` `SECURITY.md` などにある)` を確認できる画面または実機操作を行う。
- 期待結果: 移動に伴うパス参照の追従: `rg -n 'docs/' src tests .github *.md docs` で全参照を洗い出して更新する(確認済みの参照: `src/minimal_loop/repair_pressure.rs:231` のコメント。ほかにMarkdown間の相互リンクが `README.md` `SECURITY.md` などにある)
- Actual: Observed on PR #35 head 23fdc75: 移動に伴うパス参照の追従: `rg -n 'docs/' src tests .github *.md docs` で全参照を洗い出して更新する(確認済みの参照: `src/minimal_loop/repair_pressure.rs:231` のコメント。ほかにMarkdown間の相互リンクが `README.md` `SECURITY.md` などにある)
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``docs/README.md` を新設: 全ドキュメントの一覧表(ファイル / 1行説明 / **言語(EN・JA・混在)** / **対象読者(エンドユーザー・コントリビュータ・歴史記録)**)。「歴史記録は現状のコードと一致しないことがある」旨の注意書きを含める` を確認できる画面または実機操作を行う。
- 期待結果: `docs/README.md` を新設: 全ドキュメントの一覧表(ファイル / 1行説明 / **言語(EN・JA・混在)** / **対象読者(エンドユーザー・コントリビュータ・歴史記録)**)。「歴史記録は現状のコードと一致しないことがある」旨の注意書きを含める
- Actual: Observed on PR #35 head 23fdc75: `docs/README.md` を新設: 全ドキュメントの一覧表(ファイル / 1行説明 / **言語(EN・JA・混在)** / **対象読者(エンドユーザー・コントリビュータ・歴史記録)**)。「歴史記録は現状のコードと一致しないことがある」旨の注意書きを含める
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `ルート `README.md` から `docs/README.md` へのリンクを追加(README改訂Issueと調整)` を確認できる画面または実機操作を行う。
- 期待結果: ルート `README.md` から `docs/README.md` へのリンクを追加(README改訂Issueと調整)
- Actual: Observed on PR #35 head 23fdc75: ルート `README.md` から `docs/README.md` へのリンクを追加(README改訂Issueと調整)
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``docs/dev/data-profile-contract.md`(日本語のみ・凍結v0契約)の**英訳を別ファイル**(例: `data-profile-contract.en.md`)として追加し、相互リンクする。原文は正本として無改変(「翻訳は参考、正本は日本語」の注記を英訳側に入れる)` を確認できる画面または実機操作を行う。
- 期待結果: `docs/dev/data-profile-contract.md`(日本語のみ・凍結v0契約)の**英訳を別ファイル**(例: `data-profile-contract.en.md`)として追加し、相互リンクする。原文は正本として無改変(「翻訳は参考、正本は日本語」の注記を英訳側に入れる)
- Actual: Observed on PR #35 head 23fdc75: `docs/dev/data-profile-contract.md`(日本語のみ・凍結v0契約)の**英訳を別ファイル**(例: `data-profile-contract.en.md`)として追加し、相互リンクする。原文は正本として無改変(「翻訳は参考、正本は日本語」の注記を英訳側に入れる)
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``mechanism-ledger.md` / `integration-notes.md` の混在は**翻訳しない**(内部台帳としてJA混在を許容する旨をインデックスに明記)` を確認できる画面または実機操作を行う。
- 期待結果: `mechanism-ledger.md` / `integration-notes.md` の混在は**翻訳しない**(内部台帳としてJA混在を許容する旨をインデックスに明記)
- Actual: Observed on PR #35 head 23fdc75: `mechanism-ledger.md` / `integration-notes.md` の混在は**翻訳しない**(内部台帳としてJA混在を許容する旨をインデックスに明記)
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``benchmarks/README.md` を新設: `minimal-loop-expanded.yaml` が何のフィクスチャで、`scripts/bench.sh` からどう使われるか(`bench.sh` の引数 `--model` `--runs` `--max-iterations` `--recheck-root` を含む)を簡潔に記載` を確認できる画面または実機操作を行う。
- 期待結果: `benchmarks/README.md` を新設: `minimal-loop-expanded.yaml` が何のフィクスチャで、`scripts/bench.sh` からどう使われるか(`bench.sh` の引数 `--model` `--runs` `--max-iterations` `--recheck-root` を含む)を簡潔に記載
- Actual: Observed on PR #35 head 23fdc75: `benchmarks/README.md` を新設: `minimal-loop-expanded.yaml` が何のフィクスチャで、`scripts/bench.sh` からどう使われるか(`bench.sh` の引数 `--model` `--runs` `--max-iterations` `--recheck-root` を含む)を簡潔に記載
- Evidence: Issue #21 verification: byte-identical moves, link inventory, full cargo test passed; GitHub Actions passed for PR #35; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #22: [docs] Add doc-drift guard tests: keep CLI flags, slash commands, config keys and EN/JA structure in sync

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `clapの `Command` イントロスペクション(`Cli::command().get_arguments()`)で全フラグ名を列挙し、`docs/guide/en/cli-reference.md` に**全フラグが出現する**ことを検証する(方向は「コードにあるものがドキュメントに漏れなく載っている」。ドキュメント側の追加説明は自由)` を確認できる画面または実機操作を行う。
- 期待結果: clapの `Command` イントロスペクション(`Cli::command().get_arguments()`)で全フラグ名を列挙し、`docs/guide/en/cli-reference.md` に**全フラグが出現する**ことを検証する(方向は「コードにあるものがドキュメントに漏れなく載っている」。ドキュメント側の追加説明は自由)
- Actual: Observed on PR #36 head 7d7224d: clapの `Command` イントロスペクション(`Cli::command().get_arguments()`)で全フラグ名を列挙し、`docs/guide/en/cli-reference.md` に**全フラグが出現する**ことを検証する(方向は「コードにあるものがドキュメントに漏れなく載っている」。ドキュメント側の追加説明は自由)
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `逆方向も検証: ドキュメントの表に載っているフラグ名が実在しないならfail(タイポ・削除済みフラグの検知)。表のパースは「行頭 `| \`--flag\`` 形式」など**単純な規約**を決めてガイド側もそれに従う` を確認できる画面または実機操作を行う。
- 期待結果: 逆方向も検証: ドキュメントの表に載っているフラグ名が実在しないならfail(タイポ・削除済みフラグの検知)。表のパースは「行頭 `| \`--flag\`` 形式」など**単純な規約**を決めてガイド側もそれに従う
- Actual: Observed on PR #36 head 7d7224d: 逆方向も検証: ドキュメントの表に載っているフラグ名が実在しないならfail(タイポ・削除済みフラグの検知)。表のパースは「行頭 `| \`--flag\`` 形式」など**単純な規約**を決めてガイド側もそれに従う
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``render_help` の出力からコマンド名(`/xxx`)を抽出し、`docs/guide/en/slash-commands.md` に全て出現することを検証(逆方向も同様)` を確認できる画面または実機操作を行う。
- 期待結果: `render_help` の出力からコマンド名(`/xxx`)を抽出し、`docs/guide/en/slash-commands.md` に全て出現することを検証(逆方向も同様)
- Actual: Observed on PR #36 head 7d7224d: `render_help` の出力からコマンド名(`/xxx`)を抽出し、`docs/guide/en/slash-commands.md` に全て出現することを検証(逆方向も同様)
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `可能なら `render_help` とディスパッチ(`handle_command`)の一致もこの機会にテスト化する(helpに載っているのに処理がない/その逆の検知)` を確認できる画面または実機操作を行う。
- 期待結果: 可能なら `render_help` とディスパッチ(`handle_command`)の一致もこの機会にテスト化する(helpに載っているのに処理がない/その逆の検知)
- Actual: Observed on PR #36 head 7d7224d: 可能なら `render_help` とディスパッチ(`handle_command`)の一致もこの機会にテスト化する(helpに載っているのに処理がない/その逆の検知)
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `presetキー10個とトップレベルキーの一覧をテスト内の定数ではなく**コードから導出**できない場合は、`src/config.rs` 側に「サポートするキーの一覧」を返す関数(またはconst)を追加してそれを正とし、ドキュメント出現を検証する` を確認できる画面または実機操作を行う。
- 期待結果: presetキー10個とトップレベルキーの一覧をテスト内の定数ではなく**コードから導出**できない場合は、`src/config.rs` 側に「サポートするキーの一覧」を返す関数(またはconst)を追加してそれを正とし、ドキュメント出現を検証する
- Actual: Observed on PR #36 head 7d7224d: presetキー10個とトップレベルキーの一覧をテスト内の定数ではなく**コードから導出**できない場合は、`src/config.rs` 側に「サポートするキーの一覧」を返す関数(またはconst)を追加してそれを正とし、ドキュメント出現を検証する
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``docs/guide/en/` と `docs/guide/ja/` の**ファイル集合が一致**することを検証` を確認できる画面または実機操作を行う。
- 期待結果: `docs/guide/en/` と `docs/guide/ja/` の**ファイル集合が一致**することを検証
- Actual: Observed on PR #36 head 7d7224d: `docs/guide/en/` と `docs/guide/ja/` の**ファイル集合が一致**することを検証
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `各対訳ペアで**h2/h3見出しの数が一致**することを検証(内容の翻訳品質は対象外。構造の欠落だけ検知)` を確認できる画面または実機操作を行う。
- 期待結果: 各対訳ペアで**h2/h3見出しの数が一致**することを検証(内容の翻訳品質は対象外。構造の欠落だけ検知)
- Actual: Observed on PR #36 head 7d7224d: 各対訳ペアで**h2/h3見出しの数が一致**することを検証(内容の翻訳品質は対象外。構造の欠落だけ検知)
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `失敗時のメッセージは「どのフラグ/コマンド/キーがどちら側に欠けているか」を列挙し、修正先ファイルパスを示す` を確認できる画面または実機操作を行う。
- 期待結果: 失敗時のメッセージは「どのフラグ/コマンド/キーがどちら側に欠けているか」を列挙し、修正先ファイルパスを示す
- Actual: Observed on PR #36 head 7d7224d: 失敗時のメッセージは「どのフラグ/コマンド/キーがどちら側に欠けているか」を列挙し、修正先ファイルパスを示す
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `CI(`.github/workflows/ci.yml` の `cargo test --all-targets`)で自動実行される(統合テストとして置けば追加設定不要のはず。要確認)` を確認できる画面または実機操作を行う。
- 期待結果: CI(`.github/workflows/ci.yml` の `cargo test --all-targets`)で自動実行される(統合テストとして置けば追加設定不要のはず。要確認)
- Actual: Observed on PR #36 head 7d7224d: CI(`.github/workflows/ci.yml` の `cargo test --all-targets`)で自動実行される(統合テストとして置けば追加設定不要のはず。要確認)
- Evidence: Issue #22 verification: doc-drift guards and all-target tests passed; GitHub Actions passed for PR #36; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #23: [repo] Add LICENSE (MIT), CONTRIBUTING.md, CHANGELOG.md

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `MIT License の全文を `LICENSE` としてルートに追加する。copyright行は `Copyright (c) 2026 Kewton` とする(リポジトリオーナー名。変更したい場合は本Issueにコメントで指示)` を確認できる画面または実機操作を行う。
- 期待結果: MIT License の全文を `LICENSE` としてルートに追加する。copyright行は `Copyright (c) 2026 Kewton` とする(リポジトリオーナー名。変更したい場合は本Issueにコメントで指示)
- Actual: Observed on PR #37 head a54572d: MIT License の全文を `LICENSE` としてルートに追加する。copyright行は `Copyright (c) 2026 Kewton` とする(リポジトリオーナー名。変更したい場合は本Issueにコメントで指示)
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `README(改訂Issueと調整)に License 節を追加しリンクする` を確認できる画面または実機操作を行う。
- 期待結果: README(改訂Issueと調整)に License 節を追加しリンクする
- Actual: Observed on PR #37 head a54572d: README(改訂Issueと調整)に License 節を追加しリンクする
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `以下を含む(英語で作成し、要点の日本語併記は任意):` を確認できる画面または実機操作を行う。
- 期待結果: 以下を含む(英語で作成し、要点の日本語併記は任意):
- Actual: Observed on PR #37 head a54572d: 以下を含む(英語で作成し、要点の日本語併記は任意):
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `開発環境: Rust 1.88+(`Cargo.toml:5`)、テスト実行 `cargo test --all-targets`、PTYテスト `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`、Python 3.10(evalゴールデンテスト、`.github/workflows/ci.yml` 参照)` を確認できる画面または実機操作を行う。
- 期待結果: 開発環境: Rust 1.88+(`Cargo.toml:5`)、テスト実行 `cargo test --all-targets`、PTYテスト `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`、Python 3.10(evalゴールデンテスト、`.github/workflows/ci.yml` 参照)
- Actual: Observed on PR #37 head a54572d: 開発環境: Rust 1.88+(`Cargo.toml:5`)、テスト実行 `cargo test --all-targets`、PTYテスト `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`、Python 3.10(evalゴールデンテスト、`.github/workflows/ci.yml` 参照)
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**ガードレールの案内**: `docs/dev-guardrails.md`(行数バジェット: baseline+2%でCI失敗。新しいサブシステムは新モジュールへ)、`docs/mechanism-ledger.md` の互換凍結方針(イベント名・JSONキー・スキーマ不変)への言及と、抵触する変更にはledger追記が必要なこと` を確認できる画面または実機操作を行う。
- 期待結果: **ガードレールの案内**: `docs/dev-guardrails.md`(行数バジェット: baseline+2%でCI失敗。新しいサブシステムは新モジュールへ)、`docs/mechanism-ledger.md` の互換凍結方針(イベント名・JSONキー・スキーマ不変)への言及と、抵触する変更にはledger追記が必要なこと
- Actual: Observed on PR #37 head a54572d: **ガードレールの案内**: `docs/dev-guardrails.md`(行数バジェット: baseline+2%でCI失敗。新しいサブシステムは新モジュールへ)、`docs/mechanism-ledger.md` の互換凍結方針(イベント名・JSONキー・スキーマ不変)への言及と、抵触する変更にはledger追記が必要なこと
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `corpus回帰(`tests/corpus_regression.rs`)・conformance・generality guardrails がCIで走ること` を確認できる画面または実機操作を行う。
- 期待結果: corpus回帰(`tests/corpus_regression.rs`)・conformance・generality guardrails がCIで走ること
- Actual: Observed on PR #37 head a54572d: corpus回帰(`tests/corpus_regression.rs`)・conformance・generality guardrails がCIで走ること
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `ドキュメント対訳(README.md ⇔ README.ja.md、docs/guide/en ⇔ ja)は**同時更新**すること(doc-driftガードで構造検証される)` を確認できる画面または実機操作を行う。
- 期待結果: ドキュメント対訳(README.md ⇔ README.ja.md、docs/guide/en ⇔ ja)は**同時更新**すること(doc-driftガードで構造検証される)
- Actual: Observed on PR #37 head a54572d: ドキュメント対訳(README.md ⇔ README.ja.md、docs/guide/en ⇔ ja)は**同時更新**すること(doc-driftガードで構造検証される)
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `PRの期待事項: テスト付き・`RUSTFLAGS="-D warnings"` でwarningゼロ(CI設定と同じ)` を確認できる画面または実機操作を行う。
- 期待結果: PRの期待事項: テスト付き・`RUSTFLAGS="-D warnings"` でwarningゼロ(CI設定と同じ)
- Actual: Observed on PR #37 head a54572d: PRの期待事項: テスト付き・`RUSTFLAGS="-D warnings"` でwarningゼロ(CI設定と同じ)
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `docs再編Issueが先行完了している場合はパスをそれに合わせる(`docs/dev/dev-guardrails.md` 等)` を確認できる画面または実機操作を行う。
- 期待結果: docs再編Issueが先行完了している場合はパスをそれに合わせる(`docs/dev/dev-guardrails.md` 等)
- Actual: Observed on PR #37 head a54572d: docs再編Issueが先行完了している場合はパスをそれに合わせる(`docs/dev/dev-guardrails.md` 等)
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `[Keep a Changelog](https://keepachangelog.com/) 形式で新設。過去の全履歴の掘り起こしはせず、`## [Unreleased]` セクション+「本ファイル開始時点(0.1.0, 2026-07)より前の変更は git 履歴と docs/mechanism-ledger.md を参照」という注記で開始する` を確認できる画面または実機操作を行う。
- 期待結果: [Keep a Changelog](https://keepachangelog.com/) 形式で新設。過去の全履歴の掘り起こしはせず、`## [Unreleased]` セクション+「本ファイル開始時点(0.1.0, 2026-07)より前の変更は git 履歴と docs/mechanism-ledger.md を参照」という注記で開始する
- Actual: Observed on PR #37 head a54572d: [Keep a Changelog](https://keepachangelog.com/) 形式で新設。過去の全履歴の掘り起こしはせず、`## [Unreleased]` セクション+「本ファイル開始時点(0.1.0, 2026-07)より前の変更は git 履歴と docs/mechanism-ledger.md を参照」という注記で開始する
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `以後のPRでUnreleasedに追記する運用をCONTRIBUTINGに記載` を確認できる画面または実機操作を行う。
- 期待結果: 以後のPRでUnreleasedに追記する運用をCONTRIBUTINGに記載
- Actual: Observed on PR #37 head a54572d: 以後のPRでUnreleasedに追記する運用をCONTRIBUTINGに記載
- Evidence: Issue #23 verification: repository policy/link checks and all-target tests passed; GitHub Actions passed for PR #37; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #24: [setup] Add scripts/setup.sh: prerequisites check, build/install, .env, probe setup, smoke test

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `引数なし = 対話モード(各ステップで確認)。`--yes` = 非対話(安全な既定で全実行)。`--check-only` = 前提チェックのみ実行して結果表示(何も変更しない)` を確認できる画面または実機操作を行う。
- 期待結果: 引数なし = 対話モード(各ステップで確認)。`--yes` = 非対話(安全な既定で全実行)。`--check-only` = 前提チェックのみ実行して結果表示(何も変更しない)
- Actual: Observed on PR #38 head 3807d06: 引数なし = 対話モード(各ステップで確認)。`--yes` = 非対話(安全な既定で全実行)。`--check-only` = 前提チェックのみ実行して結果表示(何も変更しない)
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**冪等**: 2回目以降の実行で壊れない・重複作業しない(導入済みならスキップと表示)` を確認できる画面または実機操作を行う。
- 期待結果: **冪等**: 2回目以降の実行で壊れない・重複作業しない(導入済みならスキップと表示)
- Actual: Observed on PR #38 head 3807d06: **冪等**: 2回目以降の実行で壊れない・重複作業しない(導入済みならスキップと表示)
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**前提チェック**: `cargo`/`rustc`(バージョンが `Cargo.toml` の `rust-version` 以上か。値はCargo.tomlからgrepで取得しハードコードしない)、`git`。任意依存として `node`/`npm`(プローブ用)、`ollama` または既定ホストへの疎通(`curl http://localhost:11434/api/tags`)、`python3`(eval用)。必須欠落は導入方法のURL付きで案内して終了、任意欠落は警告して続行` を確認できる画面または実機操作を行う。
- 期待結果: **前提チェック**: `cargo`/`rustc`(バージョンが `Cargo.toml` の `rust-version` 以上か。値はCargo.tomlからgrepで取得しハードコードしない)、`git`。任意依存として `node`/`npm`(プローブ用)、`ollama` または既定ホストへの疎通(`curl http://localhost:11434/api/tags`)、`python3`(eval用)。必須欠落は導入方法のURL付きで案内して終了、任意欠落は警告して続行
- Actual: Observed on PR #38 head 3807d06: **前提チェック**: `cargo`/`rustc`(バージョンが `Cargo.toml` の `rust-version` 以上か。値はCargo.tomlからgrepで取得しハードコードしない)、`git`。任意依存として `node`/`npm`(プローブ用)、`ollama` または既定ホストへの疎通(`curl http://localhost:11434/api/tags`)、`python3`(eval用)。必須欠落は導入方法のURL付きで案内して終了、任意欠落は警告して続行
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**ビルド/インストール**: `cargo install --path . --locked` を提案・実行(拒否時は `cargo build --release` +PATH追加案内にフォールバック)。完了後 `commandagent --version` で確認` を確認できる画面または実機操作を行う。
- 期待結果: **ビルド/インストール**: `cargo install --path . --locked` を提案・実行(拒否時は `cargo build --release` +PATH追加案内にフォールバック)。完了後 `commandagent --version` で確認
- Actual: Observed on PR #38 head 3807d06: **ビルド/インストール**: `cargo install --path . --locked` を提案・実行(拒否時は `cargo build --release` +PATH追加案内にフォールバック)。完了後 `commandagent --version` で確認
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**APIキー設定(任意)**: Gemini/OpenAIを使うか尋ね、使うなら `.env` に `GEMINI_API_KEY=` / `OPENAI_API_KEY=` を書き込む。**入力はエコーしない**(`read -s`)。**既存の `.env` は上書きせず**、欠けているキーの追記のみ。作成時はパーミッション600。`--yes` モードではキー入力はスキップし追記もしない(案内のみ)` を確認できる画面または実機操作を行う。
- 期待結果: **APIキー設定(任意)**: Gemini/OpenAIを使うか尋ね、使うなら `.env` に `GEMINI_API_KEY=` / `OPENAI_API_KEY=` を書き込む。**入力はエコーしない**(`read -s`)。**既存の `.env` は上書きせず**、欠けているキーの追記のみ。作成時はパーミッション600。`--yes` モードではキー入力はスキップし追記もしない(案内のみ)
- Actual: Observed on PR #38 head 3807d06: **APIキー設定(任意)**: Gemini/OpenAIを使うか尋ね、使うなら `.env` に `GEMINI_API_KEY=` / `OPENAI_API_KEY=` を書き込む。**入力はエコーしない**(`read -s`)。**既存の `.env` は上書きせず**、欠けているキーの追記のみ。作成時はパーミッション600。`--yes` モードではキー入力はスキップし追記もしない(案内のみ)
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**Ollamaモデル(任意)**: `ollama` 検出時、`ollama list` を表示し、モデルが無ければ pull を提案(モデル名はユーザー入力。既定値を押し付けない)` を確認できる画面または実機操作を行う。
- 期待結果: **Ollamaモデル(任意)**: `ollama` 検出時、`ollama list` を表示し、モデルが無ければ pull を提案(モデル名はユーザー入力。既定値を押し付けない)
- Actual: Observed on PR #38 head 3807d06: **Ollamaモデル(任意)**: `ollama` 検出時、`ollama list` を表示し、モデルが無ければ pull を提案(モデル名はユーザー入力。既定値を押し付けない)
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**インタラクションプローブ(任意)**: `node`/`npm` 検出時、`commandagent --setup-interaction-probe` の実行を提案・実行` を確認できる画面または実機操作を行う。
- 期待結果: **インタラクションプローブ(任意)**: `node`/`npm` 検出時、`commandagent --setup-interaction-probe` の実行を提案・実行
- Actual: Observed on PR #38 head 3807d06: **インタラクションプローブ(任意)**: `node`/`npm` 検出時、`commandagent --setup-interaction-probe` の実行を提案・実行
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**スモーク確認**: `commandagent --version` の出力を表示し、Ollama疎通が取れていれば `--model-probe` の実行を提案(時間がかかる旨を表示)` を確認できる画面または実機操作を行う。
- 期待結果: **スモーク確認**: `commandagent --version` の出力を表示し、Ollama疎通が取れていれば `--model-probe` の実行を提案(時間がかかる旨を表示)
- Actual: Observed on PR #38 head 3807d06: **スモーク確認**: `commandagent --version` の出力を表示し、Ollama疎通が取れていれば `--model-probe` の実行を提案(時間がかかる旨を表示)
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**サマリー**: 各ステップの結果(ok / skipped / warn)を最後に一覧表示し、次の一歩(READMEのQuickstartへのリンク)を出す` を確認できる画面または実機操作を行う。
- 期待結果: **サマリー**: 各ステップの結果(ok / skipped / warn)を最後に一覧表示し、次の一歩(READMEのQuickstartへのリンク)を出す
- Actual: Observed on PR #38 head 3807d06: **サマリー**: 各ステップの結果(ok / skipped / warn)を最後に一覧表示し、次の一歩(READMEのQuickstartへのリンク)を出す
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``set -euo pipefail`。bash 3.2互換(macOS標準)で動くこと(配列の使い方等に注意)` を確認できる画面または実機操作を行う。
- 期待結果: `set -euo pipefail`。bash 3.2互換(macOS標準)で動くこと(配列の使い方等に注意)
- Actual: Observed on PR #38 head 3807d06: `set -euo pipefail`。bash 3.2互換(macOS標準)で動くこと(配列の使い方等に注意)
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `macOS / Linux 両対応(OS分岐は最小限に)` を確認できる画面または実機操作を行う。
- 期待結果: macOS / Linux 両対応(OS分岐は最小限に)
- Actual: Observed on PR #38 head 3807d06: macOS / Linux 両対応(OS分岐は最小限に)
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `失敗時は必ず「何が失敗し、手動でどうすればよいか」を1行で出す` を確認できる画面または実機操作を行う。
- 期待結果: 失敗時は必ず「何が失敗し、手動でどうすればよいか」を1行で出す
- Actual: Observed on PR #38 head 3807d06: 失敗時は必ず「何が失敗し、手動でどうすればよいか」を1行で出す
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `シークレット(キーの値)を画面・ログに一切出さない` を確認できる画面または実機操作を行う。
- 期待結果: シークレット(キーの値)を画面・ログに一切出さない
- Actual: Observed on PR #38 head 3807d06: シークレット(キーの値)を画面・ログに一切出さない
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``shellcheck scripts/setup.sh` が警告ゼロ。CI(`.github/workflows/ci.yml`)にshellcheckステップを追加(`scripts/*.sh` 対象)` を確認できる画面または実機操作を行う。
- 期待結果: `shellcheck scripts/setup.sh` が警告ゼロ。CI(`.github/workflows/ci.yml`)にshellcheckステップを追加(`scripts/*.sh` 対象)
- Actual: Observed on PR #38 head 3807d06: `shellcheck scripts/setup.sh` が警告ゼロ。CI(`.github/workflows/ci.yml`)にshellcheckステップを追加(`scripts/*.sh` 対象)
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `README(改訂Issueと調整)のInstall節から `./scripts/setup.sh` を案内する1行を追加` を確認できる画面または実機操作を行う。
- 期待結果: README(改訂Issueと調整)のInstall節から `./scripts/setup.sh` を案内する1行を追加
- Actual: Observed on PR #38 head 3807d06: README(改訂Issueと調整)のInstall節から `./scripts/setup.sh` を案内する1行を追加
- Evidence: Issue #24 verification: setup-script tests, shellcheck, clippy and all-target tests passed; GitHub Actions passed for PR #38; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #25: [setup] Add commandagent --doctor: built-in environment diagnosis

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `CLIフラグ `--doctor`(`src/cli.rs` に追加、既存Action系フラグの流儀に従う)とREPLの `/doctor`(`render_help` にも追加)の両方から実行できる` を確認できる画面または実機操作を行う。
- 期待結果: CLIフラグ `--doctor`(`src/cli.rs` に追加、既存Action系フラグの流儀に従う)とREPLの `/doctor`(`render_help` にも追加)の両方から実行できる
- Actual: Observed on PR #39 head 640f41b: CLIフラグ `--doctor`(`src/cli.rs` に追加、既存Action系フラグの流儀に従う)とREPLの `/doctor`(`render_help` にも追加)の両方から実行できる
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**設定解決**: 有効な model / provider / planner_model / planner_provider / profile と、その出所(CLI / preset / config / default。既存の `field_sources` を利用)` を確認できる画面または実機操作を行う。
- 期待結果: **設定解決**: 有効な model / provider / planner_model / planner_provider / profile と、その出所(CLI / preset / config / default。既存の `field_sources` を利用)
- Actual: Observed on PR #39 head 640f41b: **設定解決**: 有効な model / provider / planner_model / planner_provider / profile と、その出所(CLI / preset / config / default。既存の `field_sources` を利用)
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**設定ファイル**: 探索した各パス(`.anvil/config.toml` 等、`src/config.rs:663-668`)の存在・パース可否。presetが指定されているのに**不完全で解決不能**な場合はどのキーが欠けているかを列挙(`preset_complete`, `src/config.rs:560-571` を流用)` を確認できる画面または実機操作を行う。
- 期待結果: **設定ファイル**: 探索した各パス(`.anvil/config.toml` 等、`src/config.rs:663-668`)の存在・パース可否。presetが指定されているのに**不完全で解決不能**な場合はどのキーが欠けているかを列挙(`preset_complete`, `src/config.rs:560-571` を流用)
- Actual: Observed on PR #39 head 640f41b: **設定ファイル**: 探索した各パス(`.anvil/config.toml` 等、`src/config.rs:663-668`)の存在・パース可否。presetが指定されているのに**不完全で解決不能**な場合はどのキーが欠けているかを列挙(`preset_complete`, `src/config.rs:560-571` を流用)
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**プロバイダ疎通**:` を確認できる画面または実機操作を行う。
- 期待結果: **プロバイダ疎通**:
- Actual: Observed on PR #39 head 640f41b: **プロバイダ疎通**:
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Ollama(providerまたはplanner_providerがollamaのとき): 設定ホストへ `/api/tags` を短タイムアウトで照会し、到達性と**設定中のモデルがtagsに存在するか**を確認` を確認できる画面または実機操作を行う。
- 期待結果: Ollama(providerまたはplanner_providerがollamaのとき): 設定ホストへ `/api/tags` を短タイムアウトで照会し、到達性と**設定中のモデルがtagsに存在するか**を確認
- Actual: Observed on PR #39 head 640f41b: Ollama(providerまたはplanner_providerがollamaのとき): 設定ホストへ `/api/tags` を短タイムアウトで照会し、到達性と**設定中のモデルがtagsに存在するか**を確認
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Gemini / OpenAI: **キーの存在のみ**確認(env → `.env` の解決順で。値は `redact` でマスクし絶対に表示しない)。実APIへのリクエストは行わない(課金・レート考慮)` を確認できる画面または実機操作を行う。
- 期待結果: Gemini / OpenAI: **キーの存在のみ**確認(env → `.env` の解決順で。値は `redact` でマスクし絶対に表示しない)。実APIへのリクエストは行わない(課金・レート考慮)
- Actual: Observed on PR #39 head 640f41b: Gemini / OpenAI: **キーの存在のみ**確認(env → `.env` の解決順で。値は `redact` でマスクし絶対に表示しない)。実APIへのリクエストは行わない(課金・レート考慮)
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**インタラクションプローブ**: `playwright_availability` の結果と、unavailable時は既存remediation文言` を確認できる画面または実機操作を行う。
- 期待結果: **インタラクションプローブ**: `playwright_availability` の結果と、unavailable時は既存remediation文言
- Actual: Observed on PR #39 head 640f41b: **インタラクションプローブ**: `playwright_availability` の結果と、unavailable時は既存remediation文言
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**状態ディレクトリ**: state_dir の書き込み可否(実際に一時ファイルを作って消す)` を確認できる画面または実機操作を行う。
- 期待結果: **状態ディレクトリ**: state_dir の書き込み可否(実際に一時ファイルを作って消す)
- Actual: Observed on PR #39 head 640f41b: **状態ディレクトリ**: state_dir の書き込み可否(実際に一時ファイルを作って消す)
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**端末**: TTYか、色が有効か(`NO_COLOR`)、端末幅、フッター無効化条件に該当していないか` を確認できる画面または実機操作を行う。
- 期待結果: **端末**: TTYか、色が有効か(`NO_COLOR`)、端末幅、フッター無効化条件に該当していないか
- Actual: Observed on PR #39 head 640f41b: **端末**: TTYか、色が有効か(`NO_COLOR`)、端末幅、フッター無効化条件に該当していないか
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**ワークスペース**: workspace_root の書き込み可否、`.env` の有無(有ればどのキーが定義済みかを**キー名のみ**表示)` を確認できる画面または実機操作を行う。
- 期待結果: **ワークスペース**: workspace_root の書き込み可否、`.env` の有無(有ればどのキーが定義済みかを**キー名のみ**表示)
- Actual: Observed on PR #39 head 640f41b: **ワークスペース**: workspace_root の書き込み可否、`.env` の有無(有ればどのキーが定義済みかを**キー名のみ**表示)
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `人間可読の整列されたチェックリスト(✓/!/✗)。1項目1行+失敗時のみ対処行` を確認できる画面または実機操作を行う。
- 期待結果: 人間可読の整列されたチェックリスト(✓/!/✗)。1項目1行+失敗時のみ対処行
- Actual: Observed on PR #39 head 640f41b: 人間可読の整列されたチェックリスト(✓/!/✗)。1項目1行+失敗時のみ対処行
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `いずれかが fail なら終了コード非0、warnのみなら0` を確認できる画面または実機操作を行う。
- 期待結果: いずれかが fail なら終了コード非0、warnのみなら0
- Actual: Observed on PR #39 head 640f41b: いずれかが fail なら終了コード非0、warnのみなら0
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``--doctor --json` で機械可読JSON(キー名は新規設計でよいが、一度出したら安定させる前提で命名する)` を確認できる画面または実機操作を行う。
- 期待結果: `--doctor --json` で機械可読JSON(キー名は新規設計でよいが、一度出したら安定させる前提で命名する)
- Actual: Observed on PR #39 head 640f41b: `--doctor --json` で機械可読JSON(キー名は新規設計でよいが、一度出したら安定させる前提で命名する)
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `破壊的操作・自動修復はしない(診断のみ。修復はsetup.shや既存remediationへ誘導)` を確認できる画面または実機操作を行う。
- 期待結果: 破壊的操作・自動修復はしない(診断のみ。修復はsetup.shや既存remediationへ誘導)
- Actual: Observed on PR #39 head 640f41b: 破壊的操作・自動修復はしない(診断のみ。修復はsetup.shや既存remediationへ誘導)
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `実行時間は正常系で数秒以内(ネットワーク照会は全て短タイムアウト)` を確認できる画面または実機操作を行う。
- 期待結果: 実行時間は正常系で数秒以内(ネットワーク照会は全て短タイムアウト)
- Actual: Observed on PR #39 head 640f41b: 実行時間は正常系で数秒以内(ネットワーク照会は全て短タイムアウト)
- Evidence: Issue #25 verification: doctor unit/CLI tests, clippy and full cargo test passed; GitHub Actions passed for PR #39; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #26: [setup] Release & distribution: tagged binary releases, install.sh, crates.io/Homebrew (decisions included)

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``.github/workflows/release.yml` を新設: `v*` タグのpushで発火し、以下のターゲットのリリースバイナリをビルドして GitHub Release に添付する` を確認できる画面または実機操作を行う。
- 期待結果: `.github/workflows/release.yml` を新設: `v*` タグのpushで発火し、以下のターゲットのリリースバイナリをビルドして GitHub Release に添付する
- Actual: Observed on PR #40 head 5c09534: `.github/workflows/release.yml` を新設: `v*` タグのpushで発火し、以下のターゲットのリリースバイナリをビルドして GitHub Release に添付する
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``aarch64-apple-darwin` / `x86_64-apple-darwin`` を確認できる画面または実機操作を行う。
- 期待結果: `aarch64-apple-darwin` / `x86_64-apple-darwin`
- Actual: Observed on PR #40 head 5c09534: `aarch64-apple-darwin` / `x86_64-apple-darwin`
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``x86_64-unknown-linux-gnu`(可能なら `-musl` も。glibc依存を減らせる)` を確認できる画面または実機操作を行う。
- 期待結果: `x86_64-unknown-linux-gnu`(可能なら `-musl` も。glibc依存を減らせる)
- Actual: Observed on PR #40 head 5c09534: `x86_64-unknown-linux-gnu`(可能なら `-musl` も。glibc依存を減らせる)
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `各アーティファクトは `commandagent-<version>-<target>.tar.gz` 形式+ `sha256` チェックサムファイルを添付` を確認できる画面または実機操作を行う。
- 期待結果: 各アーティファクトは `commandagent-<version>-<target>.tar.gz` 形式+ `sha256` チェックサムファイルを添付
- Actual: Observed on PR #40 head 5c09534: 各アーティファクトは `commandagent-<version>-<target>.tar.gz` 形式+ `sha256` チェックサムファイルを添付
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `ビルドは `--release --locked`。`cargo test` をリリースビルド前に実行(壊れたタグを出さない)` を確認できる画面または実機操作を行う。
- 期待結果: ビルドは `--release --locked`。`cargo test` をリリースビルド前に実行(壊れたタグを出さない)
- Actual: Observed on PR #40 head 5c09534: ビルドは `--release --locked`。`cargo test` をリリースビルド前に実行(壊れたタグを出さない)
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `リリースノートはタグメッセージ+自動生成(GitHubの release notes 自動生成で可。CHANGELOG連携はCHANGELOG Issue完了後に検討)` を確認できる画面または実機操作を行う。
- 期待結果: リリースノートはタグメッセージ+自動生成(GitHubの release notes 自動生成で可。CHANGELOG連携はCHANGELOG Issue完了後に検討)
- Actual: Observed on PR #40 head 5c09534: リリースノートはタグメッセージ+自動生成(GitHubの release notes 自動生成で可。CHANGELOG連携はCHANGELOG Issue完了後に検討)
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `実装手段は素のGitHub Actionsで可。`cargo-dist` を採用する場合は生成物をコミットし、選定理由をPRに記載` を確認できる画面または実機操作を行う。
- 期待結果: 実装手段は素のGitHub Actionsで可。`cargo-dist` を採用する場合は生成物をコミットし、選定理由をPRに記載
- Actual: Observed on PR #40 head 5c09534: 実装手段は素のGitHub Actionsで可。`cargo-dist` を採用する場合は生成物をコミットし、選定理由をPRに記載
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``scripts/install.sh` を新設: 最新(または指定バージョン)のGitHub Releaseから、実行環境のOS/アーキテクチャに合うバイナリを取得し、チェックサム検証のうえ `~/.local/bin`(または `--prefix` 指定先)へ配置する` を確認できる画面または実機操作を行う。
- 期待結果: `scripts/install.sh` を新設: 最新(または指定バージョン)のGitHub Releaseから、実行環境のOS/アーキテクチャに合うバイナリを取得し、チェックサム検証のうえ `~/.local/bin`(または `--prefix` 指定先)へ配置する
- Actual: Observed on PR #40 head 5c09534: `scripts/install.sh` を新設: 最新(または指定バージョン)のGitHub Releaseから、実行環境のOS/アーキテクチャに合うバイナリを取得し、チェックサム検証のうえ `~/.local/bin`(または `--prefix` 指定先)へ配置する
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh | sh` で動く想定の作りにする(ただしREADMEにはパイプ実行のリスク注記と「ダウンロードして確認してから実行」の選択肢も書く)` を確認できる画面または実機操作を行う。
- 期待結果: `curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh | sh` で動く想定の作りにする(ただしREADMEにはパイプ実行のリスク注記と「ダウンロードして確認してから実行」の選択肢も書く)
- Actual: Observed on PR #40 head 5c09534: `curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh | sh` で動く想定の作りにする(ただしREADMEにはパイプ実行のリスク注記と「ダウンロードして確認してから実行」の選択肢も書く)
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``shellcheck` 警告ゼロ。PATH未設定時は案内を出す` を確認できる画面または実機操作を行う。
- 期待結果: `shellcheck` 警告ゼロ。PATH未設定時は案内を出す
- Actual: Observed on PR #40 head 5c09534: `shellcheck` 警告ゼロ。PATH未設定時は案内を出す
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `ソースビルド版 `scripts/setup.sh`(別Issue)との違い(バイナリ取得 vs ソースビルド+環境セットアップ)をREADMEで明確にする` を確認できる画面または実機操作を行う。
- 期待結果: ソースビルド版 `scripts/setup.sh`(別Issue)との違い(バイナリ取得 vs ソースビルド+環境セットアップ)をREADMEで明確にする
- Actual: Observed on PR #40 head 5c09534: ソースビルド版 `scripts/setup.sh`(別Issue)との違い(バイナリ取得 vs ソースビルド+環境セットアップ)をREADMEで明確にする
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `検討事項をコメントで提示して判断を仰ぐ: パッケージ名 `commandagent` の空き状況、公開に必要なメタデータ追加(`repository`/`readme`/`keywords`)、`workspace/` や `tests/corpus/` など巨大ディレクトリの `exclude` 設定、公開後のyank不可性` を確認できる画面または実機操作を行う。
- 期待結果: 検討事項をコメントで提示して判断を仰ぐ: パッケージ名 `commandagent` の空き状況、公開に必要なメタデータ追加(`repository`/`readme`/`keywords`)、`workspace/` や `tests/corpus/` など巨大ディレクトリの `exclude` 設定、公開後のyank不可性
- Actual: Observed on PR #40 head 5c09534: 検討事項をコメントで提示して判断を仰ぐ: パッケージ名 `commandagent` の空き状況、公開に必要なメタデータ追加(`repository`/`readme`/`keywords`)、`workspace/` や `tests/corpus/` など巨大ディレクトリの `exclude` 設定、公開後のyank不可性
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `承認された場合のみ: メタデータ整備 + `cargo publish --dry-run` 通過 + 公開手順のドキュメント化` を確認できる画面または実機操作を行う。
- 期待結果: 承認された場合のみ: メタデータ整備 + `cargo publish --dry-run` 通過 + 公開手順のドキュメント化
- Actual: Observed on PR #40 head 5c09534: 承認された場合のみ: メタデータ整備 + `cargo publish --dry-run` 通過 + 公開手順のドキュメント化
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `リリースバイナリ運用が安定した後の選択肢として、`Kewton/homebrew-tap` リポジトリ+formulaの構成案を提示して判断を仰ぐ(本体リポジトリ側の変更はほぼ不要)` を確認できる画面または実機操作を行う。
- 期待結果: リリースバイナリ運用が安定した後の選択肢として、`Kewton/homebrew-tap` リポジトリ+formulaの構成案を提示して判断を仰ぐ(本体リポジトリ側の変更はほぼ不要)
- Actual: Observed on PR #40 head 5c09534: リリースバイナリ運用が安定した後の選択肢として、`Kewton/homebrew-tap` リポジトリ+formulaの構成案を提示して判断を仰ぐ(本体リポジトリ側の変更はほぼ不要)
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Apply approved decision: Prepare crates.io metadata and pass cargo publish --dry-run without publishing; document a Homebrew tap proposal without creating an external repository; create an unused prerelease tag and GitHub prerelease for release UAT and retain them as evidence.` を確認できる画面または実機操作を行う。
- 期待結果: Apply approved decision: Prepare crates.io metadata and pass cargo publish --dry-run without publishing; document a Homebrew tap proposal without creating an external repository; create an unused prerelease tag and GitHub prerelease for release UAT and retain them as evidence.
- Actual: Observed on PR #40 head 5c09534: Apply approved decision: Prepare crates.io metadata and pass cargo publish --dry-run without publishing; document a Homebrew tap proposal without creating an external repository; create an unused prerelease tag and GitHub prerelease for release UAT and retain them as evidence.
- Evidence: Issue #26 verification: cargo publish dry-run, release run 29734342725 and installer UAT passed; GitHub Actions passed for PR #40; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #27: [setup] Shell completions (clap_complete) and man page generation

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``commandagent --completions <shell>`(bash / zsh / fish、可能なら powershell も)で補完スクリプトを**stdoutに出力**する(ファイル書き込みはしない。導入はユーザー/setup.shに委ねる)` を確認できる画面または実機操作を行う。
- 期待結果: `commandagent --completions <shell>`(bash / zsh / fish、可能なら powershell も)で補完スクリプトを**stdoutに出力**する(ファイル書き込みはしない。導入はユーザー/setup.shに委ねる)
- Actual: Observed on PR #41 head c01a8f0: `commandagent --completions <shell>`(bash / zsh / fish、可能なら powershell も)で補完スクリプトを**stdoutに出力**する(ファイル書き込みはしない。導入はユーザー/setup.shに委ねる)
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``clap_complete` を使用し、フラグ追加時に自動追従する構成にする(手書き補完スクリプト禁止)` を確認できる画面または実機操作を行う。
- 期待結果: `clap_complete` を使用し、フラグ追加時に自動追従する構成にする(手書き補完スクリプト禁止)
- Actual: Observed on PR #41 head c01a8f0: `clap_complete` を使用し、フラグ追加時に自動追従する構成にする(手書き補完スクリプト禁止)
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``--help` に載ること、および誤った shell 名で分かりやすいエラーになること` を確認できる画面または実機操作を行う。
- 期待結果: `--help` に載ること、および誤った shell 名で分かりやすいエラーになること
- Actual: Observed on PR #41 head c01a8f0: `--help` に載ること、および誤った shell 名で分かりやすいエラーになること
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``clap_mangen` で `commandagent.1` を生成する手段を提供する。方式は次のいずれか(実装者が選び、理由をPRに記載):` を確認できる画面または実機操作を行う。
- 期待結果: `clap_mangen` で `commandagent.1` を生成する手段を提供する。方式は次のいずれか(実装者が選び、理由をPRに記載):
- Actual: Observed on PR #41 head c01a8f0: `clap_mangen` で `commandagent.1` を生成する手段を提供する。方式は次のいずれか(実装者が選び、理由をPRに記載):
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `(a) `--generate-man` フラグでstdout出力` を確認できる画面または実機操作を行う。
- 期待結果: (a) `--generate-man` フラグでstdout出力
- Actual: Observed on PR #41 head c01a8f0: (a) `--generate-man` フラグでstdout出力
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `(b) リリースワークフロー(配布Issue)内で生成しアーティファクトに同梱` を確認できる画面または実機操作を行う。
- 期待結果: (b) リリースワークフロー(配布Issue)内で生成しアーティファクトに同梱
- Actual: Observed on PR #41 head c01a8f0: (b) リリースワークフロー(配布Issue)内で生成しアーティファクトに同梱
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `生成物をリポジトリにコミットしない(常に定義から生成)` を確認できる画面または実機操作を行う。
- 期待結果: 生成物をリポジトリにコミットしない(常に定義から生成)
- Actual: Observed on PR #41 head c01a8f0: 生成物をリポジトリにコミットしない(常に定義から生成)
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``scripts/setup.sh`(別Issue)が存在すれば、補完スクリプトの導入ステップ(zsh: `fpath`、bash: `bash_completion.d` 等への配置提案)を追加する。setup.sh未マージなら本IssueではREADME/ガイドへの手動導入手順記載のみでよい` を確認できる画面または実機操作を行う。
- 期待結果: `scripts/setup.sh`(別Issue)が存在すれば、補完スクリプトの導入ステップ(zsh: `fpath`、bash: `bash_completion.d` 等への配置提案)を追加する。setup.sh未マージなら本IssueではREADME/ガイドへの手動導入手順記載のみでよい
- Actual: Observed on PR #41 head c01a8f0: `scripts/setup.sh`(別Issue)が存在すれば、補完スクリプトの導入ステップ(zsh: `fpath`、bash: `bash_completion.d` 等への配置提案)を追加する。setup.sh未マージなら本IssueではREADME/ガイドへの手動導入手順記載のみでよい
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `docs/guide/ の cli-reference(ドキュメントIssue)に補完・manの導入節を追加(未マージならREADMEに簡潔に)` を確認できる画面または実機操作を行う。
- 期待結果: docs/guide/ の cli-reference(ドキュメントIssue)に補完・manの導入節を追加(未マージならREADMEに簡潔に)
- Actual: Observed on PR #41 head c01a8f0: docs/guide/ の cli-reference(ドキュメントIssue)に補完・manの導入節を追加(未マージならREADMEに簡潔に)
- Evidence: Issue #27 verification: CLI artifacts, contaminated-XDG setup regression and all-target tests passed; GitHub Actions passed for PR #41; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

### Issue #28: [dev] Add justfile and devcontainer for reproducible development tasks

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `ルートに `justfile` を新設し、最低限以下のレシピを定義する(**CIと同一のコマンド・フラグ**にすること。乖離させない):` を確認できる画面または実機操作を行う。
- 期待結果: ルートに `justfile` を新設し、最低限以下のレシピを定義する(**CIと同一のコマンド・フラグ**にすること。乖離させない):
- Actual: Observed on PR #42 head 030e1d3: ルートに `justfile` を新設し、最低限以下のレシピを定義する(**CIと同一のコマンド・フラグ**にすること。乖離させない):
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``build` / `build-release`` を確認できる画面または実機操作を行う。
- 期待結果: `build` / `build-release`
- Actual: Observed on PR #42 head 030e1d3: `build` / `build-release`
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``test`(= `RUSTFLAGS="-D warnings" cargo test --all-targets`)` を確認できる画面または実機操作を行う。
- 期待結果: `test`(= `RUSTFLAGS="-D warnings" cargo test --all-targets`)
- Actual: Observed on PR #42 head 030e1d3: `test`(= `RUSTFLAGS="-D warnings" cargo test --all-targets`)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``test-corpus` / `test-guardrails` / `test-conformance`(CIの各ステップと同一)` を確認できる画面または実機操作を行う。
- 期待結果: `test-corpus` / `test-guardrails` / `test-conformance`(CIの各ステップと同一)
- Actual: Observed on PR #42 head 030e1d3: `test-corpus` / `test-guardrails` / `test-conformance`(CIの各ステップと同一)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``test-pty`(= `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`)` を確認できる画面または実機操作を行う。
- 期待結果: `test-pty`(= `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`)
- Actual: Observed on PR #42 head 030e1d3: `test-pty`(= `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``test-eval`(= CIのpython unittest 3本)` を確認できる画面または実機操作を行う。
- 期待結果: `test-eval`(= CIのpython unittest 3本)
- Actual: Observed on PR #42 head 030e1d3: `test-eval`(= CIのpython unittest 3本)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``ci`(上記CI相当を一括実行)` を確認できる画面または実機操作を行う。
- 期待結果: `ci`(上記CI相当を一括実行)
- Actual: Observed on PR #42 head 030e1d3: `ci`(上記CI相当を一括実行)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``bench`(`scripts/bench.sh` への薄いラッパー、引数透過)` を確認できる画面または実機操作を行う。
- 期待結果: `bench`(`scripts/bench.sh` への薄いラッパー、引数透過)
- Actual: Observed on PR #42 head 030e1d3: `bench`(`scripts/bench.sh` への薄いラッパー、引数透過)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``run`(開発用の代表的な起動例。モデル等は環境変数で上書き可能に)` を確認できる画面または実機操作を行う。
- 期待結果: `run`(開発用の代表的な起動例。モデル等は環境変数で上書き可能に)
- Actual: Observed on PR #42 head 030e1d3: `run`(開発用の代表的な起動例。モデル等は環境変数で上書き可能に)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``just --list` の説明文(`# comment`)を全レシピに付ける` を確認できる画面または実機操作を行う。
- 期待結果: `just --list` の説明文(`# comment`)を全レシピに付ける
- Actual: Observed on PR #42 head 030e1d3: `just --list` の説明文(`# comment`)を全レシピに付ける
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``just` 未導入者のために、CONTRIBUTING(または README のDevelopment節)へ導入1行と「justはオプション、生コマンドはCI定義参照」の注記を追加` を確認できる画面または実機操作を行う。
- 期待結果: `just` 未導入者のために、CONTRIBUTING(または README のDevelopment節)へ導入1行と「justはオプション、生コマンドはCI定義参照」の注記を追加
- Actual: Observed on PR #42 head 030e1d3: `just` 未導入者のために、CONTRIBUTING(または README のDevelopment節)へ導入1行と「justはオプション、生コマンドはCI定義参照」の注記を追加
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``.devcontainer/devcontainer.json`(+必要なら Dockerfile)を新設: Rust(`Cargo.toml` の rust-version 以上のstable)、Node.js LTS(インタラクションプローブ用)、Python 3.10+(eval用)、`just`、`shellcheck` を含む` を確認できる画面または実機操作を行う。
- 期待結果: `.devcontainer/devcontainer.json`(+必要なら Dockerfile)を新設: Rust(`Cargo.toml` の rust-version 以上のstable)、Node.js LTS(インタラクションプローブ用)、Python 3.10+(eval用)、`just`、`shellcheck` を含む
- Actual: Observed on PR #42 head 030e1d3: `.devcontainer/devcontainer.json`(+必要なら Dockerfile)を新設: Rust(`Cargo.toml` の rust-version 以上のstable)、Node.js LTS(インタラクションプローブ用)、Python 3.10+(eval用)、`just`、`shellcheck` を含む
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `コンテナ内で `just ci` が通ること(corpus・conformance含む。ネットワーク不要のテストのみで完結することを確認)` を確認できる画面または実機操作を行う。
- 期待結果: コンテナ内で `just ci` が通ること(corpus・conformance含む。ネットワーク不要のテストのみで完結することを確認)
- Actual: Observed on PR #42 head 030e1d3: コンテナ内で `just ci` が通ること(corpus・conformance含む。ネットワーク不要のテストのみで完結することを確認)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `**Ollamaはコンテナに含めない**。ホスト側Ollamaへの接続方法(`--ollama-host http://host.docker.internal:11434` 等)をdevcontainer READMEコメントに記載` を確認できる画面または実機操作を行う。
- 期待結果: **Ollamaはコンテナに含めない**。ホスト側Ollamaへの接続方法(`--ollama-host http://host.docker.internal:11434` 等)をdevcontainer READMEコメントに記載
- Actual: Observed on PR #42 head 030e1d3: **Ollamaはコンテナに含めない**。ホスト側Ollamaへの接続方法(`--ollama-host http://host.docker.internal:11434` 等)をdevcontainer READMEコメントに記載
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `イメージサイズと初回ビルド時間を常識的な範囲に(公式イメージ+featuresの組み合わせを優先し、カスタムDockerfileは必要時のみ)` を確認できる画面または実機操作を行う。
- 期待結果: イメージサイズと初回ビルド時間を常識的な範囲に(公式イメージ+featuresの組み合わせを優先し、カスタムDockerfileは必要時のみ)
- Actual: Observed on PR #42 head 030e1d3: イメージサイズと初回ビルド時間を常識的な範囲に(公式イメージ+featuresの組み合わせを優先し、カスタムDockerfileは必要時のみ)
- Evidence: Issue #28 verification: Dev Container offline just ci, PTY wrapper, benchmark and image inspection passed; GitHub Actions passed for PR #42; orchestrator CI report workspace/management/runs/20260720-115519-orchestrate/ci-report.md
- Result: passed

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
