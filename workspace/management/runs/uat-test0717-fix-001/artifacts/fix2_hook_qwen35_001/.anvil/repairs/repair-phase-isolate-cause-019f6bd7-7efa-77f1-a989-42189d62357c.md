Recover this failed run by producing and executing a focused ultra plan.

Original goal:
このNext.jsアプリはリスタート操作の契約フック（data-anvil-action="restart"）が欠落しており検証に失敗します。原因を特定して修正してください。既存の検証が通ることを確認してください。

Profile: nextjs

Failure scope:
- phase: isolate-cause
- step: unknown
- kind: phase_execute_error

Failure evidence:
- step inspect-layout failed verification after bounded repair: dependency_setup_missing: Next.js build dependency setup missing: node_modules/tailwindcss, node_modules/postcss, node_modules/autoprefixer; failure_kind=dependency_setup_authority_required; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true Paths: - repair prompt saved: .anvil/repairs/repair-inspect-layout-019f6bd7-7ef9-7320-ba9c-48cd438e3774.md - Recovery UltraPlan YAML saved: .

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- dependency_setup

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
