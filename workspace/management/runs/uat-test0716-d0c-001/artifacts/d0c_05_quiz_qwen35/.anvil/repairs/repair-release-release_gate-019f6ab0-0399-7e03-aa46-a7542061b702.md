Recover this failed run by producing and executing a focused ultra plan.

Original goal:
シンプルで美しいクイズアプリ（3問・スコア表示・リトライ可能）を3011ポートで起動可能なnext.jsアプリとして開発してください。

Profile: nextjs

Failure scope:
- phase: release_gate
- step: unknown
- kind: release_gate_failed

Failure evidence:
- failed acceptance layer: release_gate
- final acceptance status: incomplete
- release gate status: failed
- primary reason: hook_snapshot_regression:src/app/page.tsx missing data-anvil-state; last-known-good phase build-verification
- release gate reason: contract_instrumentation_missing:primary
- browser readiness: passed
- browser readiness evidence: /Users/<user>/share/work/localwork/commandagent_mvp/01/test0716_d0c_001/d0c_05_quiz_qwen35/.anvil/evidence/browser-readiness.json
- interaction evidence: interaction_verified_heuristic_only
- interaction evidence path: /Users/<user>/share/work/localwork/commandagent_mvp/01/test0716_d0c_001/d0c_05_quiz_qwen35/.anvil/evidence/browser-interaction.json
- interaction probe mode: heuristic
- interaction contract hook status: state_missing
- interaction restart hook reachable after start: false
- interaction redispatched inputs: canvas/center click, ArrowLeft keydown, ArrowRight keydown, Space keydown
- interaction candidate table: - rank 1: text="スタート" changed=true

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- npm run build
- start dev server with npm run dev and wait for readiness
- probe browser route GET / and record HTTP status
- write browser-readiness.json with route_rendered/http_status
- run the interaction probe and record browser-interaction.json

Changed paths:
- none

Repair targets:
- release_acceptance

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
