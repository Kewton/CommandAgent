# Maintainability boundaries

This document records the seams introduced by the 2026-08-07 maintainability
refactor. They are intended to keep orchestration code small without changing
the CLI, planner API, event schema, stop reasons, or runtime namespace.

## Planner phase lifecycle

`runner/phase/state.rs` owns the sixteen reviewed states.
`runner/phase/transition.rs` is pure and rejects illegal terminal transitions.
`runner/phase/effects.rs` is the only adapter used by the existing effectful
flow. StepPlan execution and phase-boundary cleanup live in leaf modules.

The transition layer must not perform provider, filesystem, process, browser,
or network I/O. Event ordering remains guarded by the two-phase lifecycle
fixture.

## Minimal loop

The loop's immutable policy/configuration is in `loop_run/context.rs`, mutable
recovery state is in `loop_run/state.rs`, structured errors are in
`loop_run/error.rs`, and normalized Bash execution effects are in
`loop_run/runtime_bash_effects.rs`. These modules may depend on leaf policy
modules but must not invent success or weaken verification.

## Event compatibility

`eval_events::emit(Option<&Path>, serde_json::Value)` remains the public,
backward-compatible boundary. Internal high-volume phase lifecycle producers
serialize the schemas in `eval_events/typed.rs`; equivalence tests freeze their
legacy JSON shape. New internal events should prefer a typed payload while
retaining schema version 1 unless a migration is explicitly authorized.

## Browser probe asset

The Playwright program is stored at
`src/minimal_loop/assets/interaction_probe.js` and embedded with `include_str!`.
Its SHA-256 regression test detects accidental byte drift independently from
Rust formatting.

## Verification entry point

Both GitHub Actions workflows and `just ci` execute `scripts/ci.sh`. Python
tool versions are pinned in `requirements/ci.txt`; intentional ignored Rust
tests and their opt-in paths are listed in `docs/dev/ci-ignored-tests.md`.
The Ruff rule set is explicit and isolated from machine-level configuration,
and CI validates only Git-tracked skills so an unrelated local skill draft
cannot make the repository gate nondeterministic.

The P2F settlement tests inject the immutable production-tree pin recorded by
the campaign when exercising settlement calculations. A separate negative
test proves that the production code still rejects a mismatched pin; no
historical evidence file is rewritten to accommodate current source changes.
