# M-4 再計測レポート（uat-test0714-m4-004）

## 概要

実行日: 2026-07-14  
対象ワークスペース: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_004`  
ビルド: `commandagent 0.1.0 5aa82a3`  
probe setup: Playwright 1.61.1、ready  
実行規律: 各run最大1回。再試行なし。

決定的CSVは `test0714_m4_001` から5つのdata runへ複製した。全ファイルのSHA-256は
`2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`。

## 事前チェック

- `cargo build --release`: 成功
- `cargo test`: sandboxの権限制約によりbrowser probe系を含む複数テストが失敗。M-4 run実行前に停止せず、失敗を記録した。
- Ollamaモデル確認: planner `qwen3.6:27b-coding-nvfp4`、executor `qwen3.6:35b-a3b-coding-nvfp4` / `gemma4:31b` が存在。

## Run結果

| Run | profile / executor / preset | 状態 | 備考 |
|---|---|---|---|
| 1 `data_agg_qwen27_plan_qwen35_exec_preset_profile_001` | data / qwen3.6:35b-a3b-coding-nvfp4 / profile | failed（正直終端） | `data-inspection` でread-only loop枯渇。repair/recovery artifacts保存済み。 |
| 2 `data_agg_qwen27_plan_gemma31_exec_preset_profile_001` | data / gemma4:31b / profile | interrupted / incomplete | planner実行中に計測全体を中断。再試行なし。 |
| 3 `data_agg_qwen27_plan_qwen35_exec_preset_none_001` | data / qwen3.6:35b-a3b-coding-nvfp4 / none | interrupted / incomplete | planner実行中に計測全体を中断。再試行なし。 |
| 4 `data_agg_qwen27_plan_gemma31_exec_preset_none_001` | data / gemma4:31b / none | interrupted / incomplete | planner実行中に計測全体を中断。再試行なし。 |
| 5 `data_agg_qwen27_plan_qwen35_exec_preset_profile_002` | data / qwen3.6:35b-a3b-coding-nvfp4 / profile | interrupted / incomplete | planner実行中に計測全体を中断。再試行なし。 |
| 6 `quiz_qwen27_plan_qwen35_exec_preset_profile_001` | nextjs / qwen3.6:35b-a3b-coding-nvfp4 / profile | interrupted / incomplete | project-setup完了後、core-implementation planner中に中断。再試行なし。 |

## Run 1の一次原因

Run 1は次のエラーで停止した。

```text
model_stagnation:read_only_loop:write_required exhausted for output/inspection.json
```

モデルは途中で `pipeline/main.py` と `output/inspection.json` を生成したが、検証ステップでCSV・スクリプトのReadを繰り返し、必要なinspection成果物の確定処理へ進めなかった。repair pressureのread-only枯渇規律が発火し、phase `data-inspection` を正直に失敗終了した。

これは移行ゲートの破損ではなく、data profileのinspection verify手順における停滞である。Run 1には以下が保存されている。

- `.anvil/runs/019f60f5-3299-7f21-9dac-fb760b5df37b/events.jsonl`
- `.anvil/runs/019f60f5-3299-7f21-9dac-fb760b5df37b/summary.md`
- `.anvil/repairs/repair-phase-data-inspection-019f60fa-a0d7-77f3-be47-c87d91e53ada.md`
- `.anvil/plans/recovery-ultra-plan-phase-data-inspection-019f60fa-a0d7-77f3-be47-c8882ab0b6ee.yaml`

## G基準判定

- G1移行ゲート: **FAIL**（6/6完了・収集に未到達）
- G2 B-2c assurance: **未判定**（Run 1のみ正直なfailed、残りは未完了）
- G3 B-2b配線: **未判定**
- G4 nextjs非退行: **未判定**（Run 6未完了）
- G5記録: Run 1の失敗原因と残り5本の未完了を記録。

## 結論

今回の `_004` は再計測として成立しなかった。主因はRun 1のdata-inspection read-only停滞であり、残り5本は結果を推測せず未完了として扱う。再計測を行う場合は、新規ワークスペースを用意し、今回の6本を再試行扱いにしないこと。

