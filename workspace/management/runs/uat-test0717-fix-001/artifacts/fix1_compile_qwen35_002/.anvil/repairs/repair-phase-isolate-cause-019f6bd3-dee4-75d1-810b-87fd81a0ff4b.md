Recover this failed run by producing and executing a focused ultra plan.

Original goal:
このNext.jsプロジェクトは npm run build が失敗します。原因を特定して修正してください。修正後もアプリの既存の検証が通ることを確認してください。

Profile: nextjs

Failure scope:
- phase: isolate-cause
- step: unknown
- kind: phase_execute_error

Failure evidence:
- model_stagnation:read_only_loop: write_required exhausted for src/app/components/SpaceInvaders.tsx; objective: Repair step `verify-nextjs-build`. Verification failed: implementation_compile_error: src/app/components/SpaceInvaders.tsx:305:22 Type error: Argument of type '{ x: number; y: number; }' is not assignable to parameter of type 'Bullet'.. Repair target: implementation. Fix the implementation files that should satisfy the requested behavior. Make the smallest bounded change, then stop. Pat

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- implementation

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
