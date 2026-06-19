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
- Split large work into explicit steps instead of relying on a single long
  conversation.
- Keep planning, execution, verification, and repair as separate contracts.
- Treat recovery as contract correction, not hidden autonomy.
- Treat evaluation scripts and docs as part of the product.

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

## Structured Contract Evidence

Contract correction may use structured evidence when the evidence is produced
by the guard that rejected the plan, tool call, verifier, or profile contract.
This is a way to make known facts more explicit. It is not permission to add a
new controller.

An admissible evidence packet is small, deterministic, and local to the failed
contract. It may include fields such as:

- guard or verifier name
- failed step or phase id
- violated contract code
- target field, path, command, or tool
- exact missing literals, required paths, or required tool arguments
- bounded diagnostic text from the rejecting guard

The correction prompt may render this evidence to remove ambiguity, for example
by telling the planner that a `package.json` step must literally mention
`next`, `react`, and `react-dom`. The evidence must come from existing
contracts such as plan lint, profile obligations, tool schemas, dependency
setup policy, or verifier output.

Structured evidence must not include semantic guesses, memory retrieval,
sidecar judgments, hidden task state, or provider/model-specific policy. It
must not select a new workflow, add retries, weaken the original guard, or
continue after the bounded correction has failed.

The common evidence shape is a boundary, not a common recovery engine.
Producers detect deterministic failures, evidence carries exact facts,
consumers render those facts into existing bounded prompts or packets, and
orchestration keeps the original retry and stop rules. Evidence must not carry
target authority, retry state, semantic confidence, sidecar or memory
references, or any instruction to continue automatically.

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
- Profile: small domain contract. It can name verifier commands, protected
  paths, and facts the model should know. It must not become a workflow engine.
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
