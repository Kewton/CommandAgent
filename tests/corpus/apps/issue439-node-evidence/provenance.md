# Issue #439 fixtures

Read-only source: campaign `20260907-1317-standard10`, binary parent `36f73eeb`.
R0 session `01a07a1a-1cf5-7962-928a-b30792eb90da`; S1 session
`01a07a5f-5e10-7112-8505-fbf483c35246`.

The root contains R0's retained source and the actual `smoke-check.js`, including
the `failures.length > 0` branch with exit 1 and its success branch with exit 0.
`fixtures/source-sha256.json` records hashes of the original source bytes.
R0's contract came from `.commandagent/completion-contract-ultra-plan-run.json`.
S1 source is stored as a JSON map and materialized only in temporary test roots;
it came from `.commandagent/recovery-treatments/attempt-1/workspace`. Its retained
contract is `.commandagent/recovery-runtime/completion-contract.json`.

The event JSON arrays are selected fields from `ultra_phase_complete`,
`ultra_final_acceptance`, and `recovery_promotion_decision`; they are portable
regression fixtures, not raw event logs. Browser evidence preserves observed
values with local paths, screenshot references, and URLs removed. Historical
files were neither edited nor used as test execution workspaces.

The registration test feeds the recorded commands through StepPlan registration,
loads the generated run contract, binds recovery authority, and checks handoff
acceptance. A separate configured-contract test proves the retained registry is
unchanged. Recorded build/browser success is fixture input, not a newly executed
Next.js build or a business-completeness verdict. The real smoke command is
executed on copied source, then a missing state attribute proves exit 1 remains
a command failure.

`fixtures/node-negatives.json` also includes the four source counterexamples
from the read-only reviewer record
`workspace/management/runs/20260908-0035-issue439-review-preparation/node-runtime-counterexamples.json`.
The Issue test independently runs all four with Node, confirms exit 0, and
requires weak classification. The reviewer's original record was not changed.
Two derived cases add nested shadowing and assertion alias mutation to the same
runtime/classification check.
