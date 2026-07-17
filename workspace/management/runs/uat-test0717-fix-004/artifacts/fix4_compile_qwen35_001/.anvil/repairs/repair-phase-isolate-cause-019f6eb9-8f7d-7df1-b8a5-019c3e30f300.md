Recover this failed run by producing and executing a focused ultra plan.

Original goal:
このNext.jsプロジェクトは npm run build が失敗します。原因を特定して修正してください。修正後もアプリの既存の検証が通ることを確認してください。

Profile: nextjs

Failure scope:
- phase: isolate-cause
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:read_only_loop: write_required exhausted for package.json; objective: Execute exactly one StepPlan step. Overall goal: このNext.jsプロジェクトは npm run build が失敗します。原因を特定して修正してください。修正後もアプリの既存の検証が通ることを確認してください。 Current step id: inspect-scaffold-files Current step kind: inspect Current step instruction: Read src/app/layout.tsx, src/app/globals.css, src/app/global.d.ts, and tailwind.config.ts to verify the Next.js app router scaffold is intact and CSS/module declarations are correct. Paths

Missing paths:
- none

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
