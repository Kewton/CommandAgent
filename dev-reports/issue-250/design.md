# Issue #250 design

## Goal

Allow an external draft profile manifest and its operator-local pack to run a
declarative command check at final acceptance. The result must be visible in
events and the run summary, must participate honestly in acceptance, and must
never raise a draft profile above its existing `static` admission ceiling.

## Design

- Register one `command_check` capability in `capability_catalog`. Its closed
  parameters are `argv`, the literal cwd value `workspace`, and an `expect`
  table containing required `exit_code` and `max_bytes` plus optional
  `stdout_regex`.
- Resolve the declaration to a typed value, not a shell string. Reject empty or
  oversized argv, shell/interpreter-eval escape hatches, setup/destructive
  commands, absolute or parent-relative paths, invalid regular expressions,
  unknown expectation fields, and output limits outside the compiled bound.
- Put process execution, fixed timeout, bounded output projection, result
  aggregation, event emission, and summary rendering in the new leaf module
  `src/planner/declarative_command_checks.rs`.
- Retain resolved draft-profile command bindings at registration. Execute them
  from final acceptance with minimal wiring and merge failures into the normal
  verification report. Existing internal and shell-check behavior remains
  unchanged.
- Permit only a draft local pack's additive `command_check` at
  `final_acceptance`; existing admitted pack floors and typed internal pack
  checks remain unchanged. Reuse the leaf executor and include its counts in
  the pack summary.
- Add additive fields to existing final-acceptance events and emit one
  `declarative_command_check_result` event per execution. Append a concise
  command-check section to `summary.md`.

## Assurance boundary

Command-check pass/fail affects acceptance only. It does not produce a
capability, evidence tier, admission status, or assurance formula input. The
existing profile admission cap remains the sole draft assurance projection,
and tests will assert `static` / `profile_not_admitted` after passing checks.

## Tests

- Catalog/schema tests cover positive resolution and hostile declarations,
  including free-form shell/eval, workspace escape, malformed expectations,
  and unknown fields.
- Leaf tests cover pass, exit-code failure, regex failure, timeout/output
  projection, event emission, and summary rendering.
- Integration/corpus coverage extends the Issue #249 draft-profile/local-pack
  fixture so both supply paths declare checks, results appear in telemetry and
  summary, and assurance does not promote.
