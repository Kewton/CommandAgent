# E-5f Phase State Machine Design

Status: **QUEUED** (2026-07-29). The state inventory is fixed as review
material; no production state-machine migration is authorized by E-5d.

## Purpose and seam

E-5d first separated the runner into a public facade, driver, phase
orchestration, and acceptance boundary. E-5f may replace the implicit phase
control flow with a pure typed transition layer only after review of that
extracted seam.

The proposed module boundary is:

```text
src/planner/runner/phase.rs
src/planner/runner/phase/state.rs       typed states and terminal outcomes
src/planner/runner/phase/transition.rs  pure state + observation -> transition
src/planner/runner/phase/effects.rs     existing side effects in existing order
```

The transition layer must not perform provider, filesystem, process, browser,
or network I/O. It selects existing effect labels; effects retain the current
event, evidence, error, and stop-class bytes.

## Sixteen states

| # | State | Responsibility |
|---:|---|---|
| 1 | `Initializing` | Lint the plan and initialize config, runtime, session, context, and intent runtimes. |
| 2 | `PhaseStart { index }` | Check the existing interruption boundary, emit phase start, snapshot the profile, and attach context. |
| 3 | `PhasePlanning { index }` | Resolve the deterministic, synthesized, or generated `StepPlan`. |
| 4 | `PhasePlanReady { index }` | Bind, finalize, save, and announce the phase plan. |
| 5 | `IntentBeforePhase { index, kind }` | Run the standard, fix, or investigation pre-phase hook. |
| 6 | `IntentPhaseConsumed { index, kind }` | Record that an intent runtime consumed the phase and deliberately skipped standard execution. |
| 7 | `PhaseExecuting { index }` | Execute the bounded `StepPlan` and accumulate its typed outcome. |
| 8 | `InvariantChecking { index, final_phase }` | Observe the profile invariant and current runtime snapshot. |
| 9 | `InvariantRepairing { index }` | Apply bounded non-final invariant repair or compile rollback. |
| 10 | `PhaseCommitting { index, final_phase }` | Emit existing verification/profile/complete events, clean up, reconcile, and attempt promotion. |
| 11 | `IntentFinalizing { kind }` | Finish a fix or investigation runtime after its phases. |
| 12 | `FinalAcceptance { cycle }` | Run the existing N/C/E/profile/browser checks and classify their report. |
| 13 | `FinalRepair { attempt, mode }` | Apply the existing bounded repair, re-anchor, regeneration, or rollback policy. |
| 14 | `Completed` | Emit the existing completion event and return the existing success bytes. |
| 15 | `Failed { stage }` | Preserve the current failure, evidence, handoff, and error bytes for the observed stage. |
| 16 | `Interrupted { stage }` | Preserve the current interruption bytes at an existing checked boundary. |

`Completed`, `Failed`, and `Interrupted` are terminal and reject every outgoing
transition. The machine is hierarchical: the existing bounded step/repair
loop remains an effect-owned submachine represented by one
`StepPlanRunOutcome`; counters do not create a Cartesian-product state set.

## Non-negotiable transition invariants

1. `ultra_context_initialized` precedes every phase event.
2. Phase start, context attach, scaffold complete, plan validation, execution,
   invariant observation, phase completion, and final acceptance retain their
   current order and bytes.
3. Fix/investigation-consumed phases do not gain standard execution,
   invariant, phase-commit, or create-acceptance events.
4. A non-final invariant failure may enter repair; a final-phase invariant
   failure remains an observation deferred to final acceptance.
5. No failure or interruption may emit a later phase-complete, plan-complete,
   or false-success event.
6. Interruption checks are encoded only at current boundaries; adding,
   removing, or moving a check is a separate semantic change.
7. Unlisted I/O/provider/dependency errors preserve existing propagation and
   already-emitted effects; the transition layer invents neither handoffs nor
   terminal events.

The full trigger-by-trigger inventory and source ownership are fixed in
[`e5d-split-audit.md`](../../workspace/management/runs/e5d-split-audit.md#complete-transition-inventory).

## Proposed implementation gates

E-5f remains a five-batch review candidate:

1. freeze ordered event/terminal fixtures for every transition family and
   illegal terminal transitions;
2. add pure state/transition types without a production call site;
3. migrate initialization through plan-ready while comparing old/new traces;
4. migrate execution, invariant, and phase-commit transitions;
5. migrate final acceptance, repair, and terminal transitions, removing the
   implicit loop only after byte and trace parity.

Each batch requires `cargo check`, ordered lifecycle fixtures, all snapshots,
conformance, adjudication byte fixtures, event-sequence tests, and the full
suite unchanged. Review after E-5d decides whether these batches should run.
