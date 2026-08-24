Recover this failed run by producing and executing a focused ultra plan.

Original goal:
シンプルで美しいクイズアプリ（3問・スコア表示・リトライ可能）を3011ポートで起動可能なnext.jsアプリとして開発してください。

Profile: nextjs

Failure scope:
- phase: build-verification
- step: unknown
- kind: final_acceptance_repair_failed

Failure evidence:
- artifact_follow_through_exhausted: missing expected paths: hook_snapshot_regression:src/app/page.tsx; artifact_stagnation_feedback_count: 2

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- contract_attribute_missing

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
