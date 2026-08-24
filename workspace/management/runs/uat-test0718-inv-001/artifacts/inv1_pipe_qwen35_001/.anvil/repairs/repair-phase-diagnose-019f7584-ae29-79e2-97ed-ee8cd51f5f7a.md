Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。

Profile: data

Failure scope:
- phase: diagnose
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:read_only_loop: write_required exhausted for output/diagnosis.md; objective: Execute exactly one StepPlan step. Overall goal: data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。 Current step id: diagnose Current step kind: implement Current step instruction: Read only existing workspace files and the executed reproducer output for data/sales.csv を処理する pipeline/main.py Paths: - recovery prompt saved: .anvil/repai

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
