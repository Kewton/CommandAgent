Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Repair step `implement-inspection-pipeline`. Verification failed: data_inspection_schema:inspection_schema_violation:input_row_count_mismatch:expected=60:reported=24; inspection_schema_violation:distinct_values_missing_categorical_columns:date. Repair target: implementation. Fix the implementation files that should satisfy the requested behavior. Make the smallest bounded change, then stop. Overall goal: data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Repa

Profile: data

Failure scope:
- phase: data-inspection
- step: verify
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=6
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
