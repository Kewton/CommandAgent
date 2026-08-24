Recover this failed run by producing and executing a focused ultra plan.

Original goal:
output/results.json がデータ契約のスキーマ検証に失敗します。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。

Profile: data

Failure scope:
- phase: diagnose
- step: unknown
- kind: phase_execute_error

Failure evidence:
- path does not exist: output/inspection.json

Missing paths:
- output/inspection.json

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
