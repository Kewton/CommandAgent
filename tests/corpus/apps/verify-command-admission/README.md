# verify-command-admission (Issue #952)

Synthetic corpus case for the planner verify admission of Issue #952. It is
not a capability measurement and makes no claim about any model.

## What it pins

- A `verify` entry is a command to run. An entry that reads as a sentence or
  a deferral, in English, Japanese, or mixed scripts, is a
  `verify_command_shape` lint error before anything runs. The base
  (`04e262c8`) exempted any first word with an ASCII letter, digit, `.`, `/`,
  `_`, or `-`; the `base_verdict` of each negative case records that.
- Commands that may execute are left to the runtime gate: shell builtins,
  executables on a pinned PATH, relative, absolute, and non-ASCII paths,
  scripts produced by the same or an earlier non-Verify step, tools an earlier
  setup step installs, catalog checks, and commands not on PATH yet. A command
  with the same first word as a sentence passes when that word resolves.
- A producer or setup step placed after its use is rejected by the existing
  order lints (`verify_semantics`, `dependency_order`), not by this one.
- The planner repairs a rejected sentence within the existing three
  attempts; when the later attempts carry another lint error the run stops
  with no fallback plan. Only a lint-passing plan can become the fallback.
- A host-owned completion contract check stays the obligation: a repair that
  deletes it, empties it, or swaps in `true`, `echo`, or a weaker check never
  satisfies it.

## Event contract

No event is added or changed. A rejected sentence produces the existing
`planner_error` event (`planner_stage` `lint`, `planner_error_kind`
`planner_lint_error`) with the same keys and message shape as the base
produced for a non-ASCII sentence (`base_lint_message`,
`base_planner_error_keys`). The order is diagnosis, then the bounded repair or
the stop, then execution (`event_order`).

## Provenance

Every sentence, path, and command is hand-written for this case. No benchmark
task, grader input, expected answer, generated code, run record, live model,
real key, external provider, or host path is stored here. The fixture is read
by `tests/verify_command_admission.rs`; its digest is in
`source-manifest.json` (`shasum -a 256 fixtures/events.json`).
