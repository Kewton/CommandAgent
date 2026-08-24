Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Execute exactly one StepPlan step. Overall goal: このNext.jsプロジェクトは npm run build が失敗します。原因を特定して修正してください。修正後もアプリの既存の検証が通ることを確認してください。 Current step id: inspect-scaffold-files Current step kind: inspect Current step instruction: Read src/app/layout.tsx, src/app/globals.css, src/app/global.d.ts, and tailwind.config.ts to verify the Next.js app router scaffold is intact and CSS/module declarations are correct. Before changing files, run the declared profile checks. If they already pass, report this st

Profile: nextjs

Failure scope:
- phase: isolate-cause
- step: inspect
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=0
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
