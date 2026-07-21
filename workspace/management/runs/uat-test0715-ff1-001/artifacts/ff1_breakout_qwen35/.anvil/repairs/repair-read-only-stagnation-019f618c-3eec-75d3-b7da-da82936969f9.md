Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Repair the final acceptance failure for the current ultra run. Original ultra goal: あなたが考える最高に面白くかっこいいブロック崩しゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。 Profile: nextjs Intent: create Final acceptance failure: - primary reason: missing_required_evidence:stateful_update_evidence - repair target: implementation - attempt: 1/2 Pending capability evidence remedies: - stateful_update_evidence: update visible state over time or directly from user input, and expose the updated snapshot in data-anvil-state -

Profile: nextjs

Failure scope:
- phase: plan-run-step
- step: implement
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=7
- write_required exhausted without Write/Edit to src/app/page.tsx: attempts=2/2
- write_required selected_targets=src/app/page.tsx; selection_reason=evidence_mapped

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- src/app/page.tsx

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
