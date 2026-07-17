Recover this failed run by producing and executing a focused ultra plan.

Original goal:
このNext.jsアプリはリスタート操作の契約フック（data-anvil-action="restart"）が欠落しており検証に失敗します。原因を特定して修正してください。既存の検証が通ることを確認してください。

Profile: nextjs

Failure scope:
- phase: isolate-cause
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:read_only_loop: write_required exhausted for package.json; objective: Execute exactly one StepPlan step. Overall goal: このNext.jsアプリはリスタート操作の契約フック（data-anvil-action="restart"）が欠落しており検証に失敗します。原因を特定して修正してください。既存の検証が通ることを確認してください。 Current step id: inspect-page-and-layout Current step kind: inspect Current step instruction: Read src/app/page.tsx and src/app/layout.tsx to locate the main UI structure, existing data-anvil-action attributes, and any restart/reset button or logic. Before

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
