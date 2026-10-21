# Issue #429 reopened verification

- Status: `passed`

## Checks

- `cargo test --lib issue429`: `passed`
- `cargo test --lib recovery_observation_policy`: `passed`
- `cargo test --lib json_preflight`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `diff -r /Volumes/SSD_NX/tmp/commandagent-capability-0905.vkR3MS/S1/workspace/src tests/corpus/apps/issue429-lazy-json-preflight/fixtures/campaign/S1/src`: `passed`
- `diff -r /Volumes/SSD_NX/tmp/commandagent-capability-0905.vkR3MS/S3/workspace/src tests/corpus/apps/issue429-lazy-json-preflight/fixtures/campaign/S3/src`: `passed`
- `diff -r /Volumes/SSD_NX/tmp/commandagent-capability-0905.vkR3MS/E3/workspace/src tests/corpus/apps/issue429-lazy-json-preflight/fixtures/campaign/E3/src`: `passed`
- `git diff --check`: `passed`
- `git diff --cached --check`: `passed`

Final verification completed on 2026-09-07. The full `cargo test` exited 0
outside the sandbox, allowing the existing local HTTP fixtures to bind sockets.
The library suite passed 2,395 tests with 16 default ignores; all default
integration and doc-test suites passed. Ignored/live suites were not enabled.
No Python harness changed, so Ruff and pytest were not required.

## Acceptance evidence

- Seven reopened tests pass. The complete S1/S3/E3 source trees (29 files) are
  unchanged copies; every fixture hash is checked on load and recursive diffs
  against the originals are empty. Source::parse succeeds for the actual S1
  staff/shifts routes, S3 storage, and E3 generic writer helper.
- S1/S3 register exactly data/shifts.json and data/staff.json. E3 registers
  exactly data/departments.json and data/expenses.json. Registration is identical
  before and after JSON creation, and excludes unrelated siblings. Required and
  protected destinations remain excluded.
- The isolated real-source effect replay passes all 24 combinations: three
  source trees, missing/existing outputs, and success/business failure/source
  mutation/unregistered output. Registered creation/update alone does not cause
  Unavailable. Business failure remains Failed and starts Recovery; success and
  prohibited mutations do not start a repair. Every original-session source
  hash and original data file remains unchanged.
- The existing policy suite passes 21 tests and the preflight suite passes six.
  The executable synthetic GET still lazily creates staff/shifts data and retains
  independent business failure. Source/config mutation, unregistered paths,
  traversal, symlink escape, new symlinks, directories masquerading as files,
  cwd changes, and weak authority remain rejected.
- Hidden assignments/shadowing inside nested interpolation do not authorize
  outputs. Six corpus cases reject eval, Function, new Function, global eval,
  constructor-based evaluation, and an eval alias. Slash tests cover real regex,
  division, control statements, escaped delimiters, malformed literals, postfix
  and TS ambiguity, automatic-semicolon insertion, and JSX writer lookalikes.
  Unsupported forms fail closed; literal text never becomes writer evidence.
- Expected event projections compare the existing policy fields. Actual driver
  events retain pre_recovery pass/fail/unavailable statuses, isolated observation,
  product-visible contract authority, and no external oracle. fixture_run is a
  fixture selector only; the product event schema was not changed.

## Development findings and limits

Initial focused runs exposed S1's condition/parameter confusion and a test that
looked for the driver event before invoking the driver. Both were corrected.
Clippy caught a mutable range-bound variable in argument scanning; the loop now
uses its immutable opening index. The first full run caught test-only profile
string literals through the existing boundary guard; tests now use PROFILE_ID.
All final checks above were rerun successfully; no baseline or gate was weakened.

Predecessor #420's committed implementation and passed report were inspected,
then incorporated by fast-forward to
`e7ecb137249382fa50c9c889a313c3e4b09a9f6c` before implementation. Its #435
parent and the previous #429/#425 protection behavior remain included.

Real-source preflight tests use a synthetic observer to replay filesystem
creation/update and business outcomes; they do not execute the generated Next.js
applications. The original plain-JS synthetic GET fixture does execute its lazy
writers. These results are not a fresh campaign, generated-app production build,
browser measurement, domain-integrity result, or proof of general JS/TS analysis.
Ambiguous syntax and unsupported dynamic path/forwarding forms may still produce
conservative false negatives. Acceptance and promotion remain independently
required. No live nine-run campaign was run.

Historical evidence, original generated workspaces, credentials, and the live
.anvil namespace were not changed. No push, PR, CommandMate operation, or Issue
lifecycle mutation was performed. Raw verification logs remain outside Git.
