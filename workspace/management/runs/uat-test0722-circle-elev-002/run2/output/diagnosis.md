# Diagnosis Report

## Issue
The task 『data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。』 failed because the pipeline implementation (`pipeline/main.py`) is missing from the workspace. Consequently, no execution could be performed to generate the required artifacts (`output/results.json`, `output/report.md`, etc.).

## Reproduction Steps
1. Check for the existence of `pipeline/main.py`.
2. Attempt to run the pipeline (which fails as the file is missing).

## Evidence
The following files are present in the workspace:
- data/sales.csv
- evidence/inspection-schema.json
- evidence/investigate-events.jsonl
- evidence/investigation-run.json
- evidence/workflow-events.jsonl
- sales.csv.sha256

`pipeline/main.py` is absent.

## Root Cause
The implementation of the data pipeline was not provided or was deleted, making it impossible to process `data/sales.csv` and produce the requested analysis.

修正方針: 
pipeline/main.py を新規作成し、data/sales.csv の読み込み、月次集計、前月比および3ヶ月移動平均の計算、無効行の除外ロジック、および指定形式の output/results.json と output/report.md の出力機能を実装する。
