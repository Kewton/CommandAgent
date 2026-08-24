Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: load-and-validate-data
- step: unknown
- kind: phase_execute_error

Failure evidence:
- step verify-pipeline failed verification after bounded repair: data_results_schema:failed to read /Users/<user>/share/work/localwork/commandagent_mvp/01/test0715_data_005/data5_gemma31_none_001/output/results.json: No such file or directory (os error 2); failure_kind=verify_repair_progress_unchanged; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true Paths: - repair prompt saved: .anvil/repairs/repair-verify-pipeline-019f64b6-d37c-7db1-8c93-

Missing paths:
- output/inspection.json
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
