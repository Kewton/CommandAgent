Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Repair step `verify-build-failure`. Verification failed: implementation_compile_error: src/app/components/SpaceInvaders.tsx:305:22 Type error: Argument of type '{ x: number; y: number; }' is not assignable to parameter of type 'Bullet'.. Repair target: implementation. Fix the implementation files that should satisfy the requested behavior. Make the smallest bounded change, then stop. Overall goal: このNext.jsプロジェクトは npm run build が失敗します。原因を特定して修正してください。修正後もアプリの既存の検証が通ることを確認してください。 Repair budget: -

Profile: nextjs

Failure scope:
- phase: isolate-cause
- step: verify
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=6
- write_required exhausted without Write/Edit to src/app/components/SpaceInvaders.tsx: attempts=2/2
- write_required selected_targets=src/app/components/SpaceInvaders.tsx; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- src/app/components/SpaceInvaders.tsx

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
