# Post-b68b9ed Stale Edit Rust Modify Check

Date: 2026-06-17

Root:

```text
eval/runs/stale-edit-target-rust-modify/20260617T230748
```

Binary:

```text
target/release/commandagent
```

Commit:

```text
b68b9ed74a9daa6ad3ddd95d5d5165a66068128d
```

The run recorded `dirty: false`.

## Summary

```text
case_id                run rc elapsed_ms success reason
large-rust-app-modify  1   1  443768     false   rc:1
```

## Change Under Test

The stale Edit-target evidence path was clarified:

- `edit target was not found` is now classified as `edit_target_not_found`
  instead of generic `turn_error`.
- The diagnostic tells the next repair turn that the file state is stale and
  that it must `Read` or `Glob` before using `Edit` again.
- The repair prompt adds a conditional `Repair focus` section for this reason.
- Repair budgets and retry counts were not increased.

## Finding

This run did not reproduce the stale Edit-target failure. The run failed earlier
in a different class.

Generated files included:

```text
src/main.rs
src/lib.rs
src/cli.rs
src/errors.rs
```

The failing step was:

```text
create-commands-module
```

Missing expected path:

```text
src/commands.rs
```

Repair evidence:

```text
assistant violated final answer contract: Let me read the current files...
assistant violated final answer contract: Let me read the `cli.rs` file...
missing_artifact_no_tool: The required path is still missing: src/commands.rs...
assistant violated final answer contract: Now let me read the cli.rs file...
assistant violated final answer contract: Let me also read `errors.rs`...
```

Classification:

- Not stale Edit-target in this run.
- Residual class is missing artifact plus repeated no-tool final-boundary
  violations inside a repair context.
- The existing missing-artifact guard fired once, as intended, but the model did
  not recover into a tool call.

## Design Check

The `b68b9ed` change remains aligned with the stability-first design:

- deterministic trigger: specific tool error text
- bounded effect: evidence/prompt only; no extra turns
- stable scope: common repair evidence, not Rust-specific runtime repair
- observable outcome: reason becomes `edit_target_not_found`
- provider-independent: applies to any provider/tool call that reports stale
  Edit target failure

## Next Action

Do not broaden the missing-artifact guard based on this single run. The next
triage should inspect why the generated step asks to create `src/commands.rs`
but the model repeatedly responds with natural-language inspection promises
instead of `Read`, `Glob`, or `Write` calls.

Likely areas to inspect:

- phase decomposition for Rust modify
- whether `create-commands-module` should be split into inspect and create
  steps with clearer expected results
- whether the repair prompt over-emphasizes inspection when the missing path is
  already deterministic

