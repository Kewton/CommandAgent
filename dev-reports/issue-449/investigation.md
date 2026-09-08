# Issue #449 R0 repair investigation

The observed final repair loop allowed an initial Read, selected the projects
API, then rejected two further Reads at the existing write-required limit.
There is no recorded Write/Edit call in that final subrun to demonstrate an
improper edit rejection. Two context losses reproduce in deterministic tests;
their effect on the model's choices in R0 remains **unknown**.

## Evidence identity

Campaign `20260908-2310-standard10`, session
`01a08187-2bf0-7853-b0c9-814043569f26`, historical product HEAD
`5572467ee5028ae43f8497f875e1ef16030cf470`. The central event stream has 384
lines and SHA-256
`9614fe063efc51c032ca6afd1d1b14de5e6df424285e4f3e194461ae0681affe`, matching
the integration R0 failure-evidence index. [r0-trace.json](r0-trace.json)
contains derived call/result rows, exact source location and four saved-handoff
hashes. It is not a reconstructed provider transcript.

The fixed source, contract and measured compiler results are consumed from
`tests/corpus/apps/issue448-nextjs-r0/` at
`3952f99ff3b2afde40b8d4e88380141044815836`. Integration AGENTS.md, the R0 stop
report, Codex 2 resolution and parent coordination handoff were read. Existing
evidence, generated source and runtime state were not modified.

## Diagnostic to outcome ledger

| Stage / evidence | Diagnosis, target and permission | Calls, delta and verification |
| --- | --- | --- |
| Initial API implementation, events 233–249 | At 233, compact-restatement targets are empty. The later failure is missing relative imports. Exact provider messages are unknown. | 235–237 request Edit on projects, tasks and tasks/[id]; 238–240 report ok. Old/new contents are omitted (only lengths recorded), so per-call semantic repair is unknown. Reads of types.ts follow at 242/246; 249 stops `loop_progress_exhausted`. |
| Initial saved phase prompt, event 253 | Original goal and `npm run build` retained. Targets are `none`; failure evidence describes missing implementation capability, without `isValidTaskStatus`. | No claim that the export diagnosis survived this phase transition. Event 250 has only `missing relative imports`, and 252 changes the phase reason to unresolved implementation evidence. Earlier full diagnostic handoff is unavailable in the inspected records. |
| Bound Recovery plan, events 258–262 | Original contract goal and build retained; eight scaffold/UI paths become targets. Saved YAML lacks both named API diagnostics. | Preflight reports `build_verifier_failed`. Exact diagnostic output passed to the planner is unknown. The inspect-phase wording alone does not establish a Write ban. |
| Recovery UI treatment, 314–334 | Write targets `src/app/page.tsx`, succeeds at 317. Build at 319 detects the missing export and projects Promise mismatch; 320 selects projects route. | 322–325 Read projects, both task routes and types; results 326–330 are ok. 331 has `has_edit=false`, `inspect_only=true`; 334 has unchanged before/after delta `[page.tsx]`. |
| Verify subrun, 343–358 | Bash at 344 is rejected for `workspace_path_outside_root` (345–347); linked node_modules path is outside treatment root. This is not placeholder rejection. | Reads at 349–351 succeed. Build at 356 again fails with the same API diagnostics. 357 extracts projects route; 358 reports implementation compile failure. |
| Bounded repair, 359–364 | Required path is projects route. 364 records `write_required`, reason `required_path`, streak 6. Objective snippet contains the Promise error, repair mandate and start of original goal. | Read at 362 succeeds at 363. Full submitted messages and the six schema names are **unknown**; schema count does not establish absence of Write/Edit. |
| Exhaustion, 365–373 | Same projects target. Reads 366/369 are rejected at 367/370; counts 1/2 then 2/2. | No Write/Edit call is recorded in this subrun. 372 stops `model_stagnation:read_only_loop`; 373 records zero changed paths. There is no recorded subsequent API revalidation; its result is **unknown**, not success. |
| Read-only handoff, 371 | Saved prompt retains original goal, projects target and `npm run build`, but drops the actual Promise error; only stagnation and target remain. | This diagnostic-loss boundary is reproduced and repaired in #449. It is downstream of the model's final Reads, so it cannot explain those preceding choices. |
| Terminal, 378–384 | Treatment delta contains only `page.tsx`. | Control retained; promotion rejected with `recovery_execution_failed`; auto recovery stops `not_recoverable`. No second attempt was pending. |
| Independent compiler, #448 measured matrix | Original/UI-only both retain missing export and Promise errors. Promise-only exposes the unexported helper as the blocking error; export-only retains the Promise error. | All partial variants fail. Fully repaired fixture builds, but build success alone does not establish original business/browser acceptance. |

## Reproduced product inconsistencies and limits

1. `build_compact_compile_repair_prompt_with_context` and regeneration accepted
   a `RepairContext` but omitted its original goal and verify commands. The
   runner creates a fresh session for compact retries, so prior session context
   cannot be assumed. Remaining profile failures (including the unexported
   helper while the compile frame targets projects) were also omitted. The new
   leaf renderer retains these supplied fields and relevant paths. The existing
   compile frame and bounded Write/Edit instructions stay in force. R0 did not
   record a compact retry after its final read-only exhaustion; the fixture
   improvement is not evidence this omission caused that terminal result.
2. `save_read_only_write_required_handoff` used the objective only as a fallback
   goal. When a completion contract supplied the authoritative goal, the repair
   objective's diagnostics disappeared. It now stores that objective as a quoted
   failure-evidence item. JSON quoting preserves diagnostic content without
   letting embedded headings become top-level handoff sections. Existing display
   normalization/redaction still applies, and contract goal/verification remain
   authoritative.

The tests derive definition paths using the import scanner and supply them to
the existing targeting/context boundary. They demonstrate preservation of known
targets through feedback and handoff, not automatic reconstruction of missing
initial-phase diagnostics. Neither a general target-priority error nor an
improper target Edit rejection was demonstrated. The initial phase abstraction
loss remains a recorded limitation; this patch does not claim to fix it.

Historical tool schema names, complete provider messages, reasons for model
choices, and per-call original Edit contents stay **unknown**. Current registry
and write-required tests establish current product permissions only. Model
capability, prompt wording and phase splitting are not assigned as a sole cause.
No stop limit, repair budget, verification, acceptance or promotion gate changed.

The inspected central run has no `trace/provider-*.json` records. The product
already supports opt-in `--trace` via `run_trace.rs`, recording scrubbed provider
messages, schemas and replies in owner-readable files. A separately authorized
new campaign can freeze that setting to collect the missing observations;
there is no reason to manufacture historical requests or introduce a second
raw-prompt logging mechanism here. This patch's additional persisted diagnostic
is the quoted exhausted objective, using the existing handoff scrubber.

## Effect and dependency status

This is a completed bounded investigation with reproduced context-preservation
fixes. Actual model repair effectiveness and success rate are **unconfirmed**.
Evaluate them in a new pinned campaign/B-1 after integration; R0 and its scores
remain unchanged. Browser automation was not run in this task.

Parent is adding #448 original-Next.js-contract promotion coverage. Its existing
generic positive control must not be treated as original business acceptance.
Passing #449 checks against the pinned fixture does not declare that follow-up,
dependency gate or merge readiness complete.
