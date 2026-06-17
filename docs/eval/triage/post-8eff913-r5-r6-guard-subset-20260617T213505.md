# Post-8eff913 R5/R6 Guard Subset

Date: 2026-06-17

Root:

```text
eval/runs/r5-r6-guard-subset/20260617T213505
```

Binary:

```text
target/release/commandagent
```

Commit:

```text
8eff913b6b8491a85af0407727c5bc2a5a6cff6f
```

All recorded runs have `dirty: false`.

## Summary

```text
case_id                  run rc elapsed_ms success reason
large-nextjs-app-modify  1   1  386863     false   dependency_missing
large-rust-app-modify    1   1  362055     false   rc:1
large-rust-app-new       1   0  931991     true    ok
```

## Changes Under Test

- Step-runner repair evidence now adds a one-shot `missing_artifact_no_tool`
  guard when a repair turn ends with no tool call while required step paths are
  still absent.
- Rust profile contract now states that `CARGO_BIN_EXE_<name>` references must
  match the Cargo binary name defined by `Cargo.toml`.
- The eval runner already classifies `dependency_missing` separately from
  generic `rc:1` failures.

## Case Findings

### large-nextjs-app-modify

Result:

```text
success=false reason=dependency_missing
```

Artifacts existed:

- `package.json`
- `app/page.tsx`
- `components/AnalyticsPanel.tsx`

Failure evidence:

```text
npm run build requires node_modules/.bin/next, but it is missing
```

Classification:

- Not an implementation-quality failure under the current run environment.
- This remains an eval/profile dependency setup boundary.

Next action:

- Keep `dependency_missing` separate.
- If build quality is the target metric, run a dependency-preinstalled eval or
  explicitly allow dependency setup in a setup phase.

### large-rust-app-modify

Result:

```text
success=false reason=rc:1
```

Important improvement:

- The previous missing-artifact/no-tool pattern did not recur.
- The run created `src/commands/mod.rs`, `src/commands/run.rs`, and
  `src/commands/status.rs`.

Remaining failure evidence:

```text
error[E0432]: unresolved import `status::status`
error[E0432]: unresolved import `run::run`
error[E0599]: no method named `iter` found for struct `RunArgs`
error[E0282]: type annotations needed
```

Repair then hit:

```text
assistant violated final answer contract: ... Let me read it again ...
tool error: edit target was not found
tool error: edit target was not found
```

Classification:

- The missing-artifact guard appears to have moved the run past the prior
  failure class.
- Remaining failure is implementation-quality plus stale Edit-target repair.
- This is not a reason to broaden the missing-artifact guard.

Next action:

- Treat as R6 repair affordance residue, specifically stale Edit target after
  compile errors.
- Do not add Rust-specific runtime repair logic.
- If addressed, prefer a narrow common repair rule around Edit-target misses
  with Read/Glob-first evidence, which is already partially represented in the
  repair prompt.

### large-rust-app-new

Result:

```text
success=true reason=ok
```

The generated `Cargo.toml` used a stable binary name:

```toml
[[bin]]
name = "cli-app"
path = "src/main.rs"
```

The prior `CARGO_BIN_EXE_cli` mismatch did not recur in this run.

Classification:

- The Rust naming contract is directionally effective for this slice.
- This is only `runs=1`; treat as a positive smoke result, not a stability
  claim.
- The current eval contract checks required artifacts and process success. If
  unit-test presence or semantic CLI behavior is part of the release bar, the
  large Rust case needs a stricter semantic check.

## Design Check

The changes stay aligned with the CommandAgent design:

- The guard is bounded and repair-scoped.
- The guard is based on deterministic missing paths, not model intent guessing.
- The Rust update is a profile fact contract, not runtime Rust-specific repair.
- No sidecar, unbounded retry, or legacy repair coordinator was reintroduced.

## Remaining Work

1. Decide whether `large-rust-app-modify` should get a separate, narrow stale
   Edit-target recovery improvement or remain a structured failure.
2. Decide whether large Rust eval should require semantic checks for tests and
   CLI behavior beyond `Cargo.toml` and `src/main.rs`.
3. For Next.js build-quality scoring, run a dependency-preinstalled variant or
   explicitly allow dependency setup in setup steps.

