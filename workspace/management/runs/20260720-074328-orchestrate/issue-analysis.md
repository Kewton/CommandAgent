# Issue Analysis

## Issue #19: [docs] Overhaul README (EN) and add README.ja.md: quickstart, install, features, demo, badges, license

- 種別: `documentation, enhancement`
- 目的: 現在の `README.md`(173行・英語)は唯一のユーザー入口だが、簡素な2行の説明(`README.md:3-4`)の直後にBuild節(`README.md:11`)へ飛び、最初の実行例は6フラグ+プレースホルダモデルIDのコマンド(`README.md:22`)という構成で、初見者に厳しい。「だれもが使いたくなる」品質のREADMEへ全面改訂し、日本語版 `README.ja.md` を併設する。
- 詳細化要否: `no`

### 受入条件

- **1行の価値提案**(何であるか+なぜ使うか)と簡潔なリード文
- **バッジ**: CIステータス(`.github/workflows/ci.yml` のワークフロー名 `CI` を参照)、ライセンス(MIT)。存在しないもの(crates.io等)のバッジは貼らない
- **機能一覧**: minimal loop / step plan(plan-run)/ ultra plan run / プロファイル(generic・nextjs・python_cli・data)/ マルチプロバイダ(Ollama・Gemini・OpenAI)/ 検証・修復ループ / TUI(フッター・スピナー・割り込み)を箇条書きで
- **デモ**: `--ux-demo` を録画したGIFまたはSVGアニメを `docs/assets/` に置き先頭近くに埋め込む。再現手順(録画コマンド)をアセットと同じ場所にメモとして残す(例: charmbracelet/vhs のtapeファイル or asciinema。ツール選定は実装者に委ねるが再現可能にすること)
- **Quickstart(最小経路)**: Ollamaのみで動く最短手順(Ollamaインストール→モデルpull→`cargo install --path .`→1コマンド実行)。モデルIDは「**手元に実在するモデルに置き換える**」旨を明記し、例は `<your-model>` プレースホルダ表記にする
- **Install**: 前提条件表(Rust 1.88+ / 任意: Ollama、Gemini・OpenAI APIキー、Node.js+npm(インタラクションプローブ用)、Python 3(eval用))と導入手順。`scripts/setup.sh`(別Issue)が入り次第そのリンクを張る前提の構成にする
- **Usage**: 代表的なCLI実行例とREPLの主要スラッシュコマンド抜粋(全リファレンスは docs/guide/ へのリンク。別Issue)
- **Configuration**: preset の存在と設定ファイルの場所のみ簡潔に示し、詳細はガイドへのリンク
- **License 節**(MIT)
- 現READMEの UAT・copy-validation・symlink運用などの**開発者向け内容は README から削除し docs/dev/ へ移す**(docs再編Issueと調整。両Issueを同一PRにしてもよい)
- `README.ja.md` を新設し、英語版と**同一構成・同一情報**の日本語訳とする
- 両ファイルの冒頭に相互リンク(`English | 日本語`)を置く
- 以後の更新で両方を同時に更新する旨をCONTRIBUTING(別Issue)に記載するため、本Issueでは両ファイル冒頭コメントに「対訳ファイルあり・同時更新のこと」の注記を入れる
- 記載するフラグ・コマンド・パスはすべて現行コード(`src/cli.rs`, `src/tui/slash.rs:408-429`, `src/config.rs:663-668`)と突き合わせて正確にする
- 「Anvil does not auto-create them」相当の情報(設定ファイルは自動生成されない)は**正確なので維持**する(`src/config.rs:520-558` に書き込み経路なし)
- `.anvil/` パス表記は現状の実パスのまま書く(リネームは Issue #16/#17 のスコープ。先走らない)
- 製品名表記は Issue #15(ユーザー可視ブランディング)の完了状態に合わせる。**#15完了後の着手を推奨**(未完了なら本Issueで名前だけ先行変更しない)

### 承認済み判断

- None

### 推定影響ファイル

- README.md
- README.ja.md
- .github/workflows/ci.yml
- src/tui/ux_demo.rs
- scripts/setup.sh
- src/cli.rs
- src/providers/gemini.rs
- src/providers/openai.rs
- docs/assets
- docs/guide
- docs/dev
- src/tui/slash.rs
- src/config.rs
- github/workflows/ci.yml
- src/minimal_loop/evidence.rs

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #20: [docs] Add bilingual user guide (docs/guide/ en+ja): CLI, slash commands, configuration, providers, troubleshooting

- 種別: `documentation, enhancement`
- 目的: エンドユーザー向けのリファレンスが存在しない。CLIフラグは**37個中約13個**、スラッシュコマンドは**15個中1個**しかREADMEに記載がなく、設定ファイルのスキーマ・プロバイダ設定・トラブルシューティングもまとまった文書がない。`docs/guide/` 配下に英日対訳のユーザーガイドを新設する。
- 詳細化要否: `no`

### 受入条件

- `cli-reference.md` — **全37フラグ**の表(フラグ / 引数 / 既定値 / 説明 / 関連項目)。既定値・conflicts(例: `--footer` と `--no-footer` は排他, `src/cli.rs:115-128`)は `src/cli.rs` から正確に転記
- `slash-commands.md` — **全15コマンド**(`render_help` の内容が正)+ インラインフラグ + `$(cat <path>)` 展開 + プロファイル自動推論(`src/tui/slash.rs:192-199`)の説明
- `configuration.md` — 設定解決の全体像(CLIフラグ > preset > トップレベルキー > 既定値の優先順位を実コードで確認して記載)、presetの全10キーと**完全性の罠**、トップレベルキー、探索パスと順序、レガシー `.anvil/config`、環境変数(`NO_COLOR`、`ANVIL_NO_FOOTER`/`ANVIL_NO_SPINNER`/`ANVIL_NO_MARKDOWN`/`ANVIL_NO_INTERRUPT` 等のユーザー向けのもの)
- `providers.md` — プロバイダ別セットアップ表(必要なキー・取得先URL・設定方法 env/.env・Ollamaのホスト設定)。**キーの値を画面に出さない**注意書きと `.env` の権限推奨(600)を含める
- `troubleshooting.md` — 最低限: 「`GEMINI_API_KEY is not set`」(`src/config.rs:1117` のエラー文言そのまま見出しに)/「port N is busy」preflight(`src/preflight.rs:95` 周辺の挙動と選択肢)/「interaction probe unavailable」(`src/minimal_loop/interaction_probe.rs:20` の remediation)/ フッター描画乱れ(`--footer off`)/ モデルIDが実在しない場合の失敗の見え方 / Ollama未起動
- `docs/guide/README.md` — ガイドの目次(en/ja両方への入口)。`docs/model-probe.md` へのリンクを含める
- en/ja は**同一構成・同一情報**。見出し構造(h2/h3の数と順序)を一致させる(後続のdoc-driftガードIssueで機械検証する前提)
- 各ファイル冒頭に対訳ファイルへの相互リンク
- 全記載をコードと突き合わせる。とくに既定値(`num_predict`、`max_iterations`、`chat_timeout_secs`、`chat_retries`、`context_budget`)は `src/cli.rs` / `src/config.rs` の実値を確認して記載
- `.anvil/` パス・`ANVIL_*` 変数名は現状のまま記載(リネームは Issue #16/#17。ガイド側は完了後に追従)

### 承認済み判断

- None

### 推定影響ファイル

- src/cli.rs
- docs/model-probe.md
- cli-reference.md
- slash-commands.md
- configuration.md
- providers.md
- troubleshooting.md
- docs/guide/README.md
- src/config.rs
- docs/guide
- src/tui/slash.rs
- src/providers/gemini.rs
- src/providers/openai.rs
- docs/guide/en
- docs/guide/ja
- src/preflight.rs
- src/minimal_loop/interaction_probe.rs
- src/planner/side_effect_paths.rs

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #21: [docs] Reorganize docs/ into guide vs dev with an index; translate data-profile-contract; add benchmarks README

- 種別: `documentation, enhancement`
- 目的: `docs/` 配下12ファイルはほぼ全て開発内部向け・歴史記録(mechanism-ledger、dev-guardrails、generality宣言、perfノート、UAT、移行記録)だが、ユーザー向け文書と区別するインデックスがなく、初見者が `docs/` を開くと内部台帳に迷い込む。また `docs/data-profile-contract.md` は日本語のみで英語話者が読めず、`benchmarks/` はREADMEのない孤立ディレクトリになっている。ディレクトリを再編し、案内を整備する。
- 詳細化要否: `no`

### 受入条件

- `docs/dev/` を新設し、内部向け・歴史記録を移動する: `dev-guardrails.md` `mechanism-ledger.md` `generality.md` `perf-notes.md` `integration-notes.md` `uat-corpus.md` `uat/` `migration/` `profile-manifest.md` `data-profile-contract.md`
- 移動は `git mv` で行い、**内容は一切書き換えない**(歴史記録の改変禁止。とくに `mechanism-ledger.md` と `migration/`)
- `model-probe.md` はユーザー寄りとして `docs/guide/` 側(ユーザーガイドIssueのディレクトリ)へ移すか、ガイドからリンクする(ガイドIssueと調整。同時実装可)
- 移動に伴うパス参照の追従: `rg -n 'docs/' src tests .github *.md docs` で全参照を洗い出して更新する(確認済みの参照: `src/minimal_loop/repair_pressure.rs:231` のコメント。ほかにMarkdown間の相互リンクが `README.md` `SECURITY.md` などにある)
- `docs/README.md` を新設: 全ドキュメントの一覧表(ファイル / 1行説明 / **言語(EN・JA・混在)** / **対象読者(エンドユーザー・コントリビュータ・歴史記録)**)。「歴史記録は現状のコードと一致しないことがある」旨の注意書きを含める
- ルート `README.md` から `docs/README.md` へのリンクを追加(README改訂Issueと調整)
- `docs/dev/data-profile-contract.md`(日本語のみ・凍結v0契約)の**英訳を別ファイル**(例: `data-profile-contract.en.md`)として追加し、相互リンクする。原文は正本として無改変(「翻訳は参考、正本は日本語」の注記を英訳側に入れる)
- `mechanism-ledger.md` / `integration-notes.md` の混在は**翻訳しない**(内部台帳としてJA混在を許容する旨をインデックスに明記)
- `benchmarks/README.md` を新設: `minimal-loop-expanded.yaml` が何のフィクスチャで、`scripts/bench.sh` からどう使われるか(`bench.sh` の引数 `--model` `--runs` `--max-iterations` `--recheck-root` を含む)を簡潔に記載

### 承認済み判断

- None

### 推定影響ファイル

- docs/data-profile-contract.md
- dev-guardrails.md
- mechanism-ledger.md
- generality.md
- perf-notes.md
- integration-notes.md
- uat-corpus.md
- uat/scenarios.md
- profile-manifest.md
- data-profile-contract.md
- model-probe.md
- docs/README.md
- minimal-loop-expanded.yaml
- eval/README.md
- docs/integration-notes.md
- README.md
- SECURITY.md
- docs/dev/data-profile-contract.md
- data-profile-contract.en.md
- benchmarks/README.md
- scripts/bench.sh
- bench.sh
- tests/generality_guardrails.rs
- src/minimal_loop/repair_pressure.rs
- docs/dev
- docs/guide
- src/minimal_loop/loop_run.rs

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #22: [docs] Add doc-drift guard tests: keep CLI flags, slash commands, config keys and EN/JA structure in sync

- 種別: `documentation, enhancement`
- 目的: READMEのフラグ記載が37個中約13個しかない等、ドキュメントとコードの乖離(doc drift)が既に起きている。今後リファレンス(docs/guide/)を整備しても、同期を保つ仕組みがなければ再び陳腐化する。**ドキュメントとコードの同期を機械検証するテスト(doc-drift guard)**を追加する。
- 詳細化要否: `no`

### 受入条件

- clapの `Command` イントロスペクション(`Cli::command().get_arguments()`)で全フラグ名を列挙し、`docs/guide/en/cli-reference.md` に**全フラグが出現する**ことを検証する(方向は「コードにあるものがドキュメントに漏れなく載っている」。ドキュメント側の追加説明は自由)
- 逆方向も検証: ドキュメントの表に載っているフラグ名が実在しないならfail(タイポ・削除済みフラグの検知)。表のパースは「行頭 `| \`--flag\`` 形式」など**単純な規約**を決めてガイド側もそれに従う
- `render_help` の出力からコマンド名(`/xxx`)を抽出し、`docs/guide/en/slash-commands.md` に全て出現することを検証(逆方向も同様)
- 可能なら `render_help` とディスパッチ(`handle_command`)の一致もこの機会にテスト化する(helpに載っているのに処理がない/その逆の検知)
- presetキー10個とトップレベルキーの一覧をテスト内の定数ではなく**コードから導出**できない場合は、`src/config.rs` 側に「サポートするキーの一覧」を返す関数(またはconst)を追加してそれを正とし、ドキュメント出現を検証する
- `docs/guide/en/` と `docs/guide/ja/` の**ファイル集合が一致**することを検証
- 各対訳ペアで**h2/h3見出しの数が一致**することを検証(内容の翻訳品質は対象外。構造の欠落だけ検知)
- 失敗時のメッセージは「どのフラグ/コマンド/キーがどちら側に欠けているか」を列挙し、修正先ファイルパスを示す
- CI(`.github/workflows/ci.yml` の `cargo test --all-targets`)で自動実行される(統合テストとして置けば追加設定不要のはず。要確認)

### 承認済み判断

- None

### 推定影響ファイル

- src/cli.rs
- tests/doc_drift.rs
- docs/guide/en/cli-reference.md
- docs/guide/en/slash-commands.md
- src/config.rs
- .github/workflows/ci.yml
- docs/guide/README.md
- docs/guide
- src/tui/slash.rs
- docs/guide/en
- docs/guide/ja
- github/workflows/ci.yml
- docs/dev-guardrails.md

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #23: [repo] Add LICENSE (MIT), CONTRIBUTING.md, CHANGELOG.md

- 種別: `documentation, enhancement`
- 目的: OSSプロジェクトとしての基本ファイルが不足している。`Cargo.toml:7` は `license = "MIT"` を宣言しているが **LICENSEファイルが存在しない**(宣言と実体の不一致はライセンス上問題)。CONTRIBUTING.md / CHANGELOG.md も無い。「だれもが使いたくなる」リポジトリの土台として整備する。
- 詳細化要否: `no`

### 受入条件

- MIT License の全文を `LICENSE` としてルートに追加する。copyright行は `Copyright (c) 2026 Kewton` とする(リポジトリオーナー名。変更したい場合は本Issueにコメントで指示)
- README(改訂Issueと調整)に License 節を追加しリンクする
- 以下を含む(英語で作成し、要点の日本語併記は任意):
- 開発環境: Rust 1.88+(`Cargo.toml:5`)、テスト実行 `cargo test --all-targets`、PTYテスト `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`、Python 3.10(evalゴールデンテスト、`.github/workflows/ci.yml` 参照)
- **ガードレールの案内**: `docs/dev-guardrails.md`(行数バジェット: baseline+2%でCI失敗。新しいサブシステムは新モジュールへ)、`docs/mechanism-ledger.md` の互換凍結方針(イベント名・JSONキー・スキーマ不変)への言及と、抵触する変更にはledger追記が必要なこと
- corpus回帰(`tests/corpus_regression.rs`)・conformance・generality guardrails がCIで走ること
- ドキュメント対訳(README.md ⇔ README.ja.md、docs/guide/en ⇔ ja)は**同時更新**すること(doc-driftガードで構造検証される)
- PRの期待事項: テスト付き・`RUSTFLAGS="-D warnings"` でwarningゼロ(CI設定と同じ)
- docs再編Issueが先行完了している場合はパスをそれに合わせる(`docs/dev/dev-guardrails.md` 等)
- [Keep a Changelog](https://keepachangelog.com/) 形式で新設。過去の全履歴の掘り起こしはせず、`## [Unreleased]` セクション+「本ファイル開始時点(0.1.0, 2026-07)より前の変更は git 履歴と docs/mechanism-ledger.md を参照」という注記で開始する
- 以後のPRでUnreleasedに追記する運用をCONTRIBUTINGに記載

### 承認済み判断

- None

### 推定影響ファイル

- .github/workflows/ci.yml
- docs/dev-guardrails.md
- docs/mechanism-ledger.md
- tests/corpus_regression.rs
- docs/dev/dev-guardrails.md
- docs/guide/en
- github/workflows/ci.yml
- Cargo.lock

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #24: [setup] Add scripts/setup.sh: prerequisites check, build/install, .env, probe setup, smoke test

- 種別: `enhancement`
- 目的: 現在の導入手順は `cargo build` + 手動symlink(README記載)のみで、前提条件(Rust 1.88+ / Ollama / APIキー / Node+Playwright / Python)が散在しており、初回セットアップの失敗ポイントが多い。**冪等なセットアップスクリプト `scripts/setup.sh`** を新設し、「クローン→1コマンド→動く」を実現する。
- 詳細化要否: `no`

### 受入条件

- 引数なし = 対話モード(各ステップで確認)。`--yes` = 非対話(安全な既定で全実行)。`--check-only` = 前提チェックのみ実行して結果表示(何も変更しない)
- **冪等**: 2回目以降の実行で壊れない・重複作業しない(導入済みならスキップと表示)
- **前提チェック**: `cargo`/`rustc`(バージョンが `Cargo.toml` の `rust-version` 以上か。値はCargo.tomlからgrepで取得しハードコードしない)、`git`。任意依存として `node`/`npm`(プローブ用)、`ollama` または既定ホストへの疎通(`curl http://localhost:11434/api/tags`)、`python3`(eval用)。必須欠落は導入方法のURL付きで案内して終了、任意欠落は警告して続行
- **ビルド/インストール**: `cargo install --path . --locked` を提案・実行(拒否時は `cargo build --release` +PATH追加案内にフォールバック)。完了後 `commandagent --version` で確認
- **APIキー設定(任意)**: Gemini/OpenAIを使うか尋ね、使うなら `.env` に `GEMINI_API_KEY=` / `OPENAI_API_KEY=` を書き込む。**入力はエコーしない**(`read -s`)。**既存の `.env` は上書きせず**、欠けているキーの追記のみ。作成時はパーミッション600。`--yes` モードではキー入力はスキップし追記もしない(案内のみ)
- **Ollamaモデル(任意)**: `ollama` 検出時、`ollama list` を表示し、モデルが無ければ pull を提案(モデル名はユーザー入力。既定値を押し付けない)
- **インタラクションプローブ(任意)**: `node`/`npm` 検出時、`commandagent --setup-interaction-probe` の実行を提案・実行
- **スモーク確認**: `commandagent --version` の出力を表示し、Ollama疎通が取れていれば `--model-probe` の実行を提案(時間がかかる旨を表示)
- **サマリー**: 各ステップの結果(ok / skipped / warn)を最後に一覧表示し、次の一歩(READMEのQuickstartへのリンク)を出す
- `set -euo pipefail`。bash 3.2互換(macOS標準)で動くこと(配列の使い方等に注意)
- macOS / Linux 両対応(OS分岐は最小限に)
- 失敗時は必ず「何が失敗し、手動でどうすればよいか」を1行で出す
- シークレット(キーの値)を画面・ログに一切出さない
- `shellcheck scripts/setup.sh` が警告ゼロ。CI(`.github/workflows/ci.yml`)にshellcheckステップを追加(`scripts/*.sh` 対象)
- README(改訂Issueと調整)のInstall節から `./scripts/setup.sh` を案内する1行を追加

### 承認済み判断

- None

### 推定影響ファイル

- scripts/setup.sh
- bench.sh
- eval-run.py
- Cargo.toml
- .github/workflows/ci.yml
- scripts/*.sh
- ./scripts/setup.sh
- install.sh
- src/cli.rs
- src/config.rs
- src/minimal_loop/interaction_probe.rs
- github/workflows/ci.yml
- README.md

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #25: [setup] Add commandagent --doctor: built-in environment diagnosis

- 種別: `enhancement`
- 目的: 環境診断の部品(preflight・プローブ可用性・model-probe・`/status` readinessカード)は既に揃っているが、束ねる入口がない。**`commandagent --doctor`(および `/doctor`)** として環境診断コマンドをバイナリに内蔵し、「動かない」時の一次切り分けを1コマンドにする。`scripts/setup.sh`(別Issue)より堅牢でクロスプラットフォームな中期解。
- 詳細化要否: `no`

### 受入条件

- CLIフラグ `--doctor`(`src/cli.rs` に追加、既存Action系フラグの流儀に従う)とREPLの `/doctor`(`render_help` にも追加)の両方から実行できる
- **設定解決**: 有効な model / provider / planner_model / planner_provider / profile と、その出所(CLI / preset / config / default。既存の `field_sources` を利用)
- **設定ファイル**: 探索した各パス(`.anvil/config.toml` 等、`src/config.rs:663-668`)の存在・パース可否。presetが指定されているのに**不完全で解決不能**な場合はどのキーが欠けているかを列挙(`preset_complete`, `src/config.rs:560-571` を流用)
- **プロバイダ疎通**:
- Ollama(providerまたはplanner_providerがollamaのとき): 設定ホストへ `/api/tags` を短タイムアウトで照会し、到達性と**設定中のモデルがtagsに存在するか**を確認
- Gemini / OpenAI: **キーの存在のみ**確認(env → `.env` の解決順で。値は `redact` でマスクし絶対に表示しない)。実APIへのリクエストは行わない(課金・レート考慮)
- **インタラクションプローブ**: `playwright_availability` の結果と、unavailable時は既存remediation文言
- **状態ディレクトリ**: state_dir の書き込み可否(実際に一時ファイルを作って消す)
- **端末**: TTYか、色が有効か(`NO_COLOR`)、端末幅、フッター無効化条件に該当していないか
- **ワークスペース**: workspace_root の書き込み可否、`.env` の有無(有ればどのキーが定義済みかを**キー名のみ**表示)
- 人間可読の整列されたチェックリスト(✓/!/✗)。1項目1行+失敗時のみ対処行
- いずれかが fail なら終了コード非0、warnのみなら0
- `--doctor --json` で機械可読JSON(キー名は新規設計でよいが、一度出したら安定させる前提で命名する)
- 破壊的操作・自動修復はしない(診断のみ。修復はsetup.shや既存remediationへ誘導)
- 実行時間は正常系で数秒以内(ネットワーク照会は全て短タイムアウト)

### 承認済み判断

- None

### 推定影響ファイル

- scripts/setup.sh
- docs/model-probe.md
- src/cli.rs
- .anvil/config.toml
- src/doctor.rs
- src/lib.rs
- docs/dev-guardrails.md
- runner.rs
- loop_run.rs
- docs/mechanism-ledger.md
- setup.sh
- src/preflight.rs
- src/minimal_loop/interaction_probe.rs
- src/tui/banner.rs
- src/tui/slash.rs
- src/config.rs
- anvil/config.toml
- README.md

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #26: [setup] Release & distribution: tagged binary releases, install.sh, crates.io/Homebrew (decisions included)

- 種別: `enhancement`
- 目的: 配布手段が「ソースからの `cargo build`」しかない。リリースワークフロー無し(CIはテストのみ)、リリースバイナリ無し、crates.io未公開、Homebrew無し。`cargo install` すら README に書かれていない。バイナリ配布を整備し、Rustツールチェーン無しでも導入できるようにする。
- 詳細化要否: `no`

### 受入条件

- `.github/workflows/release.yml` を新設: `v*` タグのpushで発火し、以下のターゲットのリリースバイナリをビルドして GitHub Release に添付する
- `aarch64-apple-darwin` / `x86_64-apple-darwin`
- `x86_64-unknown-linux-gnu`(可能なら `-musl` も。glibc依存を減らせる)
- 各アーティファクトは `commandagent-<version>-<target>.tar.gz` 形式+ `sha256` チェックサムファイルを添付
- ビルドは `--release --locked`。`cargo test` をリリースビルド前に実行(壊れたタグを出さない)
- リリースノートはタグメッセージ+自動生成(GitHubの release notes 自動生成で可。CHANGELOG連携はCHANGELOG Issue完了後に検討)
- 実装手段は素のGitHub Actionsで可。`cargo-dist` を採用する場合は生成物をコミットし、選定理由をPRに記載
- `scripts/install.sh` を新設: 最新(または指定バージョン)のGitHub Releaseから、実行環境のOS/アーキテクチャに合うバイナリを取得し、チェックサム検証のうえ `~/.local/bin`(または `--prefix` 指定先)へ配置する
- `curl -fsSL https://raw.githubusercontent.com/Kewton/CommandAgent/main/scripts/install.sh | sh` で動く想定の作りにする(ただしREADMEにはパイプ実行のリスク注記と「ダウンロードして確認してから実行」の選択肢も書く)
- `shellcheck` 警告ゼロ。PATH未設定時は案内を出す
- ソースビルド版 `scripts/setup.sh`(別Issue)との違い(バイナリ取得 vs ソースビルド+環境セットアップ)をREADMEで明確にする
- 検討事項をコメントで提示して判断を仰ぐ: パッケージ名 `commandagent` の空き状況、公開に必要なメタデータ追加(`repository`/`readme`/`keywords`)、`workspace/` や `tests/corpus/` など巨大ディレクトリの `exclude` 設定、公開後のyank不可性
- 承認された場合のみ: メタデータ整備 + `cargo publish --dry-run` 通過 + 公開手順のドキュメント化
- リリースバイナリ運用が安定した後の選択肢として、`Kewton/homebrew-tap` リポジトリ+formulaの構成案を提示して判断を仰ぐ(本体リポジトリ側の変更はほぼ不要)
- Apply approved decision: Prepare crates.io metadata and pass cargo publish --dry-run without publishing; document a Homebrew tap proposal without creating an external repository; create an unused prerelease tag and GitHub prerelease for release UAT and retain them as evidence.

### 承認済み判断

- Prepare crates.io metadata and pass cargo publish --dry-run without publishing; document a Homebrew tap proposal without creating an external repository; create an unused prerelease tag and GitHub prerelease for release UAT and retain them as evidence.

### 推定影響ファイル

- ci.yml
- Cargo.toml
- build.rs
- .github/workflows/release.yml
- scripts/install.sh
- scripts/setup.sh
- install.sh
- tests/corpus
- tests/tui_pty.rs
- github/workflows/release.yml
- README.md

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #27: [setup] Shell completions (clap_complete) and man page generation

- 種別: `enhancement`
- 目的: シェル補完(bash/zsh/fish)とmanページが無く、37個のCLIフラグを覚える手段がヘルプしかない。clap公式の生成機構で補完とmanページを提供する。
- 詳細化要否: `no`

### 受入条件

- `commandagent --completions <shell>`(bash / zsh / fish、可能なら powershell も)で補完スクリプトを**stdoutに出力**する(ファイル書き込みはしない。導入はユーザー/setup.shに委ねる)
- `clap_complete` を使用し、フラグ追加時に自動追従する構成にする(手書き補完スクリプト禁止)
- `--help` に載ること、および誤った shell 名で分かりやすいエラーになること
- `clap_mangen` で `commandagent.1` を生成する手段を提供する。方式は次のいずれか(実装者が選び、理由をPRに記載):
- (a) `--generate-man` フラグでstdout出力
- (b) リリースワークフロー(配布Issue)内で生成しアーティファクトに同梱
- 生成物をリポジトリにコミットしない(常に定義から生成)
- `scripts/setup.sh`(別Issue)が存在すれば、補完スクリプトの導入ステップ(zsh: `fpath`、bash: `bash_completion.d` 等への配置提案)を追加する。setup.sh未マージなら本IssueではREADME/ガイドへの手動導入手順記載のみでよい
- docs/guide/ の cli-reference(ドキュメントIssue)に補完・manの導入節を追加(未マージならREADMEに簡潔に)

### 承認済み判断

- None

### 推定影響ファイル

- Cargo.toml
- src/cli.rs
- scripts/setup.sh
- src/config.rs
- docs/guide
- README.md
- docs/codex-harness.md
- docs/generality.md

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #28: [dev] Add justfile and devcontainer for reproducible development tasks

- 種別: `enhancement`
- 目的: 開発者向けの定型タスク(テスト・PTYテスト・corpus回帰・eval・ベンチ)がREADMEやCI定義に散在しており、コントリビュータが「何をどう実行すべきか」を毎回探す必要がある。**justfile**(タスクランナー定義)と**devcontainer**(再現可能な開発環境)を整備する。
- 詳細化要否: `no`

### 受入条件

- ルートに `justfile` を新設し、最低限以下のレシピを定義する(**CIと同一のコマンド・フラグ**にすること。乖離させない):
- `build` / `build-release`
- `test`(= `RUSTFLAGS="-D warnings" cargo test --all-targets`)
- `test-corpus` / `test-guardrails` / `test-conformance`(CIの各ステップと同一)
- `test-pty`(= `ANVIL_PTY_TESTS=1 cargo test --test tui_pty`)
- `test-eval`(= CIのpython unittest 3本)
- `ci`(上記CI相当を一括実行)
- `bench`(`scripts/bench.sh` への薄いラッパー、引数透過)
- `run`(開発用の代表的な起動例。モデル等は環境変数で上書き可能に)
- `just --list` の説明文(`# comment`)を全レシピに付ける
- `just` 未導入者のために、CONTRIBUTING(または README のDevelopment節)へ導入1行と「justはオプション、生コマンドはCI定義参照」の注記を追加
- `.devcontainer/devcontainer.json`(+必要なら Dockerfile)を新設: Rust(`Cargo.toml` の rust-version 以上のstable)、Node.js LTS(インタラクションプローブ用)、Python 3.10+(eval用)、`just`、`shellcheck` を含む
- コンテナ内で `just ci` が通ること(corpus・conformance含む。ネットワーク不要のテストのみで完結することを確認)
- **Ollamaはコンテナに含めない**。ホスト側Ollamaへの接続方法(`--ollama-host http://host.docker.internal:11434` 等)をdevcontainer READMEコメントに記載
- イメージサイズと初回ビルド時間を常識的な範囲に(公式イメージ+featuresの組み合わせを優先し、カスタムDockerfileは必要時のみ)

### 承認済み判断

- None

### 推定影響ファイル

- .github/workflows/ci.yml
- tests/live_provider.rs
- scripts/bench.sh
- scripts/eval-run.py
- .devcontainer/devcontainer.json
- Cargo.toml
- tests/eval/test_
- github/workflows/ci.yml
- devcontainer/devcontainer.json
- src/planner/profiles/data/manifest.toml

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #29: Tracking: documentation modernization (EN/JA) & setup/install tooling

- 種別: `documentation, enhancement`
- 目的: developブランチの機能改善 第2弾として、(1) ドキュメント(README / docs/)の最新化・拡充・洗練と英日対応、(2) セットアップ(インストール)体験の拡充 — の2テーマを調査し、実装単位ごとにIssue化した。本Issueは進行管理用。UX/ブランディングのトラッキング #18 と対をなす。
- 詳細化要否: `yes`

### 受入条件

- None

### 承認済み判断

- None

### 推定影響ファイル

- README.md
- data-profile-contract.md
- Cargo.toml
- docs/dev-guardrails.md
- docs/mechanism-ledger.md
- scripts/setup.sh
- docs/guide
- scripts/eval_lib/acceptance_contract.py

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- 受入条件が明確ではありません。期待する完了条件を1-3点で補足してください。

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
