Recover this failed run by producing and executing a focused ultra plan.

Original goal:
data/sales.csv を処理する pipeline/main.py の実行がエラーで失敗します。原因を特定して修正してください。修正後もデータ契約の既存検証が通ることを確認してください。

Profile: data

Failure scope:
- phase: repair
- step: unknown
- kind: phase_execute_error

Failure evidence:
- duplicate expected path ownership: pipeline/main.py in fix-append-error and run-pipeline

Missing paths:
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
