# Diagnosis Report

## Failure Observation
The pipeline execution failed during the `anvil-catalog-check:data_inspection_schema` stage.

エラー引用: `inspection_schema_violation:inspection_path:path does not exist: output/inspection.json`

## Investigation
Based on the provided file list:
- `data/sales.csv` exists.
- `pipeline/main.py` is **missing** from the workspace.
- `output/inspection.json` is **missing** from the workspace.
- `output/results.json` is **missing** from the workspace.
- `output/report.md` is **missing** from the workspace.

The failure occurs because the required artifact `output/inspection.json` was never created, which is a direct result of `pipeline/main.py` not existing to process the data.

## Root Cause
The pipeline implementation (`pipeline/main.py`) is completely missing from the workspace, preventing any data processing and artifact generation.

## Reproduction Steps
1. Run the pipeline (which fails immediately as `pipeline/main.py` does not exist).
2. Execute the schema check for `output/inspection.json`.
3. Observe the error: `inspection_schema_violation:inspection_path:path does not exist: output/inspection.json`.

修正方針:
`pipeline/main.py` を実装し、`data/sales.csv` を読み込んで月次集計（売上合計、前月比、3ヶ月移動平均）を行い、`output/inspection.json`、`output/results.json`、`output/report.md` を正しく出力するように実装する。
