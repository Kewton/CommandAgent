Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Execute exactly one StepPlan step. Overall goal: output/results.json がデータ契約のスキーマ検証に失敗します。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。 Current step id: diagnose Current step kind: implement Current step instruction: Read only existing workspace files and the executed reproducer output for output/results.json がデータ契約のスキーマ検証に失敗します。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。; write output/diagnosis.md with exact error quotations, file:line references, an

Profile: data

Failure scope:
- phase: diagnose
- step: implement
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=7
- write_required exhausted without Write/Edit to output/diagnosis.md: attempts=2/2
- write_required selected_targets=output/diagnosis.md; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- output/diagnosis.md

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
