Recover this failed run by producing and executing a focused ultra plan.

Original goal:
あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。

Profile: nextjs

Failure scope:
- phase: release_gate
- step: unknown
- kind: browser_interaction_failed

Failure evidence:
- failed acceptance layer: release_gate
- final acceptance status: incomplete
- release gate status: failed
- primary reason: contract_instrumentation_missing:restart
- release gate reason: contract_instrumentation_missing:restart
- browser readiness: passed
- browser readiness evidence: /Users/<user>/share/work/localwork/commandagent_mvp/01/test0715_ff1_001/ff1_space_qwen35/.anvil/evidence/browser-readiness.json
- interaction evidence: failed:contract_instrumentation_missing:restart
- interaction evidence path: /Users/<user>/share/work/localwork/commandagent_mvp/01/test0715_ff1_001/ff1_space_qwen35/.anvil/evidence/browser-interaction.json
- interaction surface fit: div:state overflows the viewport by 6px; consider responsive sizing
- interaction probe mode: contract
- interaction contract hook status: usable
- interaction restart hook reachable after start: false
- interaction redispatched inputs: canvas/center click, ArrowLeft keydown, ArrowRight keydown, Space keydown
- interaction state dimensions changed: playerX

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
- capability_implementation

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
