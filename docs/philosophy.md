# Philosophy

CommandAgent is a minimal local-first coding agent for local and API-backed
LLMs. The design favors small deterministic control surfaces over large
agent-side orchestration.

## Product Bet

CommandAgent exists to test a narrow hypothesis: a small tool loop plus explicit
planning boundaries can solve useful coding tasks without importing a large
legacy control stack. The agent should make deterministic facts visible, keep
human-visible plans and repair prompts inspectable, and avoid hidden autonomous
machinery.

## Principles

- Keep one execution engine. There is no legacy engine switch.
- Prefer deterministic checks before adding feedback mechanisms.
- Keep runtime guards narrow, bounded, and observable.
- Treat predictable behavior as a product requirement. A fix that raises one
  benchmark but makes runtime behavior harder to anticipate is not admissible.
- Do not turn profiles into hidden applications.
- Treat profiles as structured domain contracts, not prompt-text buckets.
- Keep profile facts, artifact classification, obligations, verification, and
  recovery evidence separate.
- Treat task and step decomposition as contract data, not just planner prose.
- Validate step kind, artifact role, and workspace scope before relying on the
  execution guard.
- Split large work into explicit steps instead of relying on a single long
  conversation.
- Keep planning, execution, verification, and repair as separate contracts.
- Treat recovery as contract correction, not hidden autonomy.
- Treat recovery tasks as first-class tasks: clarify what to fix before asking
  the minimal loop to execute the repair.
- Treat evaluation scripts and docs as part of the product.

CommandAgent's control model is therefore:

```text
Planning Contract -> Profile Contract -> Execution Contract
classified failure -> Recovery Task Contract -> Execution Contract
```

The non-execution contracts are clarity boundaries, not additional engines.
They exist so normal work and repair work can be narrowed before delegation to
the minimal loop instead of asking the loop to infer strategy from broad prose.

## Stability And Predictability

CommandAgent should be boring in the parts that control execution. Local LLM
output will vary, so the surrounding runtime must avoid adding avoidable
nondeterminism.

A runtime guard, repair rule, profile contract, or eval policy is acceptable
only when it preserves these properties:

- deterministic trigger: the condition is based on observable state such as a
  missing expected path, a verifier result, or a specific tool error
- bounded effect: the mechanism has a hard cap and cannot become a hidden retry
  loop
- stable scope: the mechanism applies to a narrow layer, such as repair context,
  profile facts, verifier classification, or eval checks
- observable outcome: logs, repair packets, or eval reports show why the
  mechanism fired
- provider-independent behavior: the policy does not depend on quirks of one
  model or provider unless it is isolated in the provider transport layer

Avoid changes that make behavior unstable:

- retrying until success
- running network or dependency setup implicitly during normal eval
- accepting fake verifier success such as rewriting build scripts
- adding provider-specific prompt branches for behavioral policy
- broad no-tool guards that affect ordinary conversation or report-only tasks
- semantic checks that over-specify implementation structure instead of external
  behavior

This means a slower or lower-success result can still be the correct outcome if
the alternative is opaque, non-reproducible behavior.

## Task And Step Decomposition

Large-task reliability depends on making the work breakdown itself
inspectable. Planning is not only YAML syntax. It is a contract that assigns
responsibility to each step and names the artifacts that make the step done.

The planner may propose a decomposition, but deterministic lint should validate
the decomposition when the relevant facts are known:

- `inspect` and `report` steps are read-only.
- `verify` steps run checks and do not mutate the workspace.
- `setup` steps may prepare dependency or configuration artifacts, but must not
  create or edit application source, route, component, test, or documentation
  artifacts.
- `create`, `edit`, and `repair` steps own source, test, and documentation
  mutation under the current task scope.
- expected paths must be artifacts that the step owns, not arbitrary files that
  happen to be observed or inspected.

This makes step-kind accuracy a Planning Contract responsibility. The
Execution Contract remains the final safety boundary, but a step such as
`setup` attempting to create `app/globals.css` should be rejected by plan lint
before the model reaches file tools. Runtime tool policy is the last defense,
not the primary way to discover a bad decomposition.

The useful lesson from larger historical control stacks is not that
CommandAgent needs a hidden project manager. The lesson is that task contracts,
artifact roles, workspace scope, and recovery targets are legitimate control
data when they stay deterministic and visible:

- A task contract records the goal, required artifacts, constraints, and
  success checks that are already explicit or deterministically inferred.
- An artifact role classifies paths as setup/config, implementation, test,
  documentation, generated output, dependency cache, raw input, or derived
  output before those paths are used for lint, verification, or repair.
- Workspace scope names the paths and artifact classes that are in bounds for
  the current task or step, and requires explicit evidence before expansion.
- Recovery target hints name the file, artifact, or command that the rejecting
  guard already identified as the repair target.

These concepts should be adopted as small contract boundaries. They should not
become an active hidden controller that chooses arbitrary future jobs, retries
until success, or rewrites the user's task.

When choosing between mechanisms, prefer this order:

1. deterministic classifier or lint rule
2. structured evidence and recovery target hints
3. explicit recovery task contract under the original guard
4. setup bootstrap only when verifier evidence proves dependency setup is the
   blocker and the user or policy permits it
5. job arbitration only after a separate design decision proves that narrower
   contracts cannot classify the observed failure

This preserves the practical value of stronger task contracts without
recreating opaque orchestration.

## Recovery As Contract Correction

Recovery is admissible only when it corrects a classified contract violation.
It is not a hidden form of autonomy, a second planner, or a way to keep trying
until the model happens to succeed.

The shape of an acceptable recovery mechanism is:

```text
classified failure -> violated contract -> narrow correction action
  -> rerun the original guard/verifier -> success or explicit bounded stop
```

The recovery action must preserve the original goal, step or phase boundary,
verifier command, profile contract, and tool policy. It may provide missing
contract information that is already deterministically known, such as a missing
tool argument name, a required artifact path, a selected profile fact, or an
approved setup command. It must not invent a new workflow, broaden the task, or
weaken the check that failed.

Dependency setup remains verifier-owned. If a bounded repair changes package
manager manifests, setup state may become stale for that verifier step.
Approved online setup may run once for the new manifest fingerprint, then the
original verifier must rerun. The repair turn itself still must not run
dependency installation directly.

This keeps the runtime layers separate:

- Planning creates explicit contracts; recovery does not silently rewrite a
  failed plan into a new plan.
- Execution runs one tool-call session; recovery may correct a tool protocol
  violation only when the violated tool schema is known before mutation.
- Verification judges the original contract; recovery must rerun the same
  verifier or guard instead of replacing it with an easier check.
- Profiles provide small deterministic facts and obligations; recovery may
  preserve those facts but must not turn the profile into a domain workflow.

Repeated recovery must remain bounded by failure class and step. If the same
classified violation repeats after the allowed correction, CommandAgent should
stop with explicit evidence and a user-visible repair or replan path.

## Profile Contracts

Profiles are structured domain contracts. They may describe domain facts,
classify artifacts, project deterministic obligations, and verify
profile-specific contracts. They must not own planning, execute tools, retry
work, infer hidden workflow state, or become a provider/model-specific policy
layer.

Profile facts are observations. A fact such as a workspace entry, package
script, route path, dependency list, or config file does not become an
obligation by itself. Facts become obligations only after a profile classifier
assigns deterministic meaning to them.

Rendered profile text is for prompts, repair packets, and reports. Runtime
decisions must consume structured facts and classified artifacts directly; they
must not parse rendered profile text back into machine decisions. This keeps
the boundary clear:

```text
structured facts -> classified artifacts -> obligations/verification
structured facts -> rendered text -> prompts/reports only
```

A profile that reasons about paths must use an explicit artifact
classification boundary. The minimum shape is:

- observed path
- provenance, such as user-required artifact, step expected path, workspace
  observation, or profile fact
- artifact kind, such as route entry, route infrastructure, UI/source artifact,
  manifest, config, generated declaration, dependency cache, or raw input
- contract eligibility, such as whether it may be used for route integration,
  verification targeting, protected-path checks, or recovery targeting

Obligation generation and profile verification must consume classified
artifacts instead of broad path strings. For example, a generated framework
declaration file may be observed in the workspace, but it is not a
route-integration artifact unless the profile classifier explicitly marks it
eligible. Workspace observation alone should not create a route-integration or
source-integration obligation.

Profile verification failures may emit structured contract evidence for
repair. The profile identifies the violated contract, deterministic target, and
candidate artifacts. Recovery consumes that evidence and remains bounded under
the shared execution contract; it must not decide profile semantics on behalf
of the profile.

Integration-style profile checks must separate existence from integration. A
missing explicit artifact is a different contract violation from an existing
artifact that is not wired into the selected entry point. For example, the
Next.js profile reports `nextjs_integration_artifact_missing` before route
integration is evaluated, and reports `nextjs_route_not_integrated` only for an
existing explicit artifact that is not referenced by the selected route.

## Recovery Task Contracts

The minimal loop is an execution session, not the owner of recovery planning.
It is useful when the task is already clear, but it should not be asked to infer
the repair strategy from a broad verifier or profile failure. A repair turn
should receive a recovery task contract in the same spirit as a normal step
contract.

When deterministic evidence is specific enough, the step runner, verifier, or
profile layer should translate the failure into explicit repair instructions
before delegating to the minimal loop. A recovery task contract may state:

- the current blocker and violated contract
- what must be fixed
- the repair target or candidate artifact paths
- a small execution envelope derived from the failure class
- allowed tools or paths when the target is known
- disallowed actions, such as dependency setup in an ordinary repair turn
- required action, such as integrating an artifact through a selected route
- the original guard, verifier, or profile check that remains the authority

The execution envelope is a constraint on the next Execution Contract, not a
new executor. For example, a `step_policy:read_only_step_mutation` failure uses
a read-only envelope that requires repository read evidence from `Read`,
`Glob`, `Grep`, or read-only `Bash`; verifier/profile source repair keeps the
file-mutation repair envelope. This prevents a recovery task that says
"read-only" from being run as a mutation-allowed file repair.

This does not turn recovery into a workflow engine. The contract narrows the
next repair turn; it does not choose future phases, add attempts, run hidden
jobs, or replace the verifier. If the runtime cannot form a deterministic
recovery task contract or execution envelope, it should fall back to explicit
bounded failure evidence instead of asking the minimal loop to guess.

In short, CommandAgent's minimalism means minimal hidden authority, not minimal
repair clarity. Recovery authority is admissible only when it is visible,
bounded, and explainable from the failed contract.

## Structured Contract Evidence

Contract correction may use structured evidence when the evidence is produced
by the guard that rejected the plan, provider transport response, tool call,
verifier, or profile contract.
This is a way to make known facts more explicit. It is not permission to add a
new controller.

An admissible evidence packet is small, deterministic, and local to the failed
contract. It may include fields such as:

- guard or verifier name
- failed step or phase id
- violated contract code
- failure signature, failure kind, or diagnostic code
- target field, path, command, or tool
- candidate artifacts, repair target, and related source excerpt when the
  rejecting verifier or profile check already identified them
- bounded prior attempts or repair attempt ledger entries
- exact missing literals, required paths, or required tool arguments
- bounded diagnostic text from the rejecting guard

The correction prompt may render this evidence to remove ambiguity, for example
by telling the planner that a `package.json` step must literally mention
`next`, `react`, and `react-dom`. The evidence must come from existing
contracts such as plan lint, profile obligations, provider transport parsing,
tool schemas, step policy, or verifier output. Dependency setup results may be
attached only as diagnostic context to verifier evidence after one approved
setup attempt; setup is not a separate hidden recovery producer.

Structured evidence must not include semantic guesses, memory retrieval,
sidecar judgments, hidden task state, or provider/model-specific behavioral
policy. Provider transport evidence may state that a shared response parser
rejected malformed XML or JSON, but the repair instruction must stay shared and
must not add a model-specific prompt branch. Evidence must not select a new
workflow, add retries, weaken the original guard, or continue after the
bounded correction has failed.

The common evidence shape is a boundary, not a common recovery engine.
Producers detect deterministic failures, evidence carries exact facts,
consumers render those facts into existing bounded prompts or packets, and
orchestration keeps the original retry and stop rules. Evidence must not carry
retry authority, semantic confidence, sidecar or memory references, or any
instruction to continue automatically. A repair target is admissible only when
it is deterministically selected by the failing verifier/profile contract, such
as a compiler source path or a selected Next.js route.

## Why Legacy Is Removed

The historical source repository's legacy engine accumulated many protective mechanisms:
repair jobs, case memory, advisory layers, anti-pattern corpora, and broader
orchestration. Those mechanisms helped in some situations, but they also made
behavior hard to attribute and hard to evaluate.

CommandAgent intentionally does not copy that stack. A mechanism can be added
only when a current failure analysis shows a concrete gap and the fix can be
kept bounded. This keeps the MVP understandable and prevents a gradual return to
an opaque controller.

## Why Sidecar Is Deferred

Sidecar models are useful for semantic summarization, critique, or secondary
judgment, but they add another source of nondeterminism and make attribution
harder. CommandAgent first relies on deterministic extraction: verifier output,
source excerpts, expected paths, and explicit repair packets.

A sidecar may be introduced later only for a measured task that cannot be solved
by better deterministic evidence. It is not part of the MVP runtime contract.

## Responsibility Boundaries

- Provider: transport only. It converts CommandAgent messages into provider API
  calls and returns assistant text plus optional tool calls.
- Minimal loop: one execution session. It calls the provider, executes tools,
  appends observations, and applies bounded completion guards.
- Profile: structured domain contract. It can collect facts, classify
  artifacts, project obligations, name verifier commands, protect paths, and
  emit profile verification evidence. It must not parse rendered text back into
  decisions or become a workflow engine.
- Step runner: planning, linting, deterministic verification, and bounded
  repair. It orchestrates steps around the minimal loop without changing the
  loop into a planner.
- Eval scripts: evidence collection. They produce comparable run roots and
  reports, but do not tune behavior during a run.

## Non-goals

- Historical compatibility with older agent engines.
- Legacy engine selection or feature parity.
- Sidecar routing in the MVP.
- Memory retrieval, case memory, or anti-pattern corpora.
- Complex autonomous project managers.
- Provider-specific behavior that cannot be expressed through the shared
  provider contract.

## Admission Rule

New mechanisms must start from observed failures. A change is preferred when it
removes ambiguity, makes deterministic facts visible, or narrows an existing
contract. Adding another feedback loop is the last resort.

The minimal loop is intentionally small: provider call, tool execution,
observation append, and final-answer validation. Planning, repair, and profile
contracts live outside the loop.

Minimal-loop feedback is allowed only when it is triggered by deterministic
session facts:

- the assistant described a future tool action without issuing a tool call
- no Write/Edit has happened before a no-tool completion
- explicitly requested artifact paths are still missing

Tool-call schema rejection is also deterministic: a parsed tool call either has
the required JSON fields for the selected tool or it does not. The minimal loop
rejects that call before mutation. The step runner may include that structured
schema failure in one bounded contract correction for the current step, but it
must not become a provider-specific prompt branch, a dependency setup trigger,
or a retry-until-success loop.

Each feedback guard is bounded. It may clarify the current contradiction once,
but it must not become a hidden planner or retry engine.
