# Post-ac4e833 Rust Missing Path Contract Check

Date: 2026-06-18

Commit:

```text
ac4e833a144c31e5f8c00873ae5dfd31321ed2d0
```

The eval metadata recorded `dirty: false` and used:

```text
target/release/commandagent
model: qwen3.6:27b-coding-nvfp4
provider: ollama
```

## Change Under Test

T1/T2 from `rust-modify-missing-artifact-no-tool-work-plan.md`:

- `execute_step` now computes missing `expected_paths` with the existing
  `missing_paths(self.cwd, &step.expected_paths)` helper.
- `step_prompt` receives those computed paths and shows a short missing path
  contract only for `create`, `edit`, and `repair` steps.
- `inspect`, `verify`, and `report` steps do not receive the hint.
- No repair turn count, retry count, plan lint, profile-specific Rust logic, or
  missing-artifact guard behavior was changed.

Verification before eval:

```text
cargo test
cargo build --release
```

Result: all 182 tests passed and release build completed.

## Focused Eval Roots

```text
eval/runs/rust-modify-missing-path-contract/20260617T235542
eval/runs/rust-new-missing-path-contract/20260618T000711
```

## Summary

```text
large-rust-app-modify  run=1  rc=1  success=false  reason=rc:1  elapsed_ms=554848
large-rust-app-new     run=1  rc=1  success=false  reason=rc:1  elapsed_ms=596765
```

## Rust Modify Finding

The target failure changed class.

Previous target class:

```text
missing path: src/commands.rs
assistant repeatedly described reading/creating work without a tool call
```

This run generated the required final artifacts, including:

```text
Cargo.toml
src/main.rs
src/lib.rs
src/cli.rs
src/config.rs
```

The final failure moved to `verify-build`:

```text
error[E0432]: unresolved import `cli::Cli`
source: src/lib.rs:4
```

Manual verifier reproduction showed the concrete compile issue:

```text
error[E0432]: unresolved import `config`
 --> src/cli.rs:5:9
  |
5 | pub use config::Config;
  |         ^^^^^^ help: a similar path exists: `crate::config`
```

Classification:

- The missing-artifact + no-tool residual for Rust modify did not reproduce.
- The run reached a later compile-error class.
- This satisfies Gate 1 for the specific target issue: do not proceed to T3/T5
  based on this run.

## Rust New Finding

Rust new also created the required artifacts:

```text
Cargo.toml
src/main.rs
```

The failure was a compile error followed by stale Edit-target repair attempts:

```text
error[E0308]: mismatched types
source: src/main.rs:93
```

The repair prompt recorded repeated `edit_target_not_found` entries:

```text
Edit target was not found. The file state is stale for this Edit attempt.
Read or Glob the current file first, then use Edit only with exact current target text from this repair turn, or Write when full replacement/creation is safer.
```

Classification:

- This is not missing artifact + no-tool.
- It is compile failure plus stale Edit repair behavior.
- Because this is one stochastic run, non-degradation is not proven. However,
  the observed failure is not in the mechanism changed by `ac4e833`.

## Decision

Hold T3/T5 for now.

Rationale:

- The target Rust modify missing path residual moved to a later compile failure.
- Adding repair prompt focus or plan generation prompt changes now would mix
  effects and weaken attribution.
- The remaining visible failures are better handled under the existing R5/R6
  residual categories: compile-quality repair and stale Edit-target repair.

Next recommended action:

- Treat Rust modify as past the missing-artifact/no-tool gate for this cycle.
- Continue R5/R6 triage on stale Edit-target and compile-error repair quality.
- If future seeded repeats reintroduce the exact missing-artifact/no-tool class,
  revisit T3 as a separate commit and eval.
