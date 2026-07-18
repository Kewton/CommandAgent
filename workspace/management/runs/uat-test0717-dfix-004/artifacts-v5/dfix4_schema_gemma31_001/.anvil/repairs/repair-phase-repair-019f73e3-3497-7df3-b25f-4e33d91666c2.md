Recover this failed run by producing and executing a focused ultra plan.

Original goal:
output/results.json がデータ契約のスキーマ検証に失敗します。パイプラインを修正して正しい results.json を再生成し、既存検証が通ることを確認してください。

Profile: data

Failure scope:
- phase: repair
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:no_progress_recorded: objective: Execute exactly one StepPlan step. Overall goal: output/results.json がデータ契約のスキーマ検証に失敗します。パイプラインを修正して正しい results.json を再生成し、既存検証が通ることを確認してください。 Current step id: execute-pipeline Current step kind: implement Current step instruction: Run python pipeline/main.py to deterministically regenerate output/results.json and output/report.md based on the repaired pipeline logic. Profile contract: Build one reproducible tabular-data pipeline with Python 3 st

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- none

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
