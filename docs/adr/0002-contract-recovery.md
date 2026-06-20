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
- recovery should define a clear recovery task before delegating to the minimal
  loop when deterministic evidence identifies the blocker, target, and required
  action
- recovery must rerun the original guard or verifier instead of replacing the
  check
- recovery must stop boundedly with evidence if the same class repeats

Tool-call schema correction is one eligible contract recovery class. A missing
required tool argument can receive one current-step correction because the
selected tool schema is deterministic and the failed call is rejected before
mutation. The correction must remain provider-independent and must not turn
into dependency setup, profile-specific workflow, or retry-until-success
behavior.

Plan lint, tool protocol guards, read-only step-policy guards, and verifier
failures may carry structured contract evidence when the rejecting guard
already knows the violated contract. The evidence is limited to local facts
such as the failed step id, contract code, target field, rejected command or
path, exact missing literals, required paths, required tool arguments, and
bounded diagnostic text. Verifier and profile failures may also carry a stable
failure signature, failure kind or diagnostic code, candidate artifacts, a
single deterministic repair target, related source excerpt, observed/expected
pair, and bounded prior-attempt ledger. It is rendered into the existing
bounded correction or repair prompt; it does not create a new recovery loop.

The shared evidence type is only a data boundary. It does not imply shared
automation across plan lint, verification, profile checks, tool protocol,
step policy, or dependency setup. Dependency setup is not a standalone
evidence producer in the current design; after one approved setup attempt, its
result may be attached only as diagnostic context to a remaining verifier
failure. Future producers should be added only when a concrete observed
failure needs that evidence, and they must not add retry authority, semantic
confidence, sidecar output, memory references, provider policy, or hidden
workflow state. Repair targets are allowed only when the failing deterministic
check selects them, such as a compiler source path or selected Next.js route.

Recovery tasks are first-class contracts. The minimal loop remains an executor;
it should not be responsible for deciding the repair strategy from broad
failure prose. When a guard, verifier, or profile check can deterministically
identify the failure class, target, and required action, the repair layer may
render a recovery task contract that names what to fix, what paths or tools are
allowed, what actions are disallowed, and which original verifier or profile
check will judge the result. This is still bounded repair context. It does not
add a new planner, hidden continuation, retry budget, provider-specific branch,
or profile-owned workflow.

Recovery tasks may also carry a deterministic execution envelope. The envelope
is selected from the same contract evidence and constrains the existing
Execution Contract. A read-only step-policy failure selects read-only tools and
repository-read evidence; verifier/profile repair selects the existing
file-mutation repair behavior; tool-protocol correction keeps schema-correction
semantics. The envelope is not model-selected, does not weaken the original
guard, and does not authorize hidden continuation.

The resulting architecture has peer contract surfaces around one execution
engine: Task Contract, Planning Contract, Profile Contract, Recovery Policy
Contract, Recovery Task Contract, Execution Contract, and supporting artifact
role, workspace scope, setup bootstrap, and attempt-ledger contracts. Planning
Contract clarifies normal work, Recovery Policy Contract selects the bounded
repair policy after a classified failure, Recovery Task Contract renders that
policy into an executable repair instruction, and Execution Contract runs one
clarified task in the minimal loop. Only the Execution Contract delegates to
the execution engine; the other contracts prepare bounded instructions for it.

2026-06-20 amendment: Profile Contract is now documented as a fourth
first-class contract surface. It is not an execution surface and does not
change the recovery decision above. Its role is to provide deterministic domain
facts, artifact classification, obligations, verifier hints, protected paths,
profile-specific planning guidance, profile-specific plan-lint evidence, and
profile-verification evidence to the Planning and Recovery Task contracts.
This amendment also clarifies that Planning Contract owns step-decomposition
lint when artifact roles are known. For example, a `setup` step naming a source
or route artifact should be rejected by planning lint before the Execution
Contract's tool policy has to reject the same mutation.

2026-06-20 amendment: Contract Boundary Propagation is admitted as a design
boundary between the four contract surfaces. A deterministic failure may carry
the violated contract, repair kind, deterministic target, setup implication,
and rerun authority to the next contract layer. This is a handoff contract, not
an orchestration loop. For example, a profile dependency conflict may state
that `package.json` needs manifest dependency repair and that setup freshness
must be reconsidered after the manifest changes; a route integration failure
may state that the selected route or component tree must reference an existing
artifact; a missing module verifier diagnostic may become verifier-owned setup
recovery only when the manifest and setup policy make that action
deterministic. The propagated fields must be consumed by existing bounded
recovery tasks or verifier-owned setup recovery. They must not create hidden
continuation, increase retries, authorize model-issued dependency installs, or
turn profiles into workflow engines.

2026-06-20 amendment: CommandAgent now admits explicit contract orchestration
as a first-class design direction. The single execution engine remains the
minimal loop, but the surrounding runtime may include Task Contract,
ArtifactRole and workspace scope, Setup Bootstrap, deterministic
manifest/scaffold materialization, Active Job Arbitration, Recovery Target
Hints, Semantic Repair Planning, and Attempt Ledger when an observed failure
shows that the responsibility is needed. These mechanisms are acceptable only
as visible contract layers: they classify the current blocker, select one
bounded repair or setup action, rerun the original guard/verifier, and stop
explicitly on no progress. They must not become provider/model-specific
behavior, hidden continuation, arbitrary future-phase selection, unbounded
retry, or a second execution engine.

2026-06-20 amendment: Recovery Policy Contract is now documented as the
contract layer between structured failure evidence and Recovery Task Contract.
It owns active job arbitration, repair target admission and prioritization,
and repair action selection for the current deterministic blocker. For
example, a `nextjs_route_not_integrated` profile failure should become a route
integration repair policy that targets the selected route or nearest
route-graph connection point, selects the action "connect the existing artifact
to the selected route graph", disallows placeholder artifact creation and
unrelated feature work, and preserves profile verification plus `npm run build`
as rerun authority. This policy layer still must not execute tools, increase
retry budgets, select arbitrary future phases, run setup from an ordinary
repair turn, weaken verifiers, or add provider/model-specific behavior.

## Non-Decisions

This ADR does not reintroduce:

- the legacy engine
- case memory or anti-pattern corpora
- sidecar semantic summarization
- hidden multi-stage automatic repair
- a generic recovery manager that retries until success
- provider/model-specific prompt branches
- framework-specific hidden rules that could live in the common DSL

It does allow explicit, bounded counterparts to selected historical
responsibilities when they are represented as contract data and tested from
observed failures.

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

Structured evidence alone is not always sufficient. If the evidence identifies
the repair target and required action, the correct minimal response is to make
the recovery task explicit before calling the minimal loop, not to ask the
minimal loop to infer what should be done. This preserves the execution/planning
boundary while improving repair convergence.

Contract boundary propagation exists for the same reason. It is acceptable to
make the handoff from profile to recovery, recovery to setup, or verifier to
recovery more explicit when the failing deterministic check already owns the
facts. It is also acceptable to classify the active job and select a bounded
repair action before calling the minimal loop. It is not acceptable to use that
handoff as a hidden job manager that chooses arbitrary future phases, silently
runs setup without a setup contract, or keeps repairing until the task
succeeds.

Recovery Policy Contract formalizes that handoff. Structured evidence says
what failed; recovery policy decides, from deterministic facts, what kind of
repair job this is, which target is admitted, which target is preferred, which
single repair action is allowed, and what success check remains authoritative.
Recovery Task Contract then renders the policy into instructions. This keeps
strategy selection out of the minimal loop without reintroducing hidden
autonomy.

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

Also revisit it if contract boundary propagation starts carrying semantic
guesses, package-registry solving, provider-specific behavior, or hidden
workflow state instead of deterministic handoff facts.

Also revisit it if Recovery Policy Contract starts selecting speculative jobs,
mutating state, expanding scope without evidence, or becoming a profile-owned
workflow engine instead of a typed decision table for current deterministic
failures.
