Recover this failed run by producing and executing a focused ultra plan.

Original goal:
あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。

Profile: nextjs

Failure scope:
- phase: build-verification
- step: unknown
- kind: final_acceptance_repair_failed

Failure evidence:
- model_stagnation:read_only_loop: write_required exhausted for package.json; objective: Repair the final acceptance failure for the current ultra run. Original ultra goal: あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。 Profile: nextjs Intent: create Final acceptance failure: - primary reason: release gate failed: contract_instrumentation_missing:restart - repair target: test_or_evidence - attempt: 1/2 Pending capability evidence remedies: - none Missing paths: - none Paths: -

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- test_or_evidence

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
