Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Execute exactly one StepPlan step. Overall goal: data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を特定して修正してください。修正後もデータ契約の既存検証が通ることを確認してください。 Current step id: implement-fix Current step kind: implement Current step instruction: Update pipeline/main.py to handle empty strings in parse_amount function at line 53, preventing ValueError on int() conversion. Profile contract: Build one reproducible tabular-data pipeline with Python 3 standard-library csv/json/statistics only. Preserve input file

Profile: data

Failure scope:
- phase: repair
- step: implement
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=7
- write_required exhausted without Write/Edit to pipeline/main.py: attempts=2/2
- write_required selected_targets=pipeline/main.py; selection_reason=traceback_mapped

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- pipeline/main.py

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
