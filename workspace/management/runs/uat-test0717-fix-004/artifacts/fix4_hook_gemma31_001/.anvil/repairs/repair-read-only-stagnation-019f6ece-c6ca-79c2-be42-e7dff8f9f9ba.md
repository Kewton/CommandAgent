Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Execute exactly one StepPlan step. Overall goal: このNext.jsアプリはリスタート操作の契約フック（data-anvil-action="restart"）が欠落しており検証に失敗します。原因を特定して修正してください。既存の検証が通ることを確認してください。 Current step id: inspect-layout-source Current step kind: inspect Current step instruction: Read src/app/layout.tsx to check for global data-anvil hooks, context providers, or CSS imports that might affect the restart contract or observability. Fix F1 profile contract predicate (runtime-bound): - capability: hook_attribute_present - write-pr

Profile: nextjs

Failure scope:
- phase: isolate-cause
- step: inspect
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=0
- write_required exhausted without Write/Edit to src/app/page.tsx: attempts=2/2
- write_required selected_targets=src/app/page.tsx; selection_reason=contract_attribute

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
