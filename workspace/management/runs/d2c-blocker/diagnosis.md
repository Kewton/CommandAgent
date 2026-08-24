# D-2c blocker diagnosis

測定日: 2026-07-18
対象HEAD: `2e3c8a3`

## 1. HEAD full suite

実行コマンド:

```text
cargo test --quiet 2>&1 | tee /tmp/head-full.log
```

終了値は `0`。失敗見出し検索結果は空で、全テスト（1452 unit tests、
integration/doc testsを含む）がgreenだった。従って今回のHEAD実行には失敗出力原文は存在しない。

前回採取した失敗原文（比較対象）は `/tmp/d2c-fail.txt` に保存されており、
完全テスト名は
`planner::runner::tests::compile_error_repair_prompt_anchors_file_and_then_runs_readiness`
である。原文の主失敗は `browser_readiness_status` の期待値 `passed` に対する
`browser_readiness_failed:start_exited` だった。

## 2. コミット行列

同一環境・同一コマンドで、各コミットをdetach checkoutして単独実行した。

| commit | test | result |
|---|---|---|
| 737b9b2 | compile_error_repair_prompt_anchors_file_and_then_runs_readiness | PASS |
| a755f1a | compile_error_repair_prompt_anchors_file_and_then_runs_readiness | PASS |
| 5aa97dd | compile_error_repair_prompt_anchors_file_and_then_runs_readiness | PASS |
| 2e3c8a3 | compile_error_repair_prompt_anchors_file_and_then_runs_readiness | PASS |

737b9b2でfailしなかったため、指定されたbaseline 3回反復条件は発動しない。

## 3. 経路特定

このテストはNext.jsのplanner生成経路を通る。生成入口は
`src/planner/runner.rs:494` の `generate_step_plan_with_ui_for_phase`、決定的
テンプレート入口は同 `:771` の `deterministic_step_plan_for_phase`、setup fallback
は `:8319` の `fallback_step_plan_for_setup_phase`。

生成3箇所はそれぞれ `lint_template_contract`（`:630`, `:811`, `:8341`）を通る。
fix×data合成経路は `src/planner/fix_plan_synthesis.rs:335` から
`finalize_step_plan_for_execution`（`src/planner/step_plan_finalize.rs:5`）へ入り、
修復後にlintする。従ってa〜dの同一テスト行列では、5aa97ddの是正による
Next.js挙動差は観測されなかった。

## 結論

**B: 環境依存/flaky（D-2cとは独立）**。baseline 737b9b2を含む4コミットが
全てPASSで、HEAD full suiteもPASSだったため、前回の`start_exited`はcommit差で
再現しない非決定的な環境事象と判定する。テスト決定化（port/timing依存除去）は
別タスクとしてqueued扱いにし、fixture・src・testsは変更していない。

