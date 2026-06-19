# Add Recovery Task Contract

Date: 2026-06-19

## Input Evidence

The latest focused Gemini/Next.js evaluation showed bounded failures reaching
`verify-build` with structured contract evidence and repair focus. That
improved attribution, but the repair turn still depended too much on the model
inferring the next repair task from evidence prose.

The observed gap is:

```text
deterministic failure -> structured evidence -> model infers repair action
```

The target shape is:

```text
deterministic failure -> structured evidence -> recovery task contract
  -> bounded minimal-loop repair turn -> original guard/verifier rerun
```

## Change

CommandAgent now has a small `RecoveryTaskContract` data boundary under the
step runner. It can be derived conservatively from deterministic
`ContractEvidence` for these current producers:

- `tool_protocol`
- `step_policy`
- `verifier`
- `profile_verification`

The rendered repair prompt or saved repair packet now includes a `Recovery
task` section before `Repair focus` and `Contract evidence` when enough
deterministic information exists. The section may include:

- source
- failed step
- contract code
- blocker
- required action
- repair target
- candidate artifacts
- allowed tools
- disallowed actions
- success check
- evidence signature

Tool protocol correction prompts also render a recovery task that names the
valid tool call required by the current schema failure.

## Boundary

This is not a new engine.

The recovery task contract:

- does not increase retry budgets
- does not auto-resume failed ultra phases
- does not run dependency setup on its own
- does not change verifier commands
- does not add provider/model-specific behavior
- does not turn profiles into workflow engines

The minimal loop still executes one bounded task. The original verifier, guard,
tool schema, step policy, or profile check remains the success authority.

## Validation

Focused tests added or updated:

- `cargo test recovery_task`
- `cargo test repair::tests`
- `cargo test repair_loop::tests`

The tests cover:

- conservative conversion from contract evidence to recovery task
- verifier task rendering with original command as success check
- profile route-integration task rendering with selected route target
- mixed app-root evidence without arbitrary repair target
- tool protocol correction rendering with allowed tool and schema success check
- read-only step-policy rendering without authorizing mutation

Local verification also passed with:

- `cargo fmt --check`
- `cargo test`
- `cargo build --release`
- `scripts/eval_smoke.sh`

## Focused UAT

A single Gemini/Next.js focused UAT was run from:

- root: `/private/tmp/commandagent-uat-recovery-task-20260619-01`
- provider/model: `gemini` / `gemini-3.1-flash-lite`
- planner model: `gemini-3.5-flash`
- binary: `target/release/commandagent`

Result:

- bounded failure at `inspect-package`
- failure class: `step_policy:read_only_step_mutation`
- repair packet:
  `.commandagent/repairs/repair-inspect-package-1781879486525.md`

The saved repair packet included a `Recovery task` section:

- source: `step_policy`
- failed step: `inspect-package`
- contract code: `read_only_step_mutation`
- blocker: `Step tool policy rejected Write`
- required action: use read-only tools in inspect/report steps and move
  mutation into create/edit/repair steps
- disallowed actions: `Write`, `Edit`, and mutating `Bash` in the read-only
  step
- success check: step tool policy

This confirms the targeted behavior for this slice: deterministic step-policy
evidence is translated into a first-class recovery task in the saved repair
packet. It does not claim completion of the original Next.js app task.

## Evaluation Reading

A future UAT should not be reported as fixed merely because `Recovery task`
appears. The expected improvement is clearer bounded repair input:

- If the same evidence signature repeats, report non-convergence in the same
  class.
- If the signature changes, report the new independent failure class.
- If a Next.js app still looks poor, report that as app/game quality even if
  build or profile repair attribution improved.
- If no recovery task is rendered, report that the deterministic evidence was
  too broad to form one.
