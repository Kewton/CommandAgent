# Development Guardrails

## Runner Growth Tripwire

`src/planner/runner.rs` and `src/minimal_loop/loop_run.rs` are interim group-3
chokepoints. The extracted planner and minimal-loop leaf modules also have
file-level growth budgets so they do not become replacement chokepoints.

The CI/test guard records current baselines and fails if any file grows
above baseline +2%:

- `src/planner/runner.rs`: 144 lines
- `src/planner/runner/driver.rs`: 3,273 lines
- `src/planner/runner/phase.rs`: 3,667 lines
- `src/planner/runner/phase/effects.rs`: 164 lines
- `src/planner/runner/phase/state.rs`: 220 lines
- `src/planner/runner/phase/transition.rs`: 268 lines
- `src/planner/runner/phase/phase_boundary.rs`: 44 lines
- `src/planner/runner/phase/step_plan_execution.rs`: 162 lines
- `src/planner/runner/phase/flow.rs`: 1,574 lines
- `src/planner/runner/acceptance.rs`: 2,523 lines
- `src/planner/runner/tests/**/*.rs`: 15,206 lines in aggregate
- `src/minimal_loop/loop_run.rs`: 6,934 lines
- `src/minimal_loop/loop_run/context.rs`: 267 lines
- `src/minimal_loop/loop_run/error.rs`: 72 lines
- `src/minimal_loop/loop_run/state.rs`: 109 lines
- `src/minimal_loop/loop_run/runtime_bash_effects.rs`: 186 lines
- `src/eval_events/typed.rs`: 143 lines
- `src/minimal_loop/repair_pressure.rs`: 746 lines
- `src/planner/repair_targeting.rs`: 589 lines
- `src/planner/final_acceptance.rs`: 2,235 lines
- `src/planner/adjudication/create.rs`: 2,152 lines
- `src/planner/adjudication/terminal.rs`: 7 lines
- `src/planner/assurance.rs`: 49 lines
- `src/planner/profiles/nextjs.rs`: 3,499 lines
- `src/minimal_loop/evidence.rs`: 6,527 lines
- `src/planner/capability_catalog.rs`: 614 lines
- `src/planner/fix_diagnostics/reproducer_execution.rs`: 29 lines
- `src/config.rs`: 3,560 lines
- `src/eval_events.rs`: 4,488 lines
- `src/minimal_loop/interaction_probe.rs`: 3,898 lines
- `src/minimal_loop/completion.rs`: 3,246 lines
- `src/config/resolution.rs`: 468 lines
- `src/direct_action_dispatch.rs`: 222 lines
- `src/eval_events/completion_render.rs`: 382 lines
- `src/minimal_loop/completion/feedback.rs`: 445 lines
- `src/minimal_loop/evidence/runtime_verdict.rs`: 296 lines
- `src/minimal_loop/evidence/runtime_verdict/tests.rs`: 497 lines
- `src/minimal_loop/evidence/verify_command_classification/node_checks/tests.rs`: 498 lines
- `src/minimal_loop/evidence/verify_command_classification/node_checks/tokens.rs`: 123 lines
- `src/minimal_loop/interaction_probe/observation_decode.rs`: 709 lines
- `src/minimal_loop/loop_run/contract_outcome.rs`: 730 lines
- `src/planner/profiles/nextjs/contract_checks.rs`: 94 lines
- `src/planner/profiles/nextjs/response_shape/syntax/tests.rs`: 169 lines
- `src/planner/profiles/nextjs/response_shape/syntax/tokenize.rs`: 94 lines
- `src/planner/recovery_observation_policy/nextjs_outputs/syntax/lexer/scan.rs`: 203 lines
- `src/planner/recovery_observation_policy/nextjs_outputs/syntax/lexer/tests.rs`: 619 lines
- `src/planner/runner/phase/step_failure_handoff.rs`: 595 lines
- `src/tui/slash/command_dispatch.rs`: 201 lines
- `src/verification/build_report.rs`: 92 lines
- `src/verification/diagnostics.rs`: 205 lines
- `src/verification/mod.rs`: 12 lines
- `src/verification/report.rs`: 615 lines

The same guard also measures production code and `#[cfg(test)]` code
separately. The total baseline above remains enforced; these split baselines
prevent test growth from hiding production growth, or production shrink from
masking test bloat:

| file | production baseline | test baseline |
| --- | ---: | ---: |
| `src/planner/runner.rs` | 131 | 13 |
| `src/planner/runner/driver.rs` | 3,273 | 0 |
| `src/planner/runner/phase.rs` | 3,667 | 0 |
| `src/planner/runner/phase/effects.rs` | 164 | 0 |
| `src/planner/runner/phase/state.rs` | 220 | 0 |
| `src/planner/runner/phase/transition.rs` | 175 | 92 |
| `src/planner/runner/phase/phase_boundary.rs` | 44 | 0 |
| `src/planner/runner/phase/step_plan_execution.rs` | 162 | 0 |
| `src/planner/runner/phase/flow.rs` | 1,574 | 0 |
| `src/planner/runner/acceptance.rs` | 2,502 | 21 |
| `src/minimal_loop/loop_run.rs` | 4,418 | 2,485 |
| `src/minimal_loop/loop_run/context.rs` | 267 | 0 |
| `src/minimal_loop/loop_run/error.rs` | 72 | 0 |
| `src/minimal_loop/loop_run/state.rs` | 109 | 0 |
| `src/minimal_loop/loop_run/runtime_bash_effects.rs` | 186 | 0 |
| `src/eval_events/typed.rs` | 110 | 33 |
| `src/minimal_loop/repair_pressure.rs` | 277 | 468 |
| `src/planner/repair_targeting.rs` | 448 | 138 |
| `src/planner/final_acceptance.rs` | 2,230 | 5 |
| `src/planner/adjudication/create.rs` | 2,138 | 14 |
| `src/planner/adjudication/terminal.rs` | 7 | 0 |
| `src/planner/assurance.rs` | 49 | 0 |
| `src/planner/profiles/nextjs.rs` | 2,161 | 1,323 |
| `src/minimal_loop/evidence.rs` | 3,819 | 2,694 |
| `src/planner/capability_catalog.rs` | 407 | 205 |
| `src/planner/fix_diagnostics/reproducer_execution.rs` | 29 | 0 |
| `src/config.rs` | 1,924 | 1,636 |
| `src/eval_events.rs` | 3,050 | 1,438 |
| `src/minimal_loop/interaction_probe.rs` | 1,835 | 2,063 |
| `src/minimal_loop/completion.rs` | 1,824 | 1,422 |
| `src/config/resolution.rs` | 468 | 0 |
| `src/direct_action_dispatch.rs` | 222 | 0 |
| `src/eval_events/completion_render.rs` | 382 | 0 |
| `src/minimal_loop/completion/feedback.rs` | 185 | 260 |
| `src/minimal_loop/evidence/runtime_verdict.rs` | 293 | 3 |
| `src/minimal_loop/evidence/runtime_verdict/tests.rs` | 497 | 0 |
| `src/minimal_loop/evidence/verify_command_classification/node_checks/tests.rs` | 498 | 0 |
| `src/minimal_loop/evidence/verify_command_classification/node_checks/tokens.rs` | 123 | 0 |
| `src/minimal_loop/interaction_probe/observation_decode.rs` | 709 | 0 |
| `src/minimal_loop/loop_run/contract_outcome.rs` | 126 | 604 |
| `src/planner/profiles/nextjs/contract_checks.rs` | 94 | 0 |
| `src/planner/profiles/nextjs/response_shape/syntax/tests.rs` | 169 | 0 |
| `src/planner/profiles/nextjs/response_shape/syntax/tokenize.rs` | 94 | 0 |
| `src/planner/recovery_observation_policy/nextjs_outputs/syntax/lexer/scan.rs` | 203 | 0 |
| `src/planner/recovery_observation_policy/nextjs_outputs/syntax/lexer/tests.rs` | 619 | 0 |
| `src/planner/runner/phase/step_failure_handoff.rs` | 232 | 363 |
| `src/tui/slash/command_dispatch.rs` | 201 | 0 |
| `src/verification/build_report.rs` | 92 | 0 |
| `src/verification/diagnostics.rs` | 85 | 120 |
| `src/verification/mod.rs` | 12 | 0 |
| `src/verification/report.rs` | 213 | 402 |

Issue #736 registered budgets for `src/config.rs`, `src/eval_events.rs`,
`src/minimal_loop/interaction_probe.rs`, `src/minimal_loop/completion.rs` and
the leaves created by the #721–#735 extractions. Each baseline is the measured
size after the extractions (total, production and test counted separately by
the same `#[cfg(test)]` classifier as above), not a measured size plus extra
headroom; the existing +2% rule is the only slack. Files named `tests.rs` that
carry no inner `#[cfg(test)]` are counted as production by the classifier, so
their baseline sits in the production column. The same change lowered
baselines of files that shrank (never raised one); entries that measured
slightly above their baseline stayed untouched.

When adding behavior, put new subsystems in new modules and call them from the
runner. Refactors that shrink these files are allowed; lower the baseline only
after the shrink is intentional and reviewed. Do not raise a baseline to admit
growth.

The runner test aggregate includes 8,440 lines mechanically transferred from
the former 8,429-line inline `runner.rs` test module (the delta is module
wiring) and all 6,765 lines already externalized under
`src/planner/runner/tests/`. The transferred tests are split by driver, phase,
acceptance, step-repair, compile-repair, and support responsibility; no new
support file exceeds 499 lines. Each test file also has its own baseline in
`tests/generality_guardrails.rs`; the aggregate and per-file checks prevent
relocation from becoming a growth-guard bypass.

Declarative Next.js and evidence knowledge must be changed in
`src/planner/profiles/nextjs/knowledge.toml` and
`src/minimal_loop/evidence_knowledge.toml`, respectively. Before adopting a
knowledge change, run and record the Space, Breakout, and Quiz scenario matrix
so vocabulary or contract drift is measured independently from Rust logic.

Profile manifests may reference only capabilities registered in
`src/planner/capability_catalog.rs`; free-form shell or logic-bearing templates
must not be registered. New capabilities require implementation, schema, golden
update, and tests in the same change.

CI is an acceptance necessary condition, not a substitute for the authorized
full-suite green run. The local acceptance conditions, including privileged
full verification where required, remain in force.

## Push Preflight

Before pushing a commit series, resolve its remote parent SHA and record the
GitHub Actions `CI` conclusion for that exact SHA. A failed, pending, cancelled,
or unavailable parent conclusion blocks the push and must be reported; only an
explicit owner instruction may authorize proceeding on a non-green parent.
