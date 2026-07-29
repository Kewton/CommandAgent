# E-5d runner split audit

Date: 2026-07-29

Baseline: `3f7ccabf2398aa66d51a2947642778267317ebe8`

## Scope and rules

This is the E-5d stage-1 investigation record. It changes no production code,
test code, fixture, snapshot, guard baseline, or historical run evidence. The
only change in this commit is this new audit.

The primary subject is `src/planner/runner.rs` at the baseline above. Line
ranges are inclusive and refer to that immutable baseline. A range is assigned
to its dominant responsibility; helpers which serve several paths are called
out in the dependency column rather than counted more than once.

There are two useful ways to split the current 18,087 physical lines:

| Measurement | Production | Test | Total | Rule |
|---|---:|---:|---:|---|
| lexical top-level boundary | 9,655 | 8,432 | 18,087 | production is line 1 through the `#[cfg(test)]` immediately before `mod tests`; inline test module is line 9,656 through EOF |
| `tests/generality_guardrails.rs` classifier | 9,624 | 8,463 | 18,087 | also classifies 31 pre-boundary `#[cfg(test)]` import/helper lines as test |
| current guard baseline | 9,904 | 8,339 | 18,242 | baseline, not current size |
| current guard ceiling (`baseline + 2%`) | 10,103 | 8,506 | 18,607 | current remaining headroom is 479 production, **43 test**, and 520 total lines |

The responsibility map uses the lexical 9,655/8,432 split because it gives
stable, contiguous source ranges. Any later budget change must use the guard's
9,624/8,463 classification instead.

## Headline findings

1. The production portion is not one orchestration loop. Its largest dominant
   responsibilities are repair driving (2,085 lines), plan driving (1,655),
   acceptance boundary (1,634), event emission (1,521), phase progress
   support (1,297), and shared helpers (1,256).
2. The actual outer phase loop and the final-acceptance repair loop already
   live in `src/planner/ultra_plan_flow.rs` (1,601 lines). That file imports
   `runner.rs` with `use super::*` and calls private runner functions. Its
   current line guard is baseline 1,570, ceiling 1,602, leaving one line of
   headroom. A runner-only file move would preserve this hidden reverse
   dependency and would not complete the split.
3. The inline test module contains 141 `#[test]` functions, including two
   ignored subprocess entrypoints. Seven already-extracted files under
   `src/planner/runner/tests/` add 103 tests and 6,677 physical lines. Those
   6,677 lines are not presently included in the chokepoint budget.
4. Moving the 8,432 inline test lines without a new directory budget would
   create false headroom in `runner.rs`. The combined runner test surface is
   already 15,109 physical lines (8,432 inline + 6,677 extracted).
5. Existing verification is strong for values, event fields, terminal
   projection, conformance, and many individual recovery branches. It is not
   an exhaustive oracle for ordered phase transitions, cancellation timing,
   process cleanup, or side-effect ordering. Those are the first gaps to
   characterize before E-5f changes control flow.

## 1. Production responsibility map

### Exact range map

| Line range | Lines | Dominant responsibility | Dependency direction |
|---|---:|---|---|
| `1-127` | 127 | other | Imports shared minimal-loop/planner/provider/state APIs; declares the existing `final_acceptance`, `adjudication_create`, `assurance`, and `ultra_plan_flow` satellites; re-exports the UltraPlan public entrypoints to `planner::mod`. |
| `128-393` | 266 | helpers | Defines limits, session-mode types, requested-port policy, recovery-artifact validation, and report structs. Called by driver, phase, acceptance, and child modules; calls filesystem/YAML/profile-runtime helpers. |
| `394-879` | 486 | plan driving | StepPlan generation, provider retry, deterministic profile plans, plan presentation, and planner telemetry. Called by public wrappers and `ultra_plan_flow/{phase_plan_resolution,investigation_before}.rs`; calls provider, lint, sanitizer, preset, and profile-runtime policy. |
| `880-959` | 80 | initialization | Public StepPlan save/run/generate-and-run entrypoints. Called through `planner::mod` by `lib.rs`, `tui/slash.rs`, and tests; delegates to the session driver. |
| `960-1282` | 323 | phase progress | Step-plan outcome, `UltraRunContext`, promotion/setup context, bounded carry-forward state. Mutated by `ultra_plan_flow.rs` and step execution; calls acceptance refresh and context event helpers. |
| `1283-1667` | 385 | phase progress | Profile promotion, contract union/carry, outcome aggregation, and `StepRunOutcome`. Called at phase boundaries and by `repair_targeting.rs`; calls typed profile runtime and event emission. |
| `1668-1930` | 263 | phase progress | Error envelopes, setup-authority state, and dependency reconciliation lifecycle. Called before/after steps, phase transitions, promotion, and final acceptance; calls dependency/build verification and emits lifecycle events. |
| `1931-2091` | 161 | plan driving | StepPlan session driver, per-step loop, final-contract invocation, and lint repairability classification. Called by public wrappers and the UltraPlan phase loop; calls `run_step`, final contract verification, and UI interruption checks. |
| `2092-3276` | 1,185 | repair driving | `run_step`: execution turns, verification, bounded repair ladders, compile snapshots/regeneration/rollback, hook restore, stagnation handling, and partial outcome construction. Called only by the StepPlan session driver; calls minimal-loop execution, verifier, repair pressure/targeting, hook snapshots, and many event emitters. |
| `3277-3602` | 326 | phase progress | Setup-authority calculation, verify normalization/merge, pre-satisfied short circuit, session options, expected paths, and completion-contract binding. Called by `run_step` and plan-final verification; calls verify/profile/policy helpers and event emission. |
| `3603-3945` | 343 | acceptance boundary | Plan-level final completion-contract verification, profile behavior probe integration, release-gate/assurance projection, and final report merge. Called by the StepPlan driver; calls the profile runtime, final-acceptance adapter, evidence arbitration, and event projection. |
| `3946-4529` | 584 | helpers | Missing artifacts, invariant evidence, session-error observations, capability diagnostics, reachability evidence, report signatures, and bounded prompt/context utilities. Shared by step repair, phase repair, and final acceptance; calls profile runtime, import scan, and repair-target helpers. |
| `4530-4879` | 350 | repair driving | Intermediate profile-invariant repair, deterministic/model repair event emission, and post-repair build confirmation. Called from `ultra_plan_flow.rs`; calls minimal-loop repair, hook snapshots, invariant verification, and dependency/build lifecycle. |
| `4880-5177` | 298 | helpers | Depth/adherence/source token scans, route-bound action/input analysis, JSON reads, and profile repair prompt construction. Called by acceptance/recovery and phase summaries; calls profile runtime and source scanners. |
| `5178-5459` | 282 | event emission | Browser/interaction/profile-behavior probe execution and evidence events plus source diagnostics. Called by final acceptance and plan-final verification; calls probe engines and the resolved `ProfileRuntime`. |
| `5460-5531` | 72 | repair driving | Runtime acceptance repair guidance. Called by final-acceptance prompt/recovery construction; consumes profile-runtime guidance and failure evidence. |
| `5532-6822` | 1,291 | acceptance boundary | Next.js dev-server launch/readiness/interaction/cleanup probe, bounded logs, HTTP probe, process-group cleanup, environment/failure classification, and evidence construction. Called by the acceptance adapter; calls OS process/network APIs, bounded-process registration, browser probes, and lifecycle emitters. |
| `6823-7300` | 478 | repair driving | Release failure classification/evidence, recovery capabilities/targets, compile-output extraction, and verify-command selection. Called after acceptance failure; calls runtime acceptance diagnostics, repair targeting, and source/path scanners. |
| `7301-7831` | 531 | event emission | Phase/verification events, recovery handoff persistence and validation, failure stop text, partial-run summary, and phase-status derivation. Called mainly by `ultra_plan_flow.rs`; calls `repair.rs`, filesystem/YAML validation, and summary rendering. |
| `7832-8539` | 708 | event emission | Context events and all planner attempt/schema/lint/sanitization/fallback/quality events. Called by plan generation and phase orchestration; writes eval JSONL through `eval_events`. |
| `8540-9083` | 544 | plan driving | Planner error classification, retry prompts, message construction, UltraPlan/StepPlan system and user prompts, and metadata normalization. Called by generation loops; calls profile runtime guidance and stable prompt-layout helpers. |
| `9084-9191` | 108 | helpers | Preprovisioned scaffold note, workspace quality snapshot, and profile strengthening. Called before plan lint/finalization; calls filesystem and profile-runtime quality policy. |
| `9192-9655` | 464 | plan driving | Stable/legacy step prompts, prompt contract event, required-path injection, stable/legacy phase prompts, report formatting, and confined plan path resolution. Called by StepPlan and UltraPlan drivers; calls context/profile guidance and filesystem canonicalization. |
| **Production lexical total** | **9,655** |  |  |

### Aggregate by dominant responsibility

| Responsibility | Lines | Share of lexical production |
|---|---:|---:|
| initialization | 80 | 0.8% |
| plan driving | 1,655 | 17.1% |
| phase progress | 1,297 | 13.4% |
| acceptance boundary | 1,634 | 16.9% |
| repair driving | 2,085 | 21.6% |
| event emission | 1,521 | 15.8% |
| helpers | 1,256 | 13.0% |
| other/import and module wiring | 127 | 1.3% |
| **Total** | **9,655** | **100.0%** |

### Caller/callee topology

The intended public direction is:

```text
lib.rs / tui/slash.rs / workflow/orchestrator.rs
    -> planner::mod re-exports
    -> runner public entrypoints
    -> plan/phase driver
    -> acceptance and repair leaves
    -> minimal_loop / profile runtime / verifier / evidence
```

The current internal direction is not that simple:

```text
runner.rs
    declares ultra_plan_flow.rs
        -> use super::*
        -> calls private runner planning, phase, repair, acceptance, and event helpers
    declares final_acceptance.rs / adjudication/create.rs / assurance.rs
        -> use super::*
        -> calls private runner helpers
repair_targeting.rs
    -> imports runner::StepRunOutcome
```

The outer UltraPlan state is therefore split across `ultra_plan_flow.rs` and
`runner.rs`, while acceptance implementation is split across
`ultra_plan_flow.rs`, `runner.rs`, `final_acceptance.rs`, and
`adjudication/create.rs`. Removing `use super::*` and giving each boundary an
explicit input/output API is part of a real split; merely adding wrappers is
not.

### Existing satellites that must be accounted for

These files are outside the 18,087-line count but participate directly in the
same control path:

| File | Current lines | Role in the split |
|---|---:|---|
| `src/planner/ultra_plan_flow.rs` | 1,601 | Actual outer phase loop and final-acceptance repair loop; must become the phase driver rather than remain a glob-importing child. Current guard ceiling is 1,602. |
| `src/planner/ultra_plan_flow/before_phase.rs` | 55 | Intent-specific pre-phase dispatch. |
| `src/planner/ultra_plan_flow/investigation_before.rs` | 54 | Investigation reproducer/rebuild branch. |
| `src/planner/ultra_plan_flow/phase_plan_resolution.rs` | 26 | Preset/synthesis versus planner resolution. |
| `src/planner/fix_before.rs` | 45 | Fix-intent pre-phase branch. |
| `src/planner/final_acceptance.rs` | 2,215 | Acceptance calculations and repair prompt/evidence helpers. |
| `src/planner/adjudication/create.rs` | 2,158 | Create-intent acceptance adapter; intentionally private today. |
| `src/planner/assurance.rs` | 46 | Completion assurance bridge. |

## 2. Inline test relocation plan

### Current test surface

| Location | Physical lines | Tests | Notes |
|---|---:|---:|---|
| `runner.rs:9656-18087` | 8,432 | 141 | Two tests are ignored subprocess entrypoints. |
| `runner/tests/assurance_tests.rs` | 1,398 | 22 | Exact final-acceptance key set and assurance/projection paths. |
| `runner/tests/cli_runtime_dispatch_tests.rs` | 229 | 3 | Production-path C/N runtime activation evidence. |
| `runner/tests/data_pre_satisfied_tests.rs` | 182 | 3 | Includes a measured corpus workspace. |
| `runner/tests/final_acceptance_tests.rs` | 574 | 11 | Includes measured E2 nearest-miss evidence. |
| `runner/tests/profile_runtime_tests.rs` | 58 | 1 | Promotion contract monotonicity. |
| `runner/tests/requested_port_tests.rs` | 27 | 2 | Typed runtime port policy. |
| `runner/tests/ultra_plan_flow_tests.rs` | 4,209 | 61 | Main phase/acceptance lifecycle characterization set. |
| **Combined runner test surface** | **15,109** | **244** | 242 active and two ignored. |

The already-extracted files use the established nested form:

```rust
#[cfg(test)]
mod moved {
    use super::super::*;
    // ...
}
```

They still depend heavily on support types/functions in the inline parent
module (`FakeClient`, `config`, event readers, plan builders, Next.js workspace
builders). Support must move first or in the same atomic batch as its consumers.

### Exact inline range disposition

| Current range | Lines | Subject | Proposed destination | Snapshot/fixture and coupling notes |
|---|---:|---|---|---|
| `9656-9685` | 30 | test module imports and seven existing child declarations | `src/planner/runner/tests/mod.rs` | Replace the inline module with one `#[path = "runner/tests/mod.rs"] mod tests;` declaration. Keep child names stable. |
| `9686-9693` | 8 | `common_prefix` | `runner/tests/support/text.rs` | Used by prompt byte-prefix tests in several existing child modules. |
| `9694-9721` | 28 | env-conflict child harness and port-owner parser | root `runner/tests/mod.rs` plus `acceptance_boundary_tests.rs` | Keep the ignored child at `planner::runner::tests::dev_server_marker_with_contamination_is_env_node_env_conflict_child`; a hard-coded exact test name launches it. |
| `9722-11197` | 1,476 | StepPlan storage/path confinement, generation retries, sanitizer/fallback, prompts, quality, and contract inference | `runner/tests/driver_tests.rs` | Directly includes `src/eval/fixtures/plans/{source-step-plan.json,existing-mvp-step-plan.yaml,source-step-plan.expected.yaml}`. Most event checks are field/substr assertions, not whole-stream bytes. |
| `11198-12466` | 1,269 | dependency lifecycle, profile promotion, contract/handoff, plan-level acceptance | `runner/tests/phase_runtime_tests.rs` | Synthetic temp workspaces and JSONL assertions; shares plan/client/workspace builders. Some cases overlap existing `ultra_plan_flow_tests.rs` and should be moved without deduplication in the pure-move batch. |
| `12467-13778` | 1,312 | dev-server lifecycle and behavioral/final-acceptance repair | `runner/tests/acceptance_boundary_tests.rs` | Uses real subprocess/process-group tests, fake browser evidence, fake package manager, and exact lifecycle stage assertions. Timing and cleanup are the highest-risk test area. |
| `13779-14390` | 612 | dependency build events, external completion contract, step repair/stagnation/caps | `runner/tests/step_repair_tests.rs` | Synthetic execution replies plus recovery handoff files and JSONL event assertions. |
| `14391-16583` | 2,193 | fake clients and shared plan/event/Next.js/browser/build fixture builders | split under `runner/tests/support/{client,plan,event,nextjs,build}.rs` | Do not move this as one 2,193-line replacement chokepoint. `FakeClient` is consumed by every existing child module. |
| `16584-17840` | 1,257 | compile failure, compact/reanchor/regeneration/rollback, build-verifier routing | `runner/tests/compile_repair_tests.rs` | Relies on fake npm/build scripts, source snapshots, full build output, and JSONL ordering assertions. |
| `17841-17943` | 103 | shell quoting, dev-server guard/port/process/event helpers | `runner/tests/support/process.rs` | Unix process-group behavior is platform-sensitive. |
| `17944-18087` | 144 | requested-port boundary, legacy prompt bytes, frozen profile binding, shared `config` | tests to `driver_tests.rs`/`phase_runtime_tests.rs`; `config` to `support/config.rs` | Preserve `omitted_intent_preserves_legacy_ultra_prompt_bytes` as an exact prompt compatibility test. |
| **Inline total** | **8,432** |  |  |  |

### Hard-coded test identities

Three strings make a naïve module move fail even though production behavior is
unchanged:

- `planner::runner::tests::dev_server_marker_with_contamination_is_env_node_env_conflict_child`
- `planner::runner::tests::fake_dev_server_package_manager_child` in the fake
  package-manager script
- the same `fake_dev_server_package_manager_child` path in the compile-error
  workspace script

The two ignored child functions should remain directly under
`planner::runner::tests` in `tests/mod.rs`. Moving only their callers and
builders preserves the exact subprocess entrypoint and avoids an unrelated
test-harness migration.

### Growth-guard changes required when the move is authorized

No baseline is changed in this audit. A later move needs all of the following:

1. Lower the `runner.rs` total/production/test baselines after each reviewed
   shrink; never retain the vacated allowance.
2. Add an aggregate physical-line budget for
   `src/planner/runner/tests/**/*.rs`. The current classifier cannot infer that
   every included file is test-only when a file itself lacks `#[cfg(test)]`.
   The migration baseline must be the exact post-move total, seeded from the
   current combined 15,109 lines plus only unavoidable module-wiring deltas.
3. Add per-file caps for the new support and domain test files so the aggregate
   does not permit a new 8,000-line test chokepoint.
4. Add individual production/test budgets for `runner/driver.rs`,
   `runner/phase.rs`, `runner/acceptance.rs`, and any zero-behavior shared
   context module. Rebase by transferred lines; do not use the move to create
   net allowance.
5. Update `docs/dev/dev-guardrails.md` to the reviewed post-move values. Its
   current runner baseline is still 18,242/9,904/8,339.
6. Expand `tests/profile_runtime_guardrails.rs` from scanning only the text
   before `runner.rs`'s inline test module to scanning all production runner
   modules. Otherwise a split would let profile dispatch evade the E-5b guard.
7. Re-anchor, without increasing their semantic count:
   - `planner_template_lint_calls_have_three_audited_sites`, which currently
     requires all three sites to end in `runner.rs`;
   - `nextjs_boundary_erosion_tripwire_keeps_dispatch_sites_audited`, whose
     per-file count includes `runner.rs`;
   - `adjudication_dependency_direction_stays_create_to_skeleton`, which
     expects `mod adjudication_create;` to be private in `runner.rs`.
8. Keep `e5b-dispatch-audit.md` immutable. Its 110 historical line anchors
   remain a settlement record; a new split guard should check the new module
   set rather than rewriting history.

## 3. Split proposals

Both proposals use three responsibility modules. A small shared context/type
module is permitted only for data types with no orchestration behavior; it is
not a fourth responsibility bucket.

### Option A — pure three-way extraction

#### Proposed module structure

```text
src/planner/runner.rs                 public facade, module declarations, re-exports
src/planner/runner/context.rs         shared value types only
src/planner/runner/driver.rs          initialization + plan generation/driving
src/planner/runner/phase.rs           StepPlan/UltraPlan phase progress + step repair
src/planner/runner/acceptance.rs      final-contract/probe/recovery boundary
src/planner/runner/tests/...          relocated tests and support

Existing leaf implementations remain leaves:
src/planner/final_acceptance.rs
src/planner/adjudication/create.rs
src/planner/assurance.rs
src/planner/repair_targeting.rs
```

`ultra_plan_flow.rs` should be moved under or made an explicitly imported part
of `runner/phase.rs`; it must no longer use `super::*`. The target dependency
direction is:

```text
runner facade -> driver -> phase -> acceptance -> existing leaves
                         \-> minimal_loop/verifier/profile runtime
```

`context.rs` can be read by the three modules, but it must not call back into
them. `StepRunOutcome` should move to context or a phase-owned public(crate)
type; `repair_targeting.rs` must not depend on the runner facade.

#### Estimated migration batches

| Batch | Pure-move objective | Required gate before commit |
|---:|---|---|
| A0 | Add missing ordered-lifecycle characterization fixtures identified below | focused event tests + conformance |
| A1 | Move test support and preserve the two ignored subprocess identities | runner unit tests |
| A2 | Move remaining inline tests by responsibility; establish test-directory budgets | runner unit tests + guardrails |
| A3 | Extract acceptance low-level probe/recovery helpers | acceptance/assurance/adjudication tests |
| A4 | Extract the acceptance boundary and give existing leaves explicit imports | profile conformance + adjudication byte fixtures |
| A5 | Extract StepPlan execution/repair and phase context | runner/phase/repair tests + conformance |
| A6 | Move the outer UltraPlan flow into the phase module and remove `use super::*` | ordered lifecycle fixtures + TUI/runtime integration |
| A7 | Extract driver/initialization/prompt generation; leave a thin facade | plan golden/prompt tests + all targets |
| A8 | Rebase all budgets/dispatch guards/docs and run privileged full suite | full suite + CI/acceptance |

Estimate: **9 buildable batches**. A3/A4 and A5/A6 should remain separate even
if the compiler permits a larger move, because they separate value/schema
compatibility from control-flow/event-order compatibility.

#### Risks

- Pure extraction is well covered for function outputs, prompt bytes, event
  fields, terminal projection, and profile conformance.
- It is not fully covered for process lifetime, child cleanup timing,
  cancellation between events, provider retry timing, filesystem write order,
  or every recovery handoff byte.
- Moving private items can force visibility/ownership changes. Broad
  `pub(crate)` exposure would be a failed split; use narrow input/output
  structs rather than exporting the old internals.
- `ultra_plan_flow.rs` is already at its growth ceiling. Transitional wrappers
  cannot be parked there.
- Tests which only use `contains` can pass after event reordering. The A0
  characterization batch is therefore required even for a “pure” move.

### Option B — Option A plus explicit phase state machine (E-5f)

#### Additional module structure

```text
src/planner/runner/phase.rs
src/planner/runner/phase/state.rs       typed states and terminal outcomes
src/planner/runner/phase/transition.rs  pure transition function/table
src/planner/runner/phase/effects.rs     existing side effects keyed by transitions
```

The pure transition function should follow the admitted
`repair_pressure.rs` precedent: state + observed outcome -> next state +
effect labels. Effects perform the existing calls and emit the existing bytes;
the transition layer must not own provider, filesystem, process, or network
I/O.

#### State inventory

The state machine should be hierarchical rather than encode a Cartesian
product of phase, step, repair, and acceptance counters.

| State | Meaning and current source |
|---|---|
| `Initializing` | Lint plan, clone active config/plan, resolve runtime/expected paths, create session/context/intent runtimes (`ultra_plan_flow.rs:227-247`). |
| `PhaseStart { index }` | Interruption check, `ultra_phase_start`, profile snapshot, context attach (`248-268`). |
| `PhasePlanning { index }` | Resolve deterministic/synthesized/generated StepPlan (`269-324`). |
| `PhasePlanReady { index }` | Bind/finalize/save the plan and emit scaffold/lint completion (`325-348`). |
| `IntentBeforePhase { index, kind }` | Fix/investigation pre-phase hook; `kind = Standard | Fix | Investigation` (`349-365`). |
| `IntentPhaseConsumed { index, kind }` | A Fix/Investigation runtime consumed the phase and the standard StepPlan execution path was deliberately skipped (`363-365`). |
| `PhaseExecuting { index }` | Run the StepPlan and accumulate context/outcome (`366-476`); the existing bounded step/repair loop remains an effect-owned submachine in `run_step`. |
| `InvariantChecking { index, final_phase }` | Verify profile invariant and snapshot current runtime (`477-478`, `519-607`). |
| `InvariantRepairing { index }` | Deterministic/model repair and optional compile rollback for a non-final phase (`479-518`). |
| `PhaseCommitting { index, final_phase }` | Emit verification/profile/complete events, reap children, reconcile dependencies, and attempt profile promotion (`608-727`). |
| `IntentFinalizing { kind }` | Finish fix or investigation runtime after all phases (`728-733`). |
| `FinalAcceptance { cycle }` | Run N/C/E/profile/browser acceptance and classify the report (`734-760`, rechecks throughout the bounded loop). |
| `FinalRepair { attempt, mode }` | Apply bounded repair, re-anchor/compact/regenerate/rollback, then return to acceptance. `mode` is existing `RepairSessionMode` plus deterministic regeneration/rollback labels. |
| `Completed` | Emit `ultra_plan_complete` and return the existing success string (`1580-1601`). |
| `Failed { stage }` | Persist the handoff/evidence where the current path does so, and return the existing error bytes. `stage` is data, not a second failure vocabulary. |
| `Interrupted { stage }` | Return the existing interruption error at the observed boundary; workflow-level terminalization remains outside this module. |

`Completed`, `Failed`, and `Interrupted` are terminal. The machine must reject
any attempted outgoing transition from them.

#### Complete transition inventory

| From | Trigger/guard | To | Existing externally visible effects that must retain order and bytes |
|---|---|---|---|
| `Initializing` | plan lint/context setup succeeds | `PhaseStart { 0 }` | `ultra_context_initialized` precedes every phase event. |
| `Initializing` | lint/setup error | `Failed { initialization }` | Existing planner error/return text. |
| any nonterminal state | `ui.interrupted()` at an existing checked boundary | `Interrupted { current_stage }` | Existing `interrupted by user` text; no invented success/phase-complete event. |
| `PhaseStart { i }` | snapshot/context/prompt succeeds | `PhasePlanning { i }` | `ultra_phase_start` then context-attached event. |
| `PhaseStart { i }` | snapshot/context error | `Failed { phase_start }` | Existing propagated error; no later phase event. |
| `PhasePlanning { i }` | valid StepPlan | `PhasePlanReady { i }` | No reordering of planner attempt/sanitize/fallback events. |
| `PhasePlanning { i }` | generation/lint/preset failure | `Failed { phase_scaffold }` | `ultra_phase_failed(scaffold)` -> `planner_error` -> recovery handoff. |
| `PhasePlanReady { i }` | bind/save succeeds | `IntentBeforePhase { i, kind }` | `ultra_phase_scaffold_complete` -> `ultra_phase_plan_validated` -> saved plan. |
| `PhasePlanReady { i }` | bind/save fails | `Failed { phase_plan_persist }` | Preserve the currently propagated error; do not invent execute or recovery events. |
| `IntentBeforePhase { i, Standard }` | standard path | `PhaseExecuting { i }` | No additional event. |
| `IntentBeforePhase { i, Fix|Investigation }` | intent runtime consumes phase | `IntentPhaseConsumed { i, kind }` | Preserve intent-runtime-owned events; do not synthesize standard execute, invariant, or phase-commit events. |
| `IntentBeforePhase { i, _ }` | intent hook fails | `Failed { before_phase }` | Existing intent-specific failure/handoff. |
| `IntentPhaseConsumed { i, _ }` | another phase exists | `PhaseStart { i + 1 }` | This is the current `continue`; only intent-runtime-owned events may precede the next phase. |
| `IntentPhaseConsumed { last, kind }` | last phase consumed | `IntentFinalizing { kind }` | Skip standard final acceptance exactly as the current intent runtimes do. |
| `PhaseExecuting { i }` | StepPlan succeeds | `InvariantChecking { i, final_phase }` | context update -> `ultra_phase_execute_complete`. |
| `PhaseExecuting { i }` | StepPlan fails/exhausts | `Failed { phase_execute }` | context update -> `ultra_phase_failed(execute)` -> evidence/handoff. |
| `InvariantChecking { i, false }` | invariant passes | `PhaseCommitting { i, false }` | hook snapshot -> intermediate verification pass. |
| `InvariantChecking { i, false }` | invariant fails and repair is available | `InvariantRepairing { i }` | Existing failure evidence is retained for repair input. |
| `InvariantRepairing { i }` | repair or rollback attempted | `InvariantChecking { i, false }` | Existing deterministic/model repair and rollback events. |
| `InvariantRepairing { i }` | repair infrastructure errors | `Failed { profile_invariant_repair }` | Preserve the current propagated error/handoff behavior. |
| `InvariantChecking { i, false }` | still fails after bounded repair | `Failed { profile_invariant }` | failed verification -> `ultra_phase_failed` -> handoff. |
| `InvariantChecking { i, true }` | pass or final observation fails | `PhaseCommitting { i, true }` | Final phase records observed pass/failure and defers the final verdict to acceptance. |
| `PhaseCommitting { i, false }` | cleanup/reconcile/promotion succeeds and another phase exists | `PhaseStart { i + 1 }` | profile check -> phase complete -> reap -> reconcile -> promotion events. |
| `PhaseCommitting { i, _ }` | cleanup/reconcile/promotion error | `Failed { phase_transition }` | Existing propagated dependency/promotion error. |
| `PhaseCommitting { last, true }` | fix/investigation runtime exists | `IntentFinalizing { kind }` | No create final-acceptance event. |
| `PhaseCommitting { last, true }` | standard/create runtime | `FinalAcceptance { 0 }` | All phase-complete events precede acceptance. |
| `IntentFinalizing { _ }` | runtime finish succeeds | `Completed` | Preserve intent-specific result/event semantics. |
| `IntentFinalizing { _ }` | runtime finish fails | `Failed { intent_finalization }` | Preserve intent-specific failure/handoff. |
| `FinalAcceptance { cycle }` | report passes | `Completed` | final cycle summary precedes `ultra_plan_complete`. |
| `FinalAcceptance { cycle }` | report fails and bounded repair remains | `FinalRepair { next_attempt, selected_mode }` | `ultra_final_acceptance_failed` only on initial failure; repair-start fields unchanged. |
| `FinalAcceptance { cycle }` | report fails with no valid repair continuation | `Failed { final_acceptance }` | exhaustion/failure event -> cycle summary -> recovery handoff -> existing error. |
| `FinalAcceptance { cycle }` | verifier/probe infrastructure errors | `Failed { final_acceptance }` | Preserve the currently propagated error and any evidence already emitted; do not synthesize a verdict. |
| `FinalRepair { attempt, _ }` | changed/regenerated/rolled-back state requires recheck | `FinalAcceptance { attempt }` | repair complete/delta event precedes the next probe/check. |
| `FinalRepair { attempt, _ }` | no-change retry policy selects another mode without recheck | `FinalRepair { attempt + 1, next_mode }` | Existing no-source-change event/counter and direct loop continuation; do not invent an acceptance probe between attempts. |
| `FinalRepair { attempt, _ }` | repair turn error, wall-clock cap, or bounded exhaustion | `Failed { final_repair }` | Existing failed/exhausted event, decision event where required, summary, and handoff. |
| any nonterminal state | an unlisted currently-propagated I/O/provider/dependency error | `Failed { current_stage }` | Preserve the original `?` propagation and already-emitted events; the typed transition must not invent a handoff or terminal event. |
| `Completed|Failed|Interrupted` | any trigger | invalid transition | Test failure/panic inside the pure transition layer; no production event. |

The table intentionally does not turn every `run_step` repair rung into an
outer phase state. That rung is already governed by repair-pressure/session
state and should expose one typed `StepPlanRunOutcome` to the phase machine.
Likewise, workflow-level start/adjudication/epoch state remains owned by the
workflow orchestrator.

#### Estimated migration batches

Option B needs all nine A batches plus:

| Batch | State-machine objective |
|---:|---|
| B0 | Freeze ordered event/terminal fixtures for every transition family and illegal terminal transitions. |
| B1 | Introduce state/transition types and a pure table with no production call site. |
| B2 | Drive initialization through phase-plan-ready states; compare old/new transition traces in tests. |
| B3 | Drive execution/invariant/phase-commit states; preserve recovery/event order. |
| B4 | Drive final acceptance/repair/terminal states; remove the old implicit loop only after byte/trace parity. |

Estimate: **14 buildable batches total** if A and B are issued together, or
five additional batches after A. Combining B2-B4 would make a byte mismatch
impossible to localize and is not recommended.

#### Additional risks

- A state enum can accidentally imply that an event is emitted for every
  state. Current special fix/investigation phases deliberately bypass standard
  execution events.
- Current final-phase invariant failure is an observation passed to final
  acceptance, while the same failure in a non-final phase is terminal after
  repair. A flattened state machine can easily erase that asymmetry.
- Current interruption checks are not at every statement boundary. Adding
  checks is a semantic change; removing or moving them changes side-effect
  timing. E-5f must initially encode only existing boundaries.
- Transition traces can prove control decisions, but not child-process cleanup
  or filesystem durability. Effect tests remain necessary.

### Recommendation

**Recommend Option A first, with A0 mandatory, then review the extracted seam
before authorizing Option B/E-5f.** This is a sequencing recommendation, not a
decision; the choice remains review adjudication.

The reason is evidence-based:

- 9,655 production lines and 8,432 inline test lines must move while a
  1,601-line satellite already sits at its guard ceiling.
- Existing tests strongly constrain emitted values but do not exhaustively
  constrain transition order, cancellation, and process effects.
- A pure extraction creates an explicit phase/acceptance interface on which a
  pure transition table can be tested. Introducing that interface and changing
  control semantics in one batch would make a failure ambiguous between module
  movement and state logic.
- The repository already has a successful precedent: first isolate a seam,
  then centralize transitions as a pure table (`repair_pressure.rs`).

Option A should be designed so that choosing B later does not move files again:
`phase.rs` owns orchestration, `acceptance.rs` returns typed observations, and
all effects are invoked behind explicit phase operations.

## 4. Verification asset inventory

### Existing assets by responsibility

| Responsibility/boundary | Existing verifier assets | What they prove | What they do not prove |
|---|---|---|---|
| StepPlan generation/storage/prompts | Inline driver tests; three `src/eval/fixtures/plans/*` fixtures; stable/legacy prompt tests; planner retry/sanitizer tests | YAML/JSON parse compatibility, path confinement, retry classification, prompt sections/prefixes, deterministic fallback events | Exact full prompt bytes for every profile/layout and full ordered planner event stream |
| Phase progression | 61 tests in `runner/tests/ultra_plan_flow_tests.rs`; inline phase/dependency/promotion tests; `tests/tui_integration.rs` | Success/failure phase paths, promotion, dependency lifecycle, recovery files, many phase event fields, end-to-end command flow | Exhaustive legal/illegal transition table; exact event order for every branch; interruption at every phase boundary |
| Step execution and repair | Inline step/compile repair tests; repair-pressure tests; conformance boundedness cases | Iteration caps, no-change handling, compact/reanchor/regeneration/rollback outcomes, honest failure | Equivalence of all side-effect ordering and timing after ownership/module moves |
| Acceptance/projection | 22 assurance tests; 11 final-acceptance tests; C/N runtime activation tests; six `tests/adjudication_compat.rs` byte fixtures | Final event key set, serialized adjudication/projection bytes, assurance caps, runtime activation, selected repair evidence | Whole acceptance event stream bytes and ordering across every repair cycle |
| Profile contracts | Data/CLI/ingest profile conformance; fix/investigation/workflow-circle conformance; profile manifest/golden tests | Negative/full fixtures, assurance rules, claims/evidence semantics, honest terminal classification | Runner orchestration timing and OS resource cleanup |
| Cross-scenario behavior | `tests/corpus_regression.rs` over 112 corpus case directories and 741 files | Detector/probe/profile parity, committed fixture contents, selected ordered tokens | Live provider distribution, every runner event sequence, dynamic process behavior |
| Generic conformance | `tests/conformance/mod.rs`: 19 tests (18 active, one ignored child) | Bounded execution, precise exhaustion, false-success rejection, simulated panic terminalization | Full phase-state transition coverage and all platform-specific cleanup paths |
| Dispatch integrity | `tests/profile_runtime_guardrails.rs` and immutable E-5b audit | Three reviewed profile identity residuals in current `runner.rs`; all 110 historical sites accounted for | Split child modules unless the scan is expanded |
| Growth integrity | `runner_chokepoints_do_not_grow_past_interim_budget` | Current `runner.rs` total/production/test ceiling and listed leaf budgets | Existing `runner/tests/*.rs`; aggregate budget after relocation |
| Process/browser lifecycle | Inline fake dev-server/process-group tests and TUI integration | Selected Unix cleanup, readiness-before-cleanup, requested-port binding | Non-Unix cleanup, every timing interleaving, cancellation during cleanup, live browser/provider behavior |

### Byte-compatibility verifiers

The strongest existing byte-sensitive assets are:

1. `tests/adjudication_compat.rs`: exact serialized completion event key set,
   persisted event bytes, and adjudication projection bytes for six shapes.
2. `omitted_intent_preserves_legacy_ultra_prompt_bytes` and prompt-prefix/order
   tests in runner tests.
3. Corpus fixtures that compare archived measured E2 artifacts byte-for-byte,
   plus profile conformance fixtures for E/F/I/C/N families.
4. JSONL assertions in runner/UltraPlan tests that pin event names and selected
   arrays/fields.

They are necessary but not sufficient to claim whole-run byte equality. Most
runner JSONL tests use substring or selected-field assertions; the corpus
ordered-token sections cover selected fixtures, not every phase lifecycle.

### Verification gaps and first hardening target

The primary blank area is an **ordered lifecycle characterization matrix**.
Before any production move, add source-only fixtures for these representative
traces:

1. two-phase success through final acceptance;
2. phase scaffold failure;
3. phase execution exhaustion;
4. non-final invariant repair pass;
5. non-final invariant repair exhaustion and handoff;
6. final invariant observation followed by acceptance repair pass;
7. final acceptance repair exhaustion;
8. fix/investigation phase consumed by its intent runtime;
9. interruption before phase start and during an existing checked execution
   boundary.

Each fixture should pin:

- ordered event names;
- phase index/id and lifecycle stage;
- terminal result/error bytes and stop class;
- recovery artifact paths and parseability;
- absence of forbidden later events (`ultra_phase_complete`,
  `ultra_plan_complete`, or final acceptance after the relevant failure).

This is the first hardening target because snapshots and selected-field tests
cannot detect event reordering or an extra terminal event, while E-5f directly
changes those decisions.

Secondary gaps, which remain effect tests rather than transition-table tests:

- cancellation during provider backoff, dev-server wait, and process cleanup;
- child/process-group cleanup parity on non-Unix platforms;
- exact write ordering/durability of event, summary, recovery prompt, and
  recovery YAML;
- live provider/browser behavior (intentionally excluded from CI);
- a directory-level line budget for all runner tests.

## Review boundary

This audit recommends the A0 -> Option A -> separate E-5f sequence. It does not
authorize that sequence, change a guard baseline, or decide the final module
names. Production changes must wait for review adjudication.

## Stage 2 migration ledger

Review selected Option A on 2026-07-29. The following checkboxes bind each
mechanical transfer back to this audit; later batches append rows rather than
renumbering the original responsibility map.

| Batch | Status | Audited ownership consumed | Mechanical transfer | Compatibility result |
|---:|---|---|---|---|
| 0 | [x] complete (`f2e2b0c`) | Verification gap: ordered lifecycle characterization | Added a 22-event normalized two-phase lifecycle fixture before moving production code. | Focused fixture, adjudication 6/6, conformance 18/18 (1 ignored), corpus 1/1 green. |
| 1 | [x] complete | Inline test region `runner.rs:9656-18087` | Moved 8,429 source lines to `runner/tests/mod.rs` (8,384 after rustfmt), retained the `planner::runner::tests` module path, lowered the runner baseline to 9,658 lines, and enrolled all 15,149 runner-test lines in per-file plus aggregate growth guards. | `cargo check` and byte-sensitive suites green; no event/evidence/fixture bytes changed. |
| 2 | [x] complete | Inline disposition ranges `9686-18086` | Split the relocated body into driver, phase, acceptance, step-repair, compile-repair, and eight bounded support files. `include!` keeps all test identities directly under `planner::runner::tests`; the 8,429 moved lines plus 11 wiring lines and 6,765 existing external lines yield a guarded 15,205-line aggregate. | Ordered lifecycle and all 1,727 library-test definitions compile; ignored subprocess identities remain unchanged. |
| 3 | [x] complete | Acceptance helpers `runner.rs:5178-7298` | Moved 2,121 source lines of browser/interaction probes, dev-server lifecycle, evidence construction, and recovery classification to `runner/acceptance.rs` (2,149 formatted/wired lines). Rebased runner to 7,540 lines and added a 2,149-line leaf guard. | `cargo check`, acceptance-focused test compilation, ordered lifecycle, conformance, adjudication bytes, and corpus green. |
| 4 | [x] complete | Plan acceptance boundary `runner.rs:3603-3944` plus existing acceptance leaves | Moved the 342-line plan final-contract boundary into `runner/acceptance.rs`. Replaced glob parent imports with enumerated imports in `runner/acceptance.rs`, `final_acceptance.rs`, `adjudication/create.rs`, and `assurance.rs`; runner fell to 7,196 lines and all touched budgets were measured anew. | Production and test compilation green; ordered lifecycle bytes unchanged. |
| 5 | [x] complete | Phase/context/StepPlan execution `runner.rs:960-3602` | Moved 2,643 source lines into `runner/phase.rs` (2,660 formatted/wired lines). Preserved `runner::StepRunOutcome` through an explicit crate-visible re-export for `repair_targeting.rs`; runner fell to 4,557 lines. | `cargo check`, all 1,727 test definitions, ordered lifecycle, phase/repair conformance, adjudication bytes, and corpus green. |
| 6 | [x] complete | Intermediate invariant/phase recovery `runner.rs:4530-5177,7301-7831`; outer `ultra_plan_flow.rs` | Added 1,179 audited source lines to the phase boundary, moved the 1,601-line outer flow to `runner/phase/flow.rs`, and replaced its parent glob with 119 enumerated imports (including child-leaf requirements). Final measured budgets: runner 3,373, phase 3,857, flow 1,656 lines. | Ordered 22-event lifecycle fixture, TUI/runtime integration, conformance, adjudication bytes, and corpus green. |
| 7 | [x] complete | Driver/initialization/prompt generation `runner.rs:1-959,3945-4529,7832-9655` after prior transfers | Moved the remaining 3,242 source lines of constants, StepPlan APIs, initialization, event emission, and prompt construction to `runner/driver.rs` (3,273 formatted/wired lines). The 144-line `runner.rs` now contains imports, module wiring, stable public re-exports, and the test-module anchor only; both files have independent growth budgets. Mechanically re-anchored the compile-parser protection allowlist to `acceptance.rs`, excluded the already-guarded `runner/tests/` fixtures from the production child-process scan, and expanded the E-5b dispatch guard from the facade to every production module below `runner/`. | `cargo check`, the three audited template-lint sites, profile dispatch, and protection coverage green at their new owners; observable strings and event construction are mechanically unchanged. |
| 8 | [x] complete | Option A dependency-direction and guard settlement | Replaced the remaining production parent globs in `runner/driver.rs` and `runner/phase.rs` with compiler-derived explicit import sets. The formatted wiring adds 18 and 74 physical lines respectively, both inside the already-recorded +2% ceilings; no baseline was raised. Test namespace globs remain deliberately confined to `runner/tests/`. | `cargo check`, growth/dispatch/protection guards, ordered lifecycle, prompt bytes, conformance, adjudication bytes, and corpus remain green. |

## Stage 2 final settlement

### Final responsibility map

| File or family | Physical lines | Production/test classification | Final ownership |
|---|---:|---:|---|
| `src/planner/runner.rs` | 144 | 131 / 13 | Public facade, module declarations, stable re-exports, test anchor |
| `src/planner/runner/driver.rs` | 3,291 | 3,291 / 0 | Initialization, StepPlan generation/driving, prompt and shared event construction |
| `src/planner/runner/phase.rs` | 3,931 | 3,931 / 0 | Step execution/repair, phase context, invariant and recovery orchestration |
| `src/planner/runner/phase/flow.rs` | 1,656 | 1,656 / 0 | Outer UltraPlan lifecycle, owned by the phase responsibility |
| `src/planner/runner/acceptance.rs` | 2,523 | 2,502 / 21 | Final-contract, probe, evidence, and recovery boundary |
| `src/planner/runner/tests/**/*.rs` | 15,206 | test-only aggregate | Driver/phase/acceptance/repair domains plus bounded support modules |

The three responsibility modules are `driver`, `phase` (including `flow`), and
`acceptance`; `runner.rs` is a 144-line facade rather than a fourth behavior
bucket. Production parent globs are zero. Four same-namespace globs remain
under `runner/tests/` to preserve hard-coded test identities and shared
fixtures.

### Growth-guard coverage

The comparable physical runner-family guard surface is:

| Measure | Before stage 2 | Final | Delta |
|---|---:|---:|---:|
| Guarded runner family | 19,688 (`runner.rs` 18,087 + old outer flow 1,601) | 26,751 (all five production-module files 11,545 + test tree 15,206) | +7,063 |
| Existing external runner tests enrolled | 0 of 6,677 | 6,677 of 6,677 | +6,677 |
| `runner.rs` alone | 18,087 | 144 | -17,943 |

Before enrollment, the same code surface including the unguarded 6,677
external test lines was 26,365 lines. The final 26,751-line surface differs by
386 physical lines of module/import/fixture wiring, while guard coverage rises
by 7,063. Every production destination has a file budget; the test tree has
both aggregate and per-file budgets. Vacated runner allowance was lowered
after each transfer and no final baseline was raised to admit behavior.

### Compatibility and E-5b follow-through

- The pre-split 22-event ordered lifecycle fixture remains byte-identical.
- Existing snapshots and conformance fixtures were not updated.
- Six adjudication byte fixtures, prompt-byte fixtures, 18 active conformance
  tests, and the corpus regression remain green.
- Final privileged `cargo test --all-targets` is 1,873 passed, 30 ignored,
  zero failed; `cargo fmt --all -- --check` and all-target clippy with warnings
  denied are green.
- The profile-dispatch guard now recursively scans every production module
  below `runner/`, excluding only `runner/tests/`; all 110 historical E-5b
  sites and three typed residuals remain accounted for.
- The E-5b fifth-profile contact simulation remains **26 -> 20**. The type and
  registry boundary carries completion projection, production acceptance
  activation, preset selection, repair policy, guidance/material injection,
  and probe selection; profile-specific semantics and fixtures remain review
  work.

### E-5f queue

The 16 reviewed states and their transition invariants are preserved in
[`docs/dev/e5f-phase-state-machine.md`](../../../docs/dev/e5f-phase-state-machine.md).
E-5f is **QUEUED**. The proposed state machine is a separate semantic migration
to be decided on the Option A terrain; E-5d does not authorize it.
