# Issue 387 Design

## Scope

Add an opt-in `--recovery-plan-auto-runs <0..=20>` control to shared confirmed
execution configuration and apply it to every UltraPlan execution entry point:
top-level generate-and-run and file-run actions, REPL `/ultra-plan-run`,
`/run-ultra-plan`, and `/resume`. Expose the same value in GUI Trial Gate 1.
The initial run is not counted. The default and explicit value `0` retain the
existing execution branch, event stream, serialized confirmation identity, and
confirmation hash.

## Design

- Parse the CLI value as a bounded integer with default `0` and store it in
  `Config`, so direct actions, confirmed REPL dispatch, and resume cannot
  silently substitute zero. Keep it outside minimal-loop and local-repair
  budgets.
- Put retry policy in a new typed `planner::auto_recovery` leaf. For positive
  limits it wraps the existing UltraPlan runner and consumes a typed
  per-attempt outcome. Recovery-plan creation records a typed candidate only
  inside the current attempt; the controller never discovers candidates from
  event files, rendered commands, stop-reason strings, or arbitrary errors.
- Before every automatic run, reuse `runs::prepare_resume` for workspace/path
  confinement, YAML parsing, recovery metadata, and workspace-drift checks,
  with typed prevalidation for missing/invalid/review-required/path-escape
  candidates. Interruption comes from the typed attempt state. Detect cycles
  from the normalized parsed `UltraPlan` content, excluding recovery comments,
  metadata, paths, and YAML formatting.
- Emit additive `recovery_plan_auto_run_*` lifecycle events only on the
  positive-limit branch. They carry the configured limit, used/current count,
  and stop reason. The final error remains the newest honest runner failure and
  includes the existing manual Recovery Plan guidance.
- Add the GUI value to `SessionSpec` and to `ConfirmationIdentity`, omitting it
  from serialization when zero. Gate 1 rendering displays it. Nonzero values
  therefore participate in the normal identity hash; zero continues through
  the legacy hash path unchanged. Delegation reads the persisted confirmed
  identity, not the mutable request, before adding the CLI flag.
- Gate 1 displays both `N` automatic Recovery executions and the maximum total
  plan executions `1 + N`, explicitly describing this as the duration/cost
  upper-bound multiplier. Project retry lifecycle state into session status so
  Gate 2, terminal, and history detail can share one compact display component.
- Treat a successful auto-recovery event as a recovery-field reset only until a
  newer recovery-bearing event is encountered. A later manual continuation
  failure therefore remains visible.

## Compatibility and safety

- No existing event is renamed or changed; the zero path emits no new events.
- Existing confirmation JSON remains readable because the new field defaults
  to zero and is omitted at zero.
- Recovery documents remain on the existing read-only endpoints. Automatic
  execution is possible only through confirmed CLI delegation.
- No changes are made to `max_iterations`, local repair, acceptance,
  verification, evidence, or release gates. Runner chokepoints receive only
  minimal wiring; policy and tests live in leaf modules.

## Verification

Add focused behavioral tests for zero/initial success, one-step recovery
success, exact retry caps, typed non-recoverable/invalid stops, normalized
cycles, all direct/REPL/resume routes, GUI proposal/delegation/hash behavior,
and the later-manual-recovery projection boundary. Keep the corpus and both
browser base-path checks, then run repository-wide Rust and GUI verification.
