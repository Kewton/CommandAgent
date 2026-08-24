Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Execute exactly one StepPlan step. Overall goal: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Current step id: execute-pipeline Current step kind: implement Current step instruction: Run python pipeline/main.py to process data/sales.csv and generate output/results.json and output/report.md. Profile contract: Build one reproducible tabular-data pipeline with Python 3 standard-library csv/json/statistics only. Preserve input files. The canonical artifacts are pi

Profile: data

Failure scope:
- phase: data-cleaning
- step: implement
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=7
- write_required exhausted without Write/Edit to output/results.json: attempts=2/2
- write_required selected_targets=output/results.json,output/report.md,pipeline/main.py,output/inspection.json; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- output/results.json
- output/report.md
- pipeline/main.py
- output/inspection.json

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
