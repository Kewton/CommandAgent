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

## Test Isolation Ratchet

Tests share one process: its panic hook, its environment, the TUI's global
presentation state, and every descriptor open for writing when another test
thread forks. Stage 1 (#1010, #1012, #993, #984, #996, #1029, #995, #968,
#992) closed the holes that touched that state outside a guard.
`tests/test_isolation_guardrails.rs` (Issue #1040) keeps those kinds of line
from growing. It counts lines per file in `src/` and `tests/` and fails when a
file goes over its limit, or when a file without a limit has such a line:

- R1: `set_hook` or `take_hook` (the process panic hook), in any code.
- R2: `set_var(` or `remove_var(`, in any code; and
  `RuntimeEnvironmentGuard::install` in test code (the production calls in
  `src/lib.rs`, `src/operator_check.rs` and `src/tui/repl.rs` are not
  counted).
- R3: in test code, an exec mode (`0o[0-7]?[157][0-7][0-7]`) passed to
  `set_mode`/`from_mode`/`set_permissions` on the same line; and every
  `fs::copy` line, since whether a copy is an executable cannot be told from
  the line.
- R4: in test code, `render_current_plan`, `render_status_card` or
  `set_plan_for_test` (test entries into the TUI global state).

A line counts once per rule however many tokens it holds; comment and
doc-comment lines do not count. Test code is all of `tests/` and, in `src/`,
what the copied `classified_rust_lines` of `tests/generality_guardrails.rs`
marks as test, plus two additions kept outside it: a `#[cfg(all(test, ...))]`
attribute opens test code as `#[cfg(test)]` does, and a file declared by a
module item under a cfg(test) attribute or inside a cfg(test) inline module
(with `#[path = "..."]`, inline-module directories, and `include!` from test
code) is test code as a whole. The guard file itself is not counted, because
it holds every token as a string literal.

The limits are the counts at the commit that added the guard, with no
headroom. When a test goes over, put the new line under the existing guard or
in a separate process (for an executable, a separate-process copy helper of
the `copy_a_program_elsewhere` kind; for a fixture nothing executes, one copy
helper in one place), or land a change that removes such lines and lowers the
limit in the same change. Do not raise a limit to admit growth.

What this gate does not catch:

- The #992 kind: readers of the sandbox state. They are ordinary Bash or
  verifier calls with no token.
- Writers of the TUI global state: every test that goes through
  `eval_events::emit`.
- New tests that call an existing helper (`TestEnvGuard::set`, the `set_env`
  helpers under `tests/`, and the like). The helper's own line counts once.

It stops these kinds from growing; it is not a tool for finding the holes that
exist now.

Known limits of the classification:

- The copied `classified_rust_lines` is kept as it is: `#[cfg(test)]` must be
  alone on its line, a multi-line cfg attribute is not recognized, braces in
  strings can move the end of a block, and `#[cfg(test)]` followed by an item
  starting `mod tests` (including `#[path = "..."] mod tests;`) marks the rest
  of the file as test code.
- Inline-module nesting for module paths is read from indentation, which
  relies on `cargo fmt`.
- A `mod name;` inside an `include!`d file is resolved as if that file were
  its own module.
- In `src/`, a test-code module item that names no file in the tree, an
  `include!` whose argument is not a plain string literal, and an `include!`
  outside test code are listed as unresolved and fail the guard unless they
  are named in its `KNOWN_UNRESOLVED` list (empty when the guard was added).
- Tokens are matched as text: a renamed import (`use std::fs::copy;` then
  `copy(...)`) is not counted, and a token inside a string literal or a block
  comment is.

## Push Preflight

Before pushing a commit series, resolve its remote parent SHA and record the
GitHub Actions `CI` conclusion for that exact SHA. A failed, pending, cancelled,
or unavailable parent conclusion blocks the push and must be reported; only an
explicit owner instruction may authorize proceeding on a non-green parent.
