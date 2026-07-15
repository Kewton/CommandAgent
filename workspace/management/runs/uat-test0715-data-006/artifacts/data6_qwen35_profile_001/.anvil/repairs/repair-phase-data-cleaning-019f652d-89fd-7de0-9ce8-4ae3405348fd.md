Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。

Profile: data

Failure scope:
- phase: data-cleaning
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:read_only_loop: objective: Execute exactly one StepPlan step. Overall goal: data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Current step id: implement-pipeline-main Current step kind: implement Current step instruction: Create pipeline/main.py using Python 3 standard library (csv, json, statistics, os, collections). The script must read data/sales.csv and output/inspection.json, validate each row against derived rules, track exclusions by reason,

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
