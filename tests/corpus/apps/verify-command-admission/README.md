# verify-command-admission (Issue #952)

Synthetic corpus case for the planner verify admission of Issue #952. It is
not a capability measurement and makes no claim about any model.

## The rule (PM decision, plan A)

A `verify` line is a `verify_command_shape` lint error as prose only when its
first word resolves to nothing (no shell builtin, no command on the planning
PATH, no existing or declared workspace path, no `#` comment) and one of two
shapes holds:

- the first word has no ASCII letter, digit, `.`, `/`, `_`, or `-`, so it
  cannot be a command name (the base rule, which keeps Japanese sentences
  rejected); or
- the line is a sentence: at least three words, every raw word (as written,
  before quotes are removed or fragments joined) is letters only, and the last
  word closes with `.`, `!`, `?`, `。`, `！`, or `？`.

A quote, glob character, digit, flag, path, `~`, `=`, `$`, backslash, comma,
or operator anywhere keeps a line out of the sentence shape. The base
(`04e262c8`) exempted any first word with an ASCII letter, digit, `.`, `/`,
`_`, or `-`; the `base_verdict` of each case records that.

## What it pins

- `negative`: sentences and deferrals in English (two share the shape of the
  smoke5 failures, a `Deferred` and a `Validate` sentence, without their
  text), Japanese, and Japanese with an ASCII word are rejected before
  anything runs.
- `positive`: commands that may execute pass lint: builtins, executables on a
  pinned PATH, a command with the same first word as a sentence when that word
  resolves, relative, absolute, and non-ASCII paths, scripts produced by the
  same or an earlier non-Verify step, tools an earlier setup step installs,
  commands not on PATH yet, capitalized names with plain arguments, glob,
  quoted, and comma arguments, Latin and mixed-script names, and quoted,
  piped, redirected, and `&&` lines that the existing normalizer accepts.
- `left_to_runtime`: the known limit of plan A, and a gap against the Issue
  text. Prose without the sentence shape (no closing mark, a comma, two words,
  one mixed-script word) passes lint and fails at runtime with exit 127 and
  the existing `deterministic_verify_command_bug` classification. It is never
  a success.
- `order_controls`: a producer or setup step placed after its use is
  rejected by the existing order lints (`verify_semantics`,
  `dependency_order`), not by this one; placed before, it passes.
- The planner repairs a rejected sentence within the existing three attempts;
  when the later attempts carry another lint error the run stops with no
  fallback plan. Only a lint-passing plan can become the fallback.
- `obligation`: with a fixed host-owned completion contract, each weak
  repair (the check step deleted, an empty verify, `true`, `echo`, a weaker
  check) goes through the public generate-and-run entry with a scripted
  executor that writes a draft artifact. The run fails every time: the empty
  verify is refused by the planner within three attempts, the others stop on
  the host-owned check. Only the repair that keeps the host check succeeds,
  and only with an artifact that satisfies it.

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
by `tests/verify_command_admission.rs`, which runs every case that resolves
commands under a pinned PATH; its digest is in `source-manifest.json`
(`shasum -a 256 fixtures/events.json`).
