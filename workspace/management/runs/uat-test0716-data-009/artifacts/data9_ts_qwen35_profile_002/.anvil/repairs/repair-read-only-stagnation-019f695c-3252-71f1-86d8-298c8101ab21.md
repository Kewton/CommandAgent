Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Execute exactly one StepPlan step. Overall goal: data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。 Current step id: inspect-data-and-rules Current step kind: verify Current step instruction: Verify the profile-owned data_manifest_artifact contract by running every declared check and report any exact failure. Required final artifacts: - pipeline/main.py - output/inspection.json - output/results.json - output/report.md Required final capabilities: - data_recon

Profile: data

Failure scope:
- phase: data-aggregation
- step: verify
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=0
- write_required exhausted without Write/Edit to pipeline/main.py: attempts=2/2
- write_required selected_targets=pipeline/main.py,output/results.json; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- pipeline/main.py
- output/results.json

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
