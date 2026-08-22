# Issue #250 implementation summary

## Implemented

- Integrated the verified predecessor commits for Issues #239 and #249 before
  editing Issue #250-owned behavior.
- Registered `command_check` in the capability catalog with a closed schema:
  direct string `argv`, literal `cwd = "workspace"`, and a typed `expect`
  table containing `exit_code`, optional `stdout_regex`, and `max_bytes`.
- Added the leaf executor in
  `src/planner/declarative_command_checks.rs`. It rejects shell strings,
  interpreter evaluation, setup or mutating programs, and workspace escapes;
  applies the shared verify policy; runs with a fixed timeout and normalized
  environment; bounds recorded output; and preserves exit, regex, timeout, and
  output-limit failures.
- Wired external draft profiles and draft-profile local packs to execute these
  checks only at final acceptance. Both paths emit
  `declarative_command_check_result` and append results to `summary.md`.
- Kept command-check results outside capability evidence, admission, and
  assurance computation. Passing checks do not change the existing
  `static` / `profile_not_admitted` cap for draft profiles.
- Added pack nested-parameter conversion, vocabulary and conformance rules,
  capability golden output, developer documentation, and a registered
  protection-audit boundary.

## Coverage

- Unit tests cover schema closure, free-form shell rejection, interpreter and
  mutating-command rejection, workspace escape, final-only declarations,
  success telemetry, honest failures, timeout, output bounds, and static
  assurance.
- Pack runtime coverage exercises an operator-local draft-profile pack and
  rejects a shell-interpreter declaration.
- `tests/corpus/apps/issue250-declarative-command-checks/` and the Issue #250
  integration test cover draft-profile and local-pack declarations, event and
  summary contracts, exact local-pack pinning, and non-promotion of assurance.
