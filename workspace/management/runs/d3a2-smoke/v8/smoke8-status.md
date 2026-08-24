# D-3a-2 workflow circle smoke v8 — 対応整理

作成日: 2026-07-22
対象: `commandagent --workflow workflows/recovery-circle-data.yaml`
origin: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke8_origin`

---

## 0. TL;DR

- **直したもの**: workflow ノード実行が**永久ハング**していた再入デッドロック（パニックバウンダリ）。→ 修正・検証・コミット・push 完了（`38db36e`, branch `develop`）。
- **今も残る事象**: デッドロックは解消したが、investigate ノードの LLM エージェント（ローカル 27B）が**成果物を書かず空回り**し、停滞検知で失敗 → circle は `circle_failed` で終端。これは配線バグではなくモデル品質/プロンプトの課題。

---

## 1. 今回やったこと

1. **Phase 1（存在確認）実施** — origin 実在、`run_stop` イベント（`.anvil/runs/019f6951-…/events.jsonl`）、recovery YAML 2本を確認。すべて run sheet 記載どおり pass。
2. **Phase 2 一次失敗の対処** — PATH 上の `commandagent`（`~/.local/bin`、旧ビルド）が `--workflow` 非対応で即エラー。旧版を `commandagent.pre-smoke8.bak` にバックアップし、workflow 対応ビルドへ差し替え。
3. **原因調査** — 4回の過去実行がすべて `workflow_node_run_created` 直後で停止していることを確認。当初「LLM ループのハング」と誤診 → ユーザー指摘（GPU 未使用）を受け再調査。
4. **ハングプロセスのスタック採取** — 6時間ハング中の実プロセス（PID 37802）を `sample` 採取し、`__psynch_mutexwait`（ミューテックス自己デッドロック）を特定。
5. **根本原因の確定とコード修正** — `src/lib.rs` の `run_resolved_config_for_workflow` を修正（下記）。
6. **環境クリーンアップ** — デッドロックプロセス（PID 37802 / 親 37799）を停止。
7. **リビルド・再インストール・実走検証** — release を再ビルド（17.8s, exit 0）、PATH へ再インストール、smoke8 を実走。
8. **コミット & push** — `38db36e "Fix workflow node re-entrancy deadlock in panic boundary"` を `origin/develop` へ push。

---

## 2. 根本原因（確定）— パニックバウンダリの再入デッドロック

### メカニズム
1. 外側 `run()` → `run_resolved_config` → `catch_cli_run` → `PanicHookGuard::install` が、
   プロセス全体で共有される**非再入ミューテックス** `PANIC_HOOK_LOCK`（`src/cli_panic_boundary.rs:11`）を
   ロックし、**run 全体の間ロックを保持し続ける**。
2. investigate ノードは `execute_node`（`src/workflow/runner.rs:190`）→
   `run_resolved_config_for_workflow`（`src/lib.rs:63`）で、**もう一度**
   `run_resolved_config → catch_cli_run → PanicHookGuard::install` を呼ぶ。
3. 同一スレッドが既に保持している同じミューテックスを再ロックしようとして**自己デッドロック**。
   `std::sync::Mutex` は再入不可のため `__psynch_mutexwait` で永久待機。

### 証拠
- ハングプロセス（PID 37802）のスタック末尾:
  `run_resolved_config_for_workflow → run_resolved_config → catch_cli_run → catch_with_context → PanicHookGuard::install → Mutex::lock → __psynch_mutexwait`
- LLM 到達前で停止するため **GPU 未使用**（ユーザー観察と一致）。
- child run の events は `workflow_node_run_created` の1行のみ。`run_stop` も `workflow_adjudicated` も無し。
- `intent_resolved` だけ残るのは、`execute_node` が LLM 実行の**直前**にそれを書くため（`runner.rs:201`）。

### 修正（`src/lib.rs`, commit `38db36e`）
```rust
pub(crate) fn run_resolved_config_for_workflow(config: Config) -> anyhow::Result<()> {
    // 既に外側のパニックバウンダリ内。run_resolved_config を再入すると
    // 同一スレッドが PANIC_HOOK_LOCK を再ロックして自己デッドロックするため、
    // run_config を直接呼ぶ。子ノードの panic は外側バウンダリが引き続き捕捉する。
    run_config(config)   // was: run_resolved_config(config)
}
```

---

## 3. 検証結果（実走 smoke8）

| 項目 | 修正前 | 修正後 |
|---|---|---|
| 挙動 | `workflow_node_run_created` 直後に永久ハング | 実エージェント走行 → `workflow_adjudicated` 到達（346秒 / exit 0） |
| GPU | 未使用 | `qwen3.6:27b-coding-nvfp4 … 100% GPU`（実推論） |
| 終端 | 無し | `workflow_adjudicated` / verdict=`circle_failed` |

- child run `019f854b-…` は複数モデルターン＋ツール実行（Read/Glob/Grep/Bash）を実行。
- run sheet Phase 2 の合格条件「verdict によらず**正直な終端に到達**」を満たす → **smoke の主目的は達成**。

---

## 4. 現在発生している事象（残課題）— investigate ノードの停滞

### 事象
実走の investigate child run（`019f854b-…/events.jsonl`）:
```json
{"event":"run_stop",
 "reason":"model_stagnation:no_progress_recorded: objective: 『起点run』の実行が失敗しました。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。",
 "verdict":"failed"}
```
→ orchestrator が `node_failed:investigate` → route `investigate→fix`（`on: full` 要件）に乗れず `circle_failed` で短絡終了。

### 仕組み（停滞検知）
- ループは「**ファイル書込/編集＝進捗、読むだけ＝進捗なし**」で `no_progress_streak` を加算（`src/minimal_loop/repair_pressure.rs`）。
- しきい値: `NO_PROGRESS_FEEDBACK_LIMIT = 3`, `WRITE_REQUIRED_NO_WRITE_LIMIT = 2`。
- 読むだけのターンが続くと「書け」と圧力を注入し、それでも書かないと空回りと判定して run 失敗。

### 実際の挙動（ログ）
- 15＋ターン、`Read pipeline/main.py` / `Glob **/*` / `Grep 起点` を反復するだけで、
  **必須成果物 `output/diagnosis.md` を一度も書かなかった** → 停滞検知が作動。
- 原因はローカル 27B モデルが「探索 → 成果物執筆」へ踏み出せず空回りしたこと。
  **配線バグではなくモデル品質/プロンプト設計の問題**。

---

## 5. 問題点の整理

| # | 分類 | 内容 | 状態 |
|---|---|---|---|
| P1 | 配線バグ | workflow ノード子実行の再入デッドロック（永久ハング・GPU 未使用） | **解決済み**（`38db36e`） |
| P2 | 運用 | PATH の `commandagent` が旧ビルドで `--workflow` 非対応だった | 暫定対処済み（下記 C3） |
| P3 | エージェント品質 | investigate ノードが成果物を書かず停滞 → `circle_failed` | **未解決**（別課題） |
| P4 | 環境 | 過去の smoke2 実行が6時間デッドロック残存していた | 解消済み（プロセス停止） |

---

## 6. 課題 / 次のアクション

- **C1（推奨・優先）**: investigate の停滞対策。まず **プロンプト強化**（`src/workflow/runner.rs:80` の investigate ゴール文に「まず `output/diagnosis.md` を作成してから調査を深める」旨を追加し早期執筆を促す）→ 効果不足なら **能力の高いモデル**を `--model` 指定。
- **C2**: 停滞しきい値の見直し（調査系タスクは読込先行になりがち）。ただし無限ループ検知を弱めるトレードオフに注意。
- **C3**: PATH バイナリ運用の恒久化。`~/.local/bin/commandagent` は現在修正版 release に差し替え済み（バックアップ `commandagent.pre-smoke8.bak`）。ビルド→インストールの導線を smoke 手順に明記するか、`cargo install --path .` を標準化。
- **C4**: circle 完走（`investigate → fix → verify_origin → circle_full`）の確認。C1 でモデル/プロンプトを整えた上で再走し、`workflow_adjudicated: circle_full` を取得。
- **C5**: 証跡の正式記録（既存 `v5-harvest.md` / `v7.md` と同様に smoke8 結果を残すか判断）。

---

## 7. 参照

- 修正コミット: `38db36e Fix workflow node re-entrancy deadlock in panic boundary`（`origin/develop`）
- run sheet: `workspace/management/runs/d3a2-smoke/smoke8-run.md`
- 実走ログ（一時）: scratchpad `smoke8.out`
- 主要ソース: `src/lib.rs:63`, `src/cli_panic_boundary.rs:11`, `src/workflow/orchestrator.rs:115`, `src/workflow/runner.rs:190`, `src/minimal_loop/repair_pressure.rs:4-7`
