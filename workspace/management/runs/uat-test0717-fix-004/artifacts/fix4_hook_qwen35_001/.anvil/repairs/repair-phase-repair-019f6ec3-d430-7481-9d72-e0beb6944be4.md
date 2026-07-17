Recover this failed run by producing and executing a focused ultra plan.

Original goal:
このNext.jsアプリはリスタート操作の契約フック（data-anvil-action="restart"）が欠落しており検証に失敗します。原因を特定して修正してください。既存の検証が通ることを確認してください。

Profile: nextjs

Failure scope:
- phase: repair
- step: unknown
- kind: phase_execute_error

Failure evidence:
- step add-restart-hook instruction is too long

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
