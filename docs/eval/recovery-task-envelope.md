# Recovery Task Execution Envelope

Date: 2026-06-19

## Baseline

Focused Gemini/Next.js UAT:

- root: `/private/tmp/commandagent-uat-recovery-task-20260619-01`
- provider/model: `gemini` / `gemini-3.1-flash-lite`
- planner model: `gemini-3.5-flash`
- failure step: `inspect-package`
- failure class: `step_policy:read_only_step_mutation`
- packet:
  `.commandagent/repairs/repair-inspect-package-1781879486525.md`

The previous slice rendered a `Recovery task` section with the correct
step-policy evidence, but the repair execution still used the generic
file-changing repair configuration:

- `ActionRequirement::Required`
- `StepToolPolicy::FileMutationAllowed`

That meant the contract text said read-only while the next repair turn was
configured as mutation-allowed and required file-change evidence.

## Change

Recovery Task Contract now carries a deterministic execution envelope when the
failure class is specific enough:

- `read_only_evidence`
  - source: `step_policy:read_only_step_mutation`
  - tool policy: `read_only`
  - evidence requirement: `repository_read_evidence`
- `file_mutation_repair`
  - source: verifier/profile repair that needs source or config edits
  - tool policy: `file_mutation_allowed`
  - evidence requirement: file change or explicit blocker
- `tool_protocol_correction`
  - source: tool schema/protocol failure
  - evidence requirement: valid tool call

The repair runtime consumes the selected envelope before calling the minimal
loop. For read-only recovery it sets:

- `StepToolPolicy::ReadOnly`
- `ActionRequirement::RepositoryEvidenceRequired`

The minimal loop now treats successful `Read`, `Glob`, `Grep`, and read-only
`Bash` calls as repository evidence under that requirement. Prose-only output
does not satisfy the requirement.

## Boundary

This is not a retry expansion or a new recovery engine.

- The envelope is selected from deterministic contract evidence.
- The envelope constrains the existing Execution Contract.
- The original guard/verifier/profile check remains authoritative.
- Retry budgets and stop behavior are unchanged.
- Provider/model-specific branches were not added.

## Validation

Local checks passed:

- `cargo fmt --check`
- `cargo test recovery_task`
- `cargo test minimal_loop`
- `cargo test repair::tests`
- `cargo test repair_loop::tests`
- `cargo test`
- `cargo build --release`
- `scripts/eval_smoke.sh`

## Focused UAT

Focused Gemini/Next.js UAT after the final release build:

- root: `/private/tmp/commandagent-uat-recovery-envelope-20260619-02`
- provider/model: `gemini` / `gemini-3.1-flash-lite`
- planner model: `gemini-3.5-flash`
- binary: `target/release/commandagent`

Result:

- The previous `inspect-package` `step_policy:read_only_step_mutation` failure
  did not repeat in this run.
- The run stopped later at profile verification after phase `verify-and-test`.
- Failure class:
  `profile_verification:nextjs_route_not_integrated`.
- Packet:
  `.commandagent/repairs/repair-profile-verify-and-test-1781881162891.md`

The saved packet includes:

- `execution_envelope: file_mutation_repair`
- `tool_policy: file_mutation_allowed`
- `evidence_requirement: file_change_or_explicit_blocker`
- `repair_target: app/page.tsx`
- `candidate_artifacts: app/page.tsx, next-env.d.ts`

This means the current slice moved past the previously observed read-only
execution-envelope mismatch. The remaining failure is a separate Next.js
profile contract issue: `next-env.d.ts` was treated as an explicit artifact
that must be referenced from the selected route. That should be triaged as a
profile obligation/artifact-classification problem, not as read-only recovery
execution mismatch.

Future UAT reading:

- If `inspect-package` fails again with `read_only_step_mutation`, the packet
  should include `execution_envelope: read_only_evidence`,
  `tool_policy: read_only`, and
  `evidence_requirement: repository_read_evidence`.
- If the repair turn calls `Read`, the turn should not fail only because no
  file changed.
- If the repair turn answers in prose, the failure should explicitly say
  repository read evidence is missing.
- Later Next.js build, visual quality, or app-quality failures are separate
  classes and should not be reported as this envelope mismatch.
