# Issue #242 design

## Scope

Add speculative phase-step planning without growing the planner runner facade.
The implementation lives in `src/planner/pipeline.rs`; the UltraPlan phase loop
in `src/planner/runner/phase/flow.rs` only owns lifecycle wiring. The existing
phase state machine, verification, repair, phase-boundary Gate, event schemas,
and `.anvil` layout remain authoritative.

## Design

The pipeline overlaps the provider-bound part of phase N+1 planning with phase
N's initial profile-invariant verification:

1. After phase N execution has updated `UltraRunContext`, build the exact phase
   N+1 prompt that a sequential run would use.
2. If the next phase requires model planning, clone the planner and request only
   the first StepPlan reply on a worker thread. Do not parse, lint, present,
   persist, or execute the speculative reply yet.
3. Run phase N invariant verification unchanged on the main thread.
4. If verification fails and repair is required, cancel and discard the worker
   before entering repair. A speculative error is never promoted to a run
   failure.
5. If verification passes, retain the reply behind the existing phase-boundary
   Gate. At phase N+1 planning time, adopt it only when an exact key containing
   the phase ID and fully rendered prompt still matches. Otherwise discard it
   and use the existing synchronous planner path.
6. Feed an adopted reply through the existing generation, normalization, lint,
   and retry path by wrapping the planner with a one-reply client. All existing
   plan events and side effects therefore remain in their sequential location.

Promotion-eligible generic runs are not speculated because their next prompt is
expected to change at the boundary. The exact-key check rejects any other
prompt-affecting boundary drift. Fix and investigation runtimes are also not
speculated because their next-phase planning depends on mutable adjudication
state. Deterministic phase plans stay on the normal path because they have no
provider latency to hide.

## Cancellation and failure semantics

The worker uses a cancellation-aware no-op UI and the existing bounded provider
call path. Verification failure, interruption, stale input, or owner drop sets
the cancellation flag and joins the worker. No speculative StepPlan is exposed
before the Gate, and provider/pipeline failure falls back to ordinary planning.

New additive pipeline lifecycle events record `started`, `adopted`, and
`discarded` decisions. Existing phase events retain their relative order; a
failed verification records discard before repair and never emits phase N+1
start, validation, persistence, or execution events.

## Tests and corpus

- Focused pipeline tests prove that provider work overlaps an open verification
  window, that a failed Gate cancels/discards work, and that a mismatched prompt
  cannot be adopted.
- A focused corpus fixture records the successful adoption and failed-Gate
  discard event contracts without changing existing corpus history.
