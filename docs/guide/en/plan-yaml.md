# Plan YAML editing

[日本語](../ja/plan-yaml.md) | [Guide index](../README.md) | [CLI reference](cli-reference.md)

`--plan-steps` and `--ultra-plan` save commented YAML designed to be reviewed
and edited before execution. Comments explain the fields and are ignored by
the YAML parser. The saved path remains the first output line, followed by the
exact validation and run commands for that file.

## Edit, validate, run

Generate a plan, edit the printed path in your text editor, validate it without
executing anything, and only then run it:

```bash
commandagent --plan-steps Update the documentation
commandagent --validate-plan .anvil/plans/plan-<id>.yaml
commandagent --run-plan .anvil/plans/plan-<id>.yaml
```

For an UltraPlan, use `--ultra-plan` to generate it and
`--run-ultra-plan <PATH>` after validation. `--validate-plan` is offline and
read-only: it does not initialize a provider client or execute a plan.

## Editable fields

Keep the document as one top-level mapping with either `steps` or `phases`, not
both. YAML comments beginning with `#` can be retained, changed, or removed.

| Field | Plan | Editing contract |
| --- | --- | --- |
| `goal` | both | Non-empty overall outcome. |
| `steps` | step | Ordered, non-empty step list. |
| `id` | both | Unique identifier; step IDs use lowercase kebab-case. |
| `kind` | step | `inspect`, `setup`, `implement`, `verify`, or `report`. |
| `expected_result` | step | `pass` or `fail`. |
| `instruction` | step | Focused natural-language instruction, not a shell command. |
| `expected_paths` / `verify` | step | YAML string lists; empty lists are allowed. Verification commands must satisfy the execution safety policy. |
| `profile` / `style` / `intent` | ultra | Preserve the generated execution context unless you intentionally change it. |
| `phases` / `prompt` | ultra | Ordered list of focused phase tasks; each phase needs a unique ID and a non-empty prompt. |

Quoting is recommended for strings containing `:`, `#`, brackets, or leading
punctuation. The generated template already uses safe quoting.

## Validation diagnostics

Success identifies the plan type and prints the next execution command:

```text
Valid step plan: /workspace/.anvil/plans/plan-<id>.yaml
Next: commandagent --run-plan /workspace/.anvil/plans/plan-<id>.yaml
```

Failure exits nonzero and reports every available source location as
`path:line:column: reason`. Syntax and field-type errors come from the YAML
parser; semantic errors use the same plan lint rules as execution. Fix every
reported error and run `--validate-plan` again. Validation is intentionally at
least as strict as loading the plan for execution.

## Recovery plans

Recovery UltraPlans remain compatible with `--run-ultra-plan`. Their leading
comments summarize the bounded difference for the next attempt:

- changed paths to retain;
- missing paths and capabilities;
- repair targets; and
- deterministic checks to rerun.

The diff is informational and comment-only; it does not add executable fields
or change recovery metadata. A successful `--validate-plan` also prints the
recorded failed scope, failure kind, retained artifacts, and the exact
`--run-ultra-plan` command.

## Troubleshooting

- If the validator says the document has both `steps` and `phases`, keep only
  the shape you intend to run.
- If an error points at a `verify` entry, split shell pipelines or chained
  commands into separate deterministic checks accepted by the plan policy.
- If a legacy hand-written plan runs but does not validate, add the explicit
  fields shown in the current commented template before editing it further.
- Use [CLI conflicts and combinations](cli-reference.md#conflicts-and-combinations)
  when another action flag conflicts with `--validate-plan`.
