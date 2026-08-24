Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Repair step `implement-inspection-pipeline`. Verification failed: data_inspection_schema:inspection_schema_violation:missing_keys:column_names,input_row_count,type_summaries. Repair target: implementation. Fix the implementation files that should satisfy the requested behavior. Make the smallest bounded change, then stop. Overall goal: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Repair budget: - attempt 1/4 Required final artifacts: - pipeline/main.py - outpu

Profile: data

Failure scope:
- phase: data-inspection
- step: verify
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=0
- write_required exhausted without Write/Edit to output/inspection.json: attempts=2/2
- write_required selected_targets=output/inspection.json; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- output/inspection.json

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
