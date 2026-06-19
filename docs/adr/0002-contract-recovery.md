# ADR 0002: Contract Recovery Without Legacy Mechanisms

Date: 2026-06-17

## Status

Accepted.

## Context

The CommandAgent migration intentionally did not copy the historical Anvil
legacy engine, case memory, sidecar routing, advisory layers, or larger repair
jobs. That choice remains correct.

The first large-task eval root exposed a different problem: several contracts
that are required by `/ultra-plan-run` were thinned too aggressively during the
migration. Modify tasks could be planned as new work, required artifacts from
eval cases were checked only after the run instead of being passed into the
runtime contract, plan lint did not know about existing workspace files, and
invalid generated plans were not always preserved for triage.

These are not legacy mechanisms. They are deterministic contracts and
observability surfaces.

Later eval work exposed another contract gap: a model can emit a parsed tool
call with invalid arguments, such as `Write` without the required `path`
field. That is not a verifier failure or a provider policy decision. It is an
execution-contract violation that can be classified before any workspace
mutation.

## Decision

CommandAgent restores the following contracts:

- common plan intent: `new`, `modify`, `investigate`, `document`, `data`, or
  `unknown`
- common step kind: `inspect`, `create`, `edit`, `setup`, `verify`, `repair`,
  or `report`
- expected result: `pass`, `fail`, or `unavailable`
- required final artifacts, passed from CLI/eval into ultra and step plans
- workspace-aware plan lint limited to shallow path existence checks
- invalid generated plan preservation under `.commandagent/plans`

These fields are used for prompt contracts, linting, reporting, and triage.
They do not introduce a second executor, an unbounded repair loop, or
profile-specific agents.

CommandAgent also admits minimal contract recovery:

- recovery can run only for a classified failure whose violated contract is
  explicit
- recovery must use a narrow action that preserves the original step or phase
  boundary
- recovery must rerun the original guard or verifier instead of replacing the
  check
- recovery must stop boundedly with evidence if the same class repeats

Tool-call schema correction is one eligible contract recovery class. A missing
required tool argument can receive one current-step correction because the
selected tool schema is deterministic and the failed call is rejected before
mutation. The correction must remain provider-independent and must not turn
into dependency setup, profile-specific workflow, or retry-until-success
behavior.

Plan and verifier correction may also carry structured contract evidence when
the rejecting guard already knows the violated contract. The evidence is
limited to local facts such as the failed step id, contract code, target field,
rejected command or path, exact missing literals, required paths, required tool
arguments, and bounded diagnostic text. It is rendered into the existing
bounded correction or repair prompt; it does not create a new recovery loop.

The shared evidence type is only a data boundary. It does not imply shared
automation across plan lint, verification, profile checks, tool protocol, or
dependency setup. Future producers should be added only when a concrete
observed failure needs that evidence, and they must not add retry state, target
authority, semantic confidence, sidecar output, memory references, or provider
policy.

## Non-Decisions

This ADR does not reintroduce:

- the legacy engine
- case memory or anti-pattern corpora
- sidecar semantic summarization
- multi-stage automatic repair
- a generic recovery manager that retries until success
- provider/model-specific prompt branches
- framework-specific hidden rules that could live in the common DSL

## Rationale

The recovery follows the minimal-first principle: remove ambiguity before adding
capability. A typed plan contract keeps setup, verification, reporting, and
artifact expectations explicit. This prevents the model from inferring hidden
state and makes large-run failures easier to classify.

The most important boundary is that `intent` and `kind` are not execution
strategy switches. They are schema fields for the planner, lint, verifier, and
reporter. If a future change uses them to dispatch separate execution engines,
that change requires a separate design decision.

The same boundary applies to recovery. Recovery is a contract-correction
mechanism, not hidden continuation. It can provide deterministic missing
contract evidence, but it cannot rewrite the goal, weaken the verifier, or
silently advance to a new phase after the original contract failed.

Structured evidence is admitted because it reduces ambiguity in an already
bounded correction path. It should be removed or narrowed if it starts carrying
semantic guesses, remembered cases, sidecar advice, provider-specific policy,
or workflow state.

## Consequences

- Older plan files remain readable through defaulted fields and are normalized
  when saved again.
- Eval expected artifacts are now visible to the runtime and saved plan files,
  not only to post-run checks.
- Plan lint can reject alternative paths and globs while accepting existing
  workspace files for verification-only contracts.
- Large eval results should become easier to triage even when success rate does
  not immediately improve.

## Revisit Criteria

Revisit this ADR only if these contracts become control mechanisms rather than
schema/reporting boundaries, or if profile rules begin accumulating
provider/model-specific fixes. In that case, remove or narrow the offending
contract before adding new repair behavior.

Also revisit it if minimal contract recovery starts to accumulate broad
failure-specific prompts, hidden retries, or model/provider-specific behavior.
The corrective action should be narrowed or removed before adding another
recovery layer.
