Recover this failed run by producing and executing a focused ultra plan.

Original goal:
output/results.json がデータ契約のスキーマ検証に失敗します。パイプラインを修正して正しい results.json を再生成し、既存検証が通ることを確認してください。

Profile: data

Failure scope:
- phase: repair
- step: unknown
- kind: phase_scaffold_error

Failure evidence:
- invalid StepPlan after corrective retries: verify step requires at least one verify command

Missing paths:
- output/inspection.json

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
