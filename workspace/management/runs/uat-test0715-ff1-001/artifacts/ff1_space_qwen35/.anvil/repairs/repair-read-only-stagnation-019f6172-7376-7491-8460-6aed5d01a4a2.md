Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Repair the final acceptance failure for the current ultra run. Original ultra goal: あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。 Profile: nextjs Intent: create Final acceptance failure: - primary reason: release gate failed: contract_instrumentation_missing:restart - repair target: test_or_evidence - attempt: 1/2 Pending capability evidence remedies: - none Missing paths: - none Dependency failures: - none Compile errors: - none Command failures: - none Profile failures: -

Profile: nextjs

Failure scope:
- phase: plan-run-step
- step: implement
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=7
- write_required exhausted without Write/Edit to package.json: attempts=2/2
- write_required selected_targets=package.json,tsconfig.json,postcss.config.js,tailwind.config.ts,src/app/layout.tsx,src/app/page.tsx,src/app/globals.css,src/app/global.d.ts; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- package.json
- tsconfig.json
- postcss.config.js
- tailwind.config.ts
- src/app/layout.tsx
- src/app/page.tsx
- src/app/globals.css
- src/app/global.d.ts

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
