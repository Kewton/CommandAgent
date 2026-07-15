Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: load-and-inspect-data
- step: unknown
- kind: phase_execute_error

Failure evidence:
- artifact_follow_through_exhausted: missing expected paths: output/results.json, output/report.md, check_outputs.py; artifact_stagnation_feedback_count: 1

Missing paths:
- output/results.json
- output/report.md

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
