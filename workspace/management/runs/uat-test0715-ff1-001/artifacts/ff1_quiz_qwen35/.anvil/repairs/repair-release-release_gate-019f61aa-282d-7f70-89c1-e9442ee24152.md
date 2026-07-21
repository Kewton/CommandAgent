Recover this failed run by producing and executing a focused ultra plan.

Original goal:
シンプルで美しいクイズアプリ（3問・スコア表示・リトライ可能）を3011ポートで起動可能なnext.jsアプリとして開発してください。

Profile: nextjs

Failure scope:
- phase: release_gate
- step: unknown
- kind: browser_interaction_failed

Failure evidence:
- failed acceptance layer: release_gate
- final acceptance status: incomplete
- release gate status: failed
- primary reason: browser_interaction_failed:start_transition_missing
- release gate reason: browser_interaction_failed:start_transition_missing
- browser readiness: passed
- browser readiness evidence: /Users/<user>/share/work/localwork/commandagent_mvp/01/test0715_ff1_001/ff1_quiz_qwen35/.anvil/evidence/browser-readiness.json
- interaction evidence: failed:start_transition_missing
- interaction evidence path: /Users/<user>/share/work/localwork/commandagent_mvp/01/test0715_ff1_001/ff1_quiz_qwen35/.anvil/evidence/browser-interaction.json
- primary/start controls must transition the visible app state before input is evaluated; wire the start action into state and render updates.
- input operations must visibly change actual application state, and that change must be reflected in the data-anvil-state JSON snapshot; wire input handlers to state updates.
- interaction probe mode: contract
- interaction contract hook status: usable
- interaction restart hook reachable after start: false

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
- start_control_wiring

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
