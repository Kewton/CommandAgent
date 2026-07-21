Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Repair step `implement-fix`. Verification failed: data_results_schema:results.json missing required key `reconciliation`. Repair target: implementation. Fix the implementation files that should satisfy the requested behavior. Make the smallest bounded change, then stop. Overall goal: output/results.json がデータ契約のスキーマ検証に失敗します。パイプラインを修正して正しい results.json を再生成し、既存検証が通ることを確認してください。 Repair budget: - attempt 1/4 Required final artifacts: - pipeline/main.py Current step instruction: Repair the F1-diagnos

Profile: data

Failure scope:
- phase: repair
- step: implement
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=7
- write_required exhausted without Write/Edit to pipeline/main.py: attempts=2/2
- write_required selected_targets=pipeline/main.py; selection_reason=required_path

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
