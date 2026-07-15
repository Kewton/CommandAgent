Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: inspect-schema-and-define-rules
- step: unknown
- kind: phase_scaffold_error

Failure evidence:
- invalid StepPlan after corrective retries: planner_empty_response: planner returned empty content on attempt 3/3

Missing paths:
- pipeline/main.py
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
- phase_scaffold

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
