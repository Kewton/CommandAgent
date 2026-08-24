# Diagnosis Report

## Failure Summary
The pipeline execution failed because the required artifact `output/inspection.json` was not created.

## Evidence
エラー引用: `inspection_schema_violation:inspection_path:path does not exist: output/inspection.json`
位置: N/A (Runtime check)

## Root Cause Analysis
Based on the provided file list, `pipeline/main.py` does not exist in the workspace. The pipeline was never implemented or was deleted, leading to the absence of all required output artifacts (`output/inspection.json`, `output/results.json`, `output/report.md`).

## Reproducer Steps
1. Attempt to run the pipeline logic (which is missing).
2. Run the `anvil-catalog-check:data_inspection_schema` validation.
3. Observe that `output/inspection.json` is missing.

## Proposed Fix
修正方針:
`pipeline/main.py` を作成し、`data/sales.csv` を読み込んで月次売上合計、前月比、3ヶ月移動平均を計算するロジックを実装する。また、計算過程で得られたデータプロファイル（無効行の数など）を `output/inspection.json` に出力し、最終結果を `output/results.json` および `output/report.md` に出力するように実装する。
