Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: data-inspection
- step: unknown
- kind: phase_execute_error

Failure evidence:
- step create-inspection-script failed verification after bounded repair: data_inspection_schema:inspection_schema_violation:distinct_values_missing_categorical_columns:date; failure_kind=bounded_repair_exhausted; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true Paths: - repair prompt saved: .anvil/repairs/repair-create-inspection-script-019f6ae9-8564-7132-b6f9-da3a178c771c.md - Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan

Missing paths:
- pipeline/main.py
- output/results.json
- output/report.md

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- implementation

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
