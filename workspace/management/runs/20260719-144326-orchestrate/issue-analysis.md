# Issue Analysis

## Issue #11: [ux] Extend terminal markdown renderer: tables, nested lists, links, code highlighting

- 種別: `enhancement`
- 目的: REPLのアシスタント出力は自前の最小Markdownレンダラー(`src/tui/markdown.rs`)で描画されるが、表現力が最先端CLIに比べ大きく不足している(表・ネストリスト・リンク・言語別ハイライト非対応)。レンダラーを拡張し、モデル出力の可読性を引き上げる。
- 詳細化要否: `no`

### 受入条件

- `| a | b |` + 区切り行の形式を検出し、列幅を揃えて描画する(桁揃えは表示幅ベース。CJK等の全角文字幅を考慮すること)。
- アライメント指定(`:---`, `:---:`, `---:`)を反映する。
- 不正な表(列数不一致など)はクラッシュせずプレーンテキストとして出力する。
- 端末幅を超える表は、はみ出してもエスケープ列が壊れない(折り返しは端末任せで可)。
- 2レベル以上のネスト(インデント2spまたは4sp)を字下げ+異なるマーカーで描画する。
- 番号付きリスト(`1. `)に対応する。
- `[text](url)` を `text (url)` 形式で描画する(最低ライン)。TTYかつ色有効時のみOSC 8ハイパーリンクにするのは任意(実装する場合は非対応端末での劣化を確認)。
- fenced code の言語タグ(`js/ts/tsx/python/rust/bash/json` 程度)を見て、キーワード/文字列/コメントの3種を色分けする**軽量**なハイライトを実装する。
- 新規依存クレートの追加は原則不可。追加する場合は理由・サイズ・`default-features` 最小化をPRで説明すること(自前実装を推奨)。
- 未知の言語タグ・タグなしは現行どおり単色。
- `strip_think` / `sanitize` / 64KiB上限 / `NO_COLOR` / `ANVIL_NO_MARKDOWN` ゲートの既存挙動を一切変えない(既存ユニットテストがそのまま通ること)。
- コンテンツ由来の文字列からSGR/エスケープ注入が起きないこと(sanitize後に装飾を適用する順序を維持)。

### 推定影響ファイル

- src/tui/markdown.rs
- src/tui/markdown/table.rs
- docs/dev-guardrails.md
- src/tui/footer.rs
- src/tui/terminal.rs
- src/completion_metadata/data.rs
- src/completion_metadata/intent.rs
- src/runs.rs

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #12: [ux] Stream assistant output token-by-token in the REPL

- 種別: `enhancement`
- 目的: 現在、アシスタント応答は**完了後に一括表示**され、待機中はスピナーと固定フッターのみが動く。最先端CLI(Claude Code / Codex CLI / Gemini CLI)との最大の体感差はここにある。プロバイダ応答をトークン単位でストリーミングし、逐次レンダリングする。
- 詳細化要否: `no`

### 受入条件

- Ollama(NDJSON stream)/ OpenAI(SSE)/ Gemini(`streamGenerateContent` SSE)の3プロバイダでストリーミング受信を実装する。
- 3実装が共通のインクリメンタルAPI(チャンクコールバック or イテレータ)を実装し、呼び出し側はプロバイダ非依存になる。
- **非ストリーミング経路は残す**。設定(例: `stream = on|off`。CLIフラグ+configファイル+既存の設定優先順位規約に従う)で切替可能。既定はREPLの対話応答でON。非TTY・`--prompt` 一括実行・テスト用フェイククライアント経路は従来どおり非ストリーミングで動くこと。
- async runtime を導入しない。blocking `reqwest` のレスポンスを逐次読みし(`Read`)、SSE/NDJSONの行分割を自前で行う。**UTF-8マルチバイトがチャンク境界で分割されるケース**を正しく扱う。
- `chat_timeout_secs` の意味を維持または明確化する(推奨: ストリーム全体のwall-clock上限として適用し、挙動をドキュメント化)。リトライ(`chat_retries`)は「最初のトークン受信前の失敗」のみ対象とする。
- 受信チャンクを `TerminalMarkdownRenderer` に逐次投入して描画する。**同一内容を非ストリーミングで一括描画した場合と最終出力が一致**すること(受け入れテスト化)。
- `<think>` ブロックはストリーミング中も表示されない(既存 `strip_think` のチャンク境界対応を利用)。
- スピナーは「最初のトークン受信まで」に役割を縮小し、最初のトークン到達時に確実に消去してから本文描画を始める(スピナー行の残骸が出ないこと)。
- 固定フッターとの共存: ストリーミング描画中もフッターが壊れない(既存のfreeze/pauseガードの範囲で整合させる。スクロール領域の外に本文が書かれること)。
- ストリーミング中の Esc/Ctrl+C でチャンク境界チェックにより速やかに中断し、**それまでの部分出力はスクロールバックに残す**。端末状態(rawモード/スクロール領域)は正しく復元。中断時の終端レコード(`interrupted` / `aborted_by_user`)の既存挙動(`tests/tui_integration.rs:759` 以降)を壊さない。
- 接続断・不正なSSE/NDJSONは、部分出力+明確なエラーメッセージで終了しパニックしない。
- ツールコールXMLフォールバック(`src/providers/xml_fallback.rs`)は**累積テキスト全体**に対して従来どおり機能する(ストリーミングは表示のみの変更であり、パース入力は完成テキスト)。
- セッション保存・events.jsonl・eval関連の記録内容は非ストリーミング時と同一(表示方式の変更がデータに漏れない)。

### 推定影響ファイル

- src/providers/openai.rs
- src/providers/gemini.rs
- Cargo.toml
- src/tui/spinner.rs
- src/tui/footer.rs
- src/tui/interrupt.rs
- src/providers/xml_fallback.rs
- src/providers/streaming.rs
- src/planner/runner.rs
- src/minimal_loop/loop_run.rs
- docs/dev-guardrails.md
- src/providers/ollama.rs
- src/tui/repl.rs
- src/tui/slash.rs
- src/tui/mod.rs
- src/tui/markdown.rs
- tests/tui_integration.rs

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #13: [ux] Handle terminal resize for the fixed footer during long runs

- 種別: `enhancement`
- 目的: 固定フッター(画面最下部の進捗バー)は起動時に一度だけ端末サイズを取得しており、長時間実行中に端末をリサイズするとスクロール領域が古いまま残り、フッターの位置ずれ・描画残骸が発生する。リサイズに追従させる。
- 詳細化要否: `no`

### 受入条件

- `render_loop` の各tickで `terminal::size()` を再取得し、サイズ変化を検出したら (1) 旧スクロール領域を解除、(2) 新サイズで領域とフッター行数を再確立、(3) フッター内容を新幅で再フィットして描画する。
- 縮小時・拡大時とも、旧フッターの描画残骸(ゴミ行)が本文スクロールバックに残らない。
- 幅が100列しきい値を跨いだ場合、1行⇔2行のフッター行数が正しく切り替わる。
- 高さ縮小で本文カーソルがフッター領域に食い込まない。
- リサイズ後もシャットダウン時(通常終了・割り込み・パニック)のスクロール領域復元が正しく行われる。
- サイズ取得失敗時(`terminal::size()` がErr)は直前のジオメトリを維持し、クラッシュしない。
- フッター無効時・非TTY時の挙動は不変。

### 推定影響ファイル

- src/tui/footer.rs
- src/tui/banner.rs
- tests/tui_pty.rs
- src/lib.rs
- src/tui/interrupt.rs
- src/planner/runner.rs
- src/tui/editor.rs
- src/completion_metadata/data.rs

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #14: [ux] Accept and queue user input while a command is running

- 種別: `enhancement`
- 目的: コマンド実行中、ユーザーのキー入力は Esc/Ctrl+C(割り込み)以外**すべて破棄**される。Claude Code のような「実行中に次の指示を打っておき、完了後に順次処理される」体験を実現する。
- 詳細化要否: `no`

### 受入条件

- 実装前にPR説明(または本Issueコメント)に短い設計ノートを書く: キーイベントの所有権(割り込み監視スレッドとの統合方法)、表示位置(フッター直上 or フッター内のペンディング行)、Escセマンティクスの整理。
- 実行中に印字可能キーを打つと、ペンディング入力バッファに蓄積され、画面上(推奨: フッターの上または2行フッターの1行を転用)にエコー表示される。Backspaceで編集可能。
- Enter でペンディング行がキューに積まれ、`queued: <先頭40文字>…` のような形で確認表示される。複数行キュー可。
- 実行完了後、キューされた行が入力された順に通常のREPL入力として処理される(履歴にも記録)。処理前に各行が何であるかが表示される。
- 割り込みキーのセマンティクス変更は次のとおり: **Ctrl+C は従来どおり常に中断要求**。Esc はペンディングバッファが非空なら「バッファクリア」、空なら従来どおり中断要求。この差はヘルプ(`/help`)とフッター表示で分かるようにする。
- キュー内容はメモリのみ(プロセス終了で消えてよい)。上限(例: 10行、各4KiB)を設け、超過時は明示的に拒否メッセージを出す。
- `ANVIL_NO_INTERRUPT` 設定時(`src/tui/interrupt.rs:28`)は本機能も無効(従来どおり)。
- 非TTY・フッター無効時も安全に無効化される。
- ペンディング行のエコーが、スピナー・フッター・本文出力(ストリーミング導入後はトークン流)と衝突して画面が壊れないこと。既存のfreeze/pauseガードの枠内で実装する。

### 推定影響ファイル

- src/tui/interrupt.rs
- src/tui/input_queue.rs
- docs/dev-guardrails.md
- tests/tui_integration.rs
- src/tui/repl.rs
- src/tui/mod.rs
- src/planner/assurance.rs
- src/planner/final_acceptance.rs

### 参考情報

- None

### テスト期待値

- cargo test

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #15: [brand] Phase 1: Replace remaining user-visible Anvil branding (banner art, REPL prompt, planner persona, docs)

- 種別: `enhancement`
- 目的: 本リポジトリは Anvil(`anvilminimal`)からの移行で作られており、crate/binary名等のリネームは完了済み(`d05a410`, `835c04f`。互換方針は `docs/mechanism-ledger.md` 末尾の記録を参照)。しかし**ユーザーの目に直接触れるブランディング**にまだ "Anvil" が残っている。本Issueは**動作変更ゼロ**の純粋なブランディング置換(Phase 1)を行う。
- 詳細化要否: `no`

### 受入条件

- REPL起動時の画面(バナー+プロンプト)に "Anvil"/"anvil" が一切表示されない。
- `rg -in 'anvil' src/tui src/repl.rs README.md docs/generality.md docs/perf-notes.md` のヒットが「環境変数 `ANVIL_*`」「`.anvil/` パス」のみになる(製品名としてのAnvilが残らない)。
- `cargo build && cargo test --quiet` 全通過、`ANVIL_PTY_TESTS=1 cargo test --test tui_pty` 通過。
- 動作変更ゼロ(文字列以外の差分がないこと)。

### 推定影響ファイル

- docs/mechanism-ledger.md
- tests/corpus_regression.rs
- README.md
- .anvil/config.toml
- docs/generality.md
- docs/perf-notes.md
- docs/uat-corpus.md
- docs/uat/scenarios.md
- eval/README.md
- src/tui/banner.rs
- src/tui/repl.rs
- tests/tui_pty.rs
- src/planner/runner.rs
- src/minimal_loop/interaction_probe.rs
- docs/migration
- workspace/management/runs
- src/tui
- src/repl.rs
- anvil/config.toml
- src/time_profile.rs

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #16: [brand] Phase 2: Migrate ANVIL_* env vars and .anvil config paths to COMMANDAGENT_* with compatibility shims

- 種別: `enhancement`
- 目的: Anvil→CommandAgent リネームのPhase 2として、**機能的な外部インターフェース**である環境変数 `ANVIL_*` と設定ファイルパス `.anvil/config(.toml)` を、後方互換を保ったまま `COMMANDAGENT_*` / `.commandagent/` へ移行する。
- 詳細化要否: `no`

### 受入条件

- 新旧環境変数のマトリクステスト(新のみ/旧のみ/両方/どちらも無し)が env_compat ヘルパーに対して存在し通過する。旧のみ時の警告が1回だけ出ることもテストする。
- 設定パス優先順位のテスト(新のみ/旧のみ/両方)が `src/config.rs` に追加され通過する。
- `rg -n 'ANVIL_' src build.rs scripts tests eval docs README.md` のヒットが「env_compat のフォールバック定義」と「フォールバック動作を検証するテスト」のみになる。
- `cargo build && cargo test --quiet` 全通過。`docs/mechanism-ledger.md` に本Phaseの記録を追記。

### 推定影響ファイル

- src/env_compat.rs
- docs/dev-guardrails.md
- src/tui/terminal.rs
- tests/tui_pty.rs
- tests/live_provider.rs
- eval/README.md
- docs/uat/scenarios.md
- .github/workflows/ci.yml
- .anvil/config.toml
- ~/.anvil/config.toml
- README.md
- docs/mechanism-ledger.md
- src/config.rs
- src/tui/footer.rs
- src/tui/interrupt.rs
- src/tui/markdown.rs
- src/tui/spinner.rs
- src/eval_events.rs
- src/minimal_loop/completion.rs
- src/planner/runner.rs
- src/minimal_loop/interaction_probe.rs
- src/minimal_loop/loop_run.rs
- src/tui/ux_demo.rs
- src/build_info.rs
- scripts/bench.sh
- src/state.rs
- src/tui/status.rs
- github/workflows/ci.yml
- anvil/config.toml
- src/time_profile.rs

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #17: [brand] Phase 3 (decision): internal protocol identifiers still named anvil (data-anvil-*, .anvil metadata, anvil_tool_call, anvil_app)

- 種別: `enhancement, question`
- 目的: Anvil→CommandAgent リネームのPhase 3。**LLMとの動作契約・機械可読データに埋め込まれた "anvil" 識別子**をどう扱うかの**方針判断Issue**。実装前に本Issueでオプションを選択すること(実装Issueは判断後に別途切る)。
- 詳細化要否: `yes`

### 受入条件

- None

### 推定影響ファイル

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
- src/time_profile.rs

### 参考情報

- None

### テスト期待値

- None

### ユーザーへの質問

- 受入条件が明確ではありません。期待する完了条件を1-3点で補足してください。

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
