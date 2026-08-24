Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: final-verification-and-cleanup
- step: unknown
- kind: phase_execute_error

Failure evidence:
- step final-artifact-validation failed verification after bounded repair: data_inspection_schema:inspection_schema_violation:missing_keys:column_names,input_row_count,type_summaries,distinct_values,sample_rows; failure_kind=verify_repair_progress_unchanged; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true Paths: - repair prompt saved: .anvil/repairs/repair-final-artifact-validation-019f64a1-6cc4-7a03-be36-409386bc8e04.md - Recovery UltraPla

Missing paths:
- none

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
