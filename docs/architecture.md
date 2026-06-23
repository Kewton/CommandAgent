# Architecture

CommandAgent is built around a small set of modules with explicit boundaries.
The boundary is more important than the module name: when a feature crosses a
boundary, it should be split before more behavior is added.

## Contract Architecture

CommandAgent has one execution engine and several first-class contract
surfaces around it:

- Task Contract: records the user goal, task kind, admission status,
  lifecycle state, request signals, required artifacts, behavior obligations,
  artifact role projections, deterministic constraints, completion evidence
  expectations, and task scope that later contracts must preserve.
- Planning Contract: turns a user goal into explicit step or phase contracts,
  expected artifacts, step ownership, and lintable success conditions.
- Profile Contract: provides deterministic domain facts, artifact
  classification, obligations, verifier hints, protected paths, and
  profile-verification evidence without owning workflow control.
- ArtifactGraph and Workspace Scope Contract: classifies artifact lifecycle,
  roles, relationships, and in-scope paths before they are used for plan lint,
  setup bootstrap, verification, route integration, or repair targeting.
- Recovery Orchestration Contract: consumes deterministic failure evidence and
  artifact graph facts, selects the active recovery job for the current
  blocker, selects or rejects the repair action, projects the tool policy, and
  hands a bounded recovery task or setup action to execution.
- Failure Observation Boundary: projects deterministic failure evidence into
  one normalized terminal-state identity record before recovery or eval
  reporting consumes it.
- Recovery Policy Contract: the policy-decision part of recovery orchestration.
  It admits and prioritizes repair targets and selects one allowed repair
  action from deterministic evidence.
- Setup Bootstrap Contract: owns bounded dependency setup, setup/config
  artifact preparation, and deterministic manifest/scaffold materialization
  when a profile can name the required setup artifacts.
- Setup Job Lifecycle Boundary: records setup/profile/verifier setup state as
  typed evidence such as manifest kind, manifest path, validation status,
  readiness, command authority, setup result, verifier rerun result, and stale
  reason. It is a record/rendering boundary and never executes commands.
- Runtime Job Report Boundary: projects already observed runtime, setup,
  verification, repair, evidence, and explicit-stop facts into one lifecycle
  report record for eval artifacts and human-readable reports. It records the
  lifecycle stage, active owner, selected action, target admission status,
  repair action plan status, attempt outcome, evidence runner status, verifier
  rerun result, explicit stop reason, and completion source. It does not select
  jobs, run verifiers, repair files, or change success criteria.
- Execution Contract: gives the minimal loop one clear executable task, a tool
  policy, path safety, observations, and bounded completion guards.
- Recovery Task Contract: turns a classified deterministic failure into a clear
  repair instruction for the minimal loop.
- Attempt Ledger Contract: records bounded repair attempts, changed files,
  verifier/profile results, before/after signatures, target/role/cluster
  exhaustion, and repeated failure classes so no-progress stops are explicit.
- Patch Validation Contract: validates model edits, mechanical proposals, and
  rollback candidates before progress is attributed. It owns deterministic
  unsafe/noop/duplicate/worsened classifications and reports them through the
  attempt ledger and eval fields.
- Mechanical Repair Adapter Boundary: maps deterministic verifier diagnostics
  to bounded hints or patch proposals after recovery owner, target, action,
  source of truth, and rerun authority are already admitted. It does not
  execute tools or choose targets.

Contract Boundary Propagation is the handoff rule between these surfaces, not a
second execution engine. When a deterministic guard rejects work, it may pass
only the facts it owns to the next layer: violated contract, artifact graph
facts, active job, repair kind, admitted target, repair action when
deterministic, semantic failure kind, source of truth, allowed change kind,
expected evidence delta, workspace scope, artifact ownership, setup
implication, recovery owner, loop control action, dispatch status, dispatch
reason, candidate jobs, tie-break reason, completion evidence, evidence
binding, deliverable obligations, repair action plan, semantic failure report,
repair job state, verifier diagnostic payload, diagnostic code,
observed/expected pairs, affected cases, preferred repair role, weak verifier
reason, admitted cluster targets, patch validation, eval report fields, rerun
authority, and attempt-ledger context.
This lets Recovery Orchestration Contract choose the correct bounded path
before Recovery Task Contract, Setup Bootstrap, or verifier-owned setup
recovery delegates execution to the minimal loop.

These fields are rendered through existing contract evidence and recovery task
payloads. For example, `repair_kind=manifest_dependency_repair` can target
`package.json`, `setup_implication=setup_after_manifest_repair_required` can
state that dependency setup may be stale after manifest repair, and
`rerun_authority=profile_verification,npm run build` can name the checks that
still judge success. The fields do not create retry authority or an unbounded
controller.

Patch validation is the admission boundary after a repair attempt proposes a
workspace change. It consumes the selected owner/action/target facts, touched
paths, source of truth, before/after signatures, and profile artifact
classification. It rejects unsafe patch classes before verifier rerun when the
violation is deterministic, and it records post-rerun outcomes such as noop,
duplicate, no progress, worsened, or passed through the attempt ledger. A
worsened attempt may enter rollback admission only when the original authority
proved the regression and safe rollback data exists; otherwise the runtime
reports a rejected rollback gate and stops or selects another bounded strategy.

Mechanical repair adapters sit beside, not inside, the minimal loop. They
convert diagnostic payloads such as Rust compile errors, Python import errors,
TypeScript/Next.js type failures, route integration failures, or dependency
missing into bounded hints for the Recovery Task Contract. The adapter output
is still validated as a patch proposal and judged by the original verifier.

The Planning Contract must validate more than schema shape. It owns step
decomposition checks such as whether a `setup` step is trying to create a
source artifact, whether a `verify` step is mixed with mutation, and whether an
expected path belongs to the step that names it. Profile artifact
classification supplies typed path facts for these checks, but the profile does
not become a planner.

Task Contract projection is the shared authority that turns explicit intent,
goal keywords, required artifacts, profile obligations, and deliverable roles
into bounded facts such as `task.contract.kind`,
`task.contract.lifecycle`, `task.contract.request_signals`,
`task.contract.constraints`,
`task.contract.expected_completion_evidence`,
`task.contract.behavior_obligation.<code>`, and
`task.contract.artifact_roles`. The step runner may include those facts in the
planning prompt, active step contract, plan-lint evidence, and eval reports.
It may reject a plan that drops a required task artifact, proceeds with a
partial/conflicting task admission where artifact ownership matters, or omits
a deterministic behavior-obligation owner for setup, manifest, route, docs,
data, test, or source obligations. It must not use Task Contract projection to
run tools, add hidden retry authority, perform semantic confirmation, or force
every ultra phase to own every final artifact.

Task Contract persistence is bounded. The contract is rendered into generated
plan prompts, active step facts, plan-lint evidence, saved run artifacts,
session-visible output, and eval reports. CommandAgent does not maintain a
separate cross-command task-contract memory; later commands reconstruct the
contract from public inputs and workspace facts instead of relying on hidden
state.

Profile-specific planning guidance and profile-specific plan lint are exposed
through the Profile Contract. The step runner may call the shared profile
interface to render guidance, classify artifacts, collect obligations, run
profile-specific plan lint, or verify profile facts. The step runner must not
embed Next.js, Python, Rust, docs, or data rules directly in generic plan-lint
logic; it should consume the profile result as common contract evidence.

Profile Contract may include small dependency compatibility producers when
they operate only on deterministic manifest facts or classified setup failure
evidence. For example, the Next.js profile can reject an observed
Tailwind/PostCSS/autoprefixer peer dependency conflict and target
`package.json` for repair, or reject a generated Next.js 14/React 18
TypeScript app that drifted to TypeScript 6 or `@types/react` 19. Profiles
still must not execute package managers, query registries, choose workflows, or
retry setup.

Profile output is rendered through one common schema across profiles. The
schema includes project root hints, classified artifacts, setup artifacts,
scaffold artifacts, route/integration artifacts, verifier commands, protected
paths, behavior obligations, verification failures, recovery candidate hints,
profile project kind, manifest artifacts, entrypoints, integration artifacts,
completion evidence requirements, failure mappings, adapter families, and a
capability matrix. These are profile facts and hints for the common contracts.
They make cross-profile parity observable without letting a profile select the
final active job, bypass dispatch, execute setup, or materialize scaffold files
by itself.

The Recovery Orchestration Contract is not an execution engine. It does not
retry until success or continue hidden work. It is a deterministic decision
layer that prepares the repair path when a guard, verifier, profile check, or
plan lint failure already knows enough to classify the blocker, map it to the
ArtifactGraph, admit the target, prioritize candidates, select one allowed
repair action, project tool policy, and name disallowed actions and rerun
authority.

Completion evidence and evidence binding are part of this visible contract
surface. Completion evidence records whether a repository edit, verifier exit,
docs section, structured-data check, report completeness check, or command
observation satisfied a deliverable. Evidence binding records whether a
required deliverable is connected to an observable proof path such as a
manifest identity, import symbol, executable handle, test script, required
section, schema column, citation, or file layout. Evidence producers may
derive these records from artifact ledger facts, verifier results, profile
facts, setup validation, or tool protocol failures, but they must not run
tools, repair files, or select recovery jobs. A binding failure can select an
`evidence_binding_repair` job, but it still remains a bounded repair task for
the existing minimal loop.

Runtime Job Report is the rendering boundary for these facts. It lets eval
output answer where a workflow stopped without reading raw logs: planning,
running, setup, verifying, repairing, rechecking, completed, blocked, failed,
explicit stop, or dry-run placeholder. Completion source is separate from
success: runtime success, existing success, dry-run placeholder success,
evidence-only success, recheck success, and recheck failure must remain
distinguishable. These values are report projection only and must not become
hidden orchestration triggers.

Artifact Ledger and Completion Authority are the attribution boundary for
deliverable completion. The ledger records required, observed, changed, and
verifier-mentioned paths with role, lifecycle, ownership, source, and source of
truth. It is fed by bounded workspace snapshots, normalized `Read` / `Write` /
`Edit` tool target paths, verifier-mentioned source paths, and deterministic
scaffold/setup provenance when such provenance is already available. Completion
Authority then classifies already observed facts into
`missing_deliverable`, `missing_evidence`, `completion_evidence_failed`,
`evidence_binding_failed`, or `stale_evidence`. It also records the completion
source of truth and freshness status when current-run evidence is required.
It does not repair, retry, choose a future phase, or replace the verifier. It
only prevents path existence, evidence execution, evidence freshness, and
evidence binding from being collapsed into one generic failure.

Verifier Diagnostic Payload is the attribution boundary for failed verifier
commands. It classifies already-observed command output into bounded,
deterministic fields such as `diagnostic_code`, `failure_signature`,
`diagnostic_failure_kind`, `observed_expected`, `affected_cases`, `candidate_artifacts`,
`source_of_truth`, `preferred_repair_role`, `weak_verifier_reason`, and
`admitted_cluster_targets`. It does not run commands, choose a model strategy,
or decide retries. Recovery Orchestration may consume those fields to avoid
collapsing `command_failed:1` into generic source repair, but the original
verifier remains the success authority.

The Recovery Task Contract renders that policy into the next bounded repair
turn for the minimal loop. It should not decide the repair strategy from broad
prose; it should consume the Recovery Policy decision and make the executable
repair instruction clear.

In short, planning, setup, recovery orchestration, recovery policy, and
recovery tasks clarify what should be done; the minimal loop executes the
already-clarified task. Profiles classify and verify domain facts for those
contracts. If CommandAgent cannot form a deterministic artifact graph mapping,
job classification, policy decision, or recovery task, it should stop with
structured evidence instead of asking the minimal loop to infer the repair
strategy from broad failure prose.

The dispatch gate is the final decision boundary inside Recovery Orchestration
before a Recovery Task Contract is rendered. It receives active-job candidates
from deterministic evidence, selects exactly one owner/action pair, projects a
loop control action such as bounded repair task, verifier-owned setup,
tool-protocol correction, or explicit stop, and records why that decision was
made. If the top candidates are ambiguous, it records `contract_conflict` and
stops instead of choosing a path by heuristic.

Active-job candidates carry their recovery owner, source layer, source of
truth, target hint, artifact role, tool-policy projection, loop-control action,
rerun authority, and deterministic reason. Profile policy may produce these
candidates or evidence hints, but it must not bypass the dispatch gate. The
gate is the single place where competing setup, manifest, route, source, test,
docs, evidence-binding, verifier-contract, and tool-protocol owners are
selected or stopped.

Propagation must stay visible and bounded. It can route a manifest dependency
repair toward `package.json`, mark dependency setup stale after that manifest
changes, route a disconnected artifact toward selected-route integration, or
select setup bootstrap when verifier evidence and policy make that action
deterministic. It can also run a bounded verifier-owned `dev_server_smoke`
job when a profile exposes a requested-port contract and a build verifier has
completed. That job validates the dev script, port preflight, endpoint smoke,
and process cleanup as runtime evidence. It must not choose arbitrary future
phases, silently run setup without a setup contract, increase retry count,
authorize model-issued dependency installs, keep background dev servers
running, or let profiles become hidden workflow engines.

Setup lifecycle records are the common runtime job evidence for setup and
manifest blockers. Manifest validation can classify Node, Cargo, and Python
manifest failures before they collapse into generic source repair. The record
can state `setup_readiness=manifest_missing`, `manifest_invalid`,
`missing_dependency_artifact`, `setup_stale`, or
`setup_attempted_for_fingerprint`, and can state command authority such as
`allowed`, `blocked_invalid_manifest`, `blocked_repeated_attempt`, or
`blocked`. The existing setup runtime remains the only place that may run an
authorized setup command, and the original verifier/profile/dev-server check
remains the success authority after setup.

Next.js route integration is profile evidence, not workflow control. The
profile may build a bounded static graph from the selected route through
relative imports and use it to distinguish transitive route-tree integration
from a disconnected artifact. The graph is limited in depth and file count,
ignores generated/cache/setup artifacts, and does not run Next.js,
TypeScript, or semantic UI checks.

Recovery Task Contract may also carry a small execution envelope. The envelope
is selected from deterministic failure evidence and consumed by the Execution
Contract before the next bounded repair turn. It is intentionally narrow:
`read_only_step_mutation` selects a read-only tool policy plus repository-read
evidence requirement; verifier/profile repair keeps file-mutation repair
semantics; `setup_step_source_mutation` keeps setup/config-only mutation
semantics; tool-protocol correction keeps the current schema-correction path.
The envelope must not add retry authority or provider/model-specific behavior.

## Runtime

- `cli`: parses command-line options and starts one-shot or REPL mode.
- `config`: merges CLI, environment, and `.commandagent/config`.
- `runtime_client`: builds the configured executor and planner provider clients.
- `providers`: hides model transport differences behind a thin chat contract.
- `agent/minimal_loop`: runs one tool-call execution session.
- `agent/repl`: provides interactive use when no prompt is passed.
- `agent/slash_command`: parses interactive planning commands.
- `agent/step_runner`: implements plan and ultra-plan execution.
- `tools`: built-in deterministic capabilities.
- `session`: stores messages, llm-io logs, and resumable state.
- `safety`: path confinement and host validation.
- `util`: shared workspace path and file classification helpers.
- `agent/events`: shared passive runtime events for UI and tests.
- `agent/event_protocol`: versioned external Job/Event envelopes, event JSONL
  observer, command accepted/rejected events, and replayable job state
  projection. It observes runtime events but does not schedule work.
- `agent/budget`: budget contract data, budget decisions, and deterministic
  tool-result truncation helpers.
- `tui`: terminal rendering for interactive progress and final-answer
  formatting.

## Boundary Summary

| Layer | Owns | Must Not Own |
| --- | --- | --- |
| Provider | HTTP/API transport, provider-specific payload shapes | Planning, repair policy, profile behavior |
| Minimal loop | Tool-call execution, observations, bounded completion guards | Multi-step plans, recovery strategy, domain profiles, unbounded retry |
| Profile | Domain facts, artifact classification, verifier hints, protected prefixes, profile evidence, setup artifact templates | Hidden task-specific agents, execution policy, package-registry solving |
| Step runner | Plan schema, step-decomposition lint, ArtifactGraph projection, verifier, recovery orchestration, active job selection, recovery policy, setup bootstrap, dev-server smoke, recovery task contracts, patch validation, mechanical repair hints, rollback admission, repair packet, setup/repair attempt ledger, ultra phase order | Provider transport, low-level tool implementation, unbounded workflow control |
| TUI | TTY-aware rendering of runtime events and final answers | Planning, repair, retry, provider parsing, filesystem policy |
| Tools | Deterministic workspace actions | Task interpretation or planning |
| Eval | Run roots, summaries, recheck, reports | Runtime behavior changes |
| Event protocol | Versioned external events, command response events, replay projection | TUI rendering, queueing, scheduling, approval UI, retry policy |
| Budget | Budget data, bounded tool-result truncation, explicit budget decisions | Provider/model selection, hidden continuation, verifier success policy |

This separation is the main defense against rebuilding hidden legacy behavior
under new names. The orchestration may become richer, but each decision must
remain attributable to a contract surface.

## Versioned Job/Event Boundary

Runtime events remain passive internal observations for TUI and tests.
CommandAgent can also adapt them into a versioned external event envelope:

```text
RuntimeEvent -> VersionedEvent(schema_version, run_id, job_id, sequence, payload)
```

The external protocol is append-only and replay-oriented. A projector can
derive a durable job state such as planning, running, verifying, repairing,
blocked, completed, failed, or cancelled from the event sequence. Unknown
event types or unknown fields must not break replay. TUI continues to consume
internal events and must not parse external JSONL display text.

For one-shot runs, setting `COMMANDAGENT_EVENT_JSONL=/path/to/events.jsonl`
records the versioned event stream while preserving normal stderr TUI output.
This is an observability path for eval and CommandMate integration, not a new
runtime control path.

## Evidence, Usage, And Budget Records

`ContractEvidence` remains readable for compatibility, but new evidence can be
projected through an `EvidenceEnvelope` and typed `EvidencePayload` variants:
planning, provider transport, tool protocol, step policy, verification,
profile, setup, recovery attempt, or unsupported. Evidence describes what
failed. ArtifactGraph describes which artifacts and relationships are involved.
Recovery Orchestration, Recovery Policy, and Recovery Task still own what to do
next.

`FailureObservation` is the shared terminal-state identity projection of
failure evidence. A `ContractEvidence` failure can render a compact observation
line, and an `EvidenceEnvelope` carries the same projection alongside its typed
payload. The observation records fields such as `terminal_state`,
`failure_class`, `contract_layer`, `violated_contract`, `producer`, `guard`,
`diagnostic_code`, `failure_signature`, `source`, `source_of_truth`, and
`actionability`. It is deliberately data-only: it does not admit targets,
select repair actions, rerun commands, or alter retry budgets.

The orchestration section may carry `recovery_owner`, `completion_evidence`,
`loop_control_action`, `dispatch_status`, `dispatch_reason`, `candidate_jobs`,
`tie_break_reason`, `evidence_binding`, `deliverable_obligations`,
`repair_action_plan`, `semantic_failure_report`, `repair_job_state`,
`attempt_outcomes`, `exhausted_targets`, `exhausted_roles`,
`exhausted_clusters`, `no_progress_strategy`, `repair_state_status`,
`safe_stop_payload`, `patch_validation`, and `eval_report_fields`.
`attempt_outcomes` are consumed by target admission through exhausted target,
role, and cluster facts; repeated ineffective repairs therefore narrow or stop
the next bounded repair instead of silently reusing the same target.
Target-admission data may carry
`proposed_targets`, `admitted_targets`, `rejected_targets`, `repair_brief`,
`selected_failure_cluster`, `repair_brief_status`, `action_envelope_status`,
`target_source_of_truth`, `target_ownership_source`, `target_workspace_scope`,
`target_evidence_freshness`, `focused_edit_status`,
`current_excerpt_available`, `target_priority_components`, and
`target_conflict_reason`. Semantic diagnostic fields add
`diagnostic_failure_kind`, `semantic_cluster_source_of_truth`,
`observed_expected`, `affected_cases`, `candidate_artifacts`, and
`unknown_diagnostic_count` so eval can distinguish raw verifier failure,
verifier-contract failure, setup/dev-server failure, and implementation
failure without reparsing logs. Repair-action envelope fields add
`allowed_tool_category`, `repair_root_cause`, `repair_hypothesis`,
`expected_improvement`, `target_confidence`, `must_preserve`, `success_check`,
and `repair_plan_rejection_reason`. A rejected repair action envelope is a
deterministic stop before a repair prompt, not another model turn. These
fields are reporting and repair-contract data. They do not grant retry
authority or create another execution loop.

Target admission is a deterministic gate between active-job dispatch and
repair prompt rendering. It uses ArtifactGraph, ArtifactLedger, workspace
scope, artifact ownership, selected active job, and selected action to admit
or reject target candidates before the model turn. Focused edit signals from
read/edit/write records, verifier mentions, setup/scaffold deltas, completion
evidence, and evidence bindings are candidate sources only. The admission gate
must reject stale targets, missing current excerpts, out-of-scope files,
role-mismatched targets, exhausted targets/roles/clusters, and ambiguous
same-priority candidates before the repair prompt is built. Semantic repair
planning is bounded contract data that selects a failure cluster and repair
hypothesis from existing evidence. The repair brief is the structured source
for the existing Recovery Task Contract; it is not a planner, executor, or
retry mechanism.

Eval failure observations use the same terminal-state taxonomy through the
shared fixture under `scripts/failure_observation_taxonomy.tsv`. The eval
helper can still backfill old run roots, but new runtime evidence should prefer
the normalized observation fields over raw reasons such as `rc:1`. Reports can
therefore flag unknown or raw failures as observation coverage defects without
changing runtime behavior.

Provider usage is normalized into a common `ModelUsage` shape at the provider
parse boundary and carried on `ChatResponse` into runtime events. Missing usage
is recorded as unavailable, not as a runtime failure. Cost records are separate
from usage records and may remain unavailable when pricing is not configured.

Tool results are subject to deterministic output budgeting. If a result is
truncated, CommandAgent emits a truncation event and includes a marker in the
tool observation. This prevents large local outputs from silently overwhelming
model context while keeping the truncation observable.

Successful repository-observation and file-mutation tool records also carry
safe target metadata. `Read`, `Write`, and `Edit` retain the normalized target
path, not raw file contents or full tool arguments. Recovery changed-file
summaries and artifact-ledger summaries use these exact paths so repair policy
can distinguish "this target was read" and "this target was edited" from the
older coarse fact that some tool ran.

## REPL

When no prompt argument is supplied and stdin is a TTY, `cli` starts the minimal
REPL. The REPL owns only line input, `/exit` and `/quit`, empty-line skipping,
and per-turn session saving. Actual work is delegated to a `ReplTurnRunner`;
the production runner calls the minimal loop, while tests use a mock runner.

Slash command parsing is a separate module. It recognizes plan/ultra-plan
commands, `--profile`, `--style`, and bounded `$(cat ...)` repair prompt
references. File references are resolved through path confinement before their
contents are expanded.

## Terminal UI

The TUI is a passive observer. The runtime emits bounded events through
`agent/events`, and `src/tui` renders those events to stderr only when stderr is
a TTY. Non-TTY stdout remains script-friendly and does not receive progress
text.

Terminal progress can show a startup logo, plan generation, saved plan paths,
plan previews, ultra phases, step starts/finishes, tool summaries, verifier
status, dependency setup start/finish, profile verification failures, artifact
status, bounded repair attempts, repair packet paths, and a standalone
suggested next command. These lines are evidence from existing runtime state;
they do not change planning, verification, repair budgets, provider behavior,
or tool policy.

For blocking planner, model, verifier, repair, and tool waits, the TUI can emit
an in-place elapsed spinner until the runtime emits completion, failure, or the
next event. Disabling the spinner affects only the active wait animation, not
the ordinary append-only progress evidence.

Assistant final answers are Markdown-formatted only when stdout is a TTY and
Markdown rendering is enabled. The renderer supports a narrow subset and emits
SGR-only ANSI escapes.

XML fallback parsing remains in provider/minimal-loop code. TUI displays
tool-call mode and parser feedback events but does not parse
`<commandagent_tool_call>` blocks or infer tool behavior from assistant text.

Artifact status uses the runtime's path-confined missing-path helpers. Step
`expected_paths` are step-local gates. Ultra-plan `required_artifacts` are final
user-requested outputs and are reported at the final ultra boundary, not as
phase-local failures.

## Provider Boundary

Provider abstraction is intentionally thin. Providers send chat turns and return
assistant content plus optional native tool calls. Ollama and Gemini may use
native tools. OpenAI uses XML fallback tool calls unless a provider-specific
native tool surface is added deliberately.

Planner and executor can use different providers and models.

Current provider capability contract:

- `ollama`: native tool calls by default
- `gemini`: native function calling by default, with XML fallback retained as a
  compatibility/downgrade path
- `openai`: XML fallback tool calls by default

The provider layer does not own planning, repair, profiles, or evaluation. A
provider-specific bug fix belongs in the provider module; a behavioral policy
belongs in the minimal loop or step runner only if it is provider-independent.

Native tool schemas come from one provider-independent `ToolSpec` argument
schema. Providers serialize that schema into their own native payload shape:
Ollama tool definitions today, Gemini `functionDeclarations`, and any future
OpenAI native surface only after a separate design decision. Native transcript
metadata is also provider-independent: assistant messages may preserve tool
call id/name/args, and tool messages may preserve the matching tool call
id/name. Providers may serialize that metadata, but the minimal loop still owns
tool execution and observation. Provider-specific transport requirements may
extend serialization details inside the provider module, such as Gemini's REST
requirement to return `thoughtSignature` with replayed `functionCall` parts.
Those details must remain transport metadata, not repair or planning policy.

XML fallback is a shared tool-call format, not provider-specific behavior.
Gemini and OpenAI provider modules may parse XML fallback blocks from provider
response text and return them as `ChatResponse.tool_calls`, while also removing
the XML block from assistant content. The minimal loop still keeps XML
extraction as a safety net so the execution contract remains provider
independent. When XML fallback tool calls are parsed into `tool_calls`, the
minimal loop renders those calls back into canonical XML in assistant history so
API providers can see the prior tool call on the next turn. A single XML block
must not result in duplicate tool execution.

Malformed native function-call shape is not a repair policy. A provider may
turn it into bounded tool-call parse evidence so the minimal loop can run the
same parser-feedback and native-to-XML fallback transition it already uses for
malformed fallback syntax. Transport failures, HTTP failures, and unparseable
provider response bodies remain provider/model errors.

## Tool Contract

File creation is done with `Write`; parent directories are created
automatically. `Bash` is for local inspection, tests, and build verification,
not for creating directories before `Write`.

`Bash` must keep offline policy consistent with the prompt: local read-only,
script-run, and build-test commands are allowed when they remain inside the
workspace; dangerous or network actions are blocked.

Dependency setup commands are a narrow exception class, not ordinary Bash
capability. `npm install`, `npm ci`, and `pnpm install` classify as `EnvSetup`.
They remain blocked for normal model-issued `Bash` tool calls, even when
`--yes` is set. The step runner may run one `EnvSetup` command only after a
verifier returns `dependency_missing`, expected source paths are present,
setup is approved, and offline mode is disabled.

If the bounded setup command itself fails with a deterministic package-manager
diagnostic, such as npm `ERESOLVE` peer dependency evidence, the setup layer may
render that diagnostic as manifest compatibility evidence. This evidence can
name the setup command, failure signature, observed and required packages, and
manifest target. It does not authorize another setup attempt or hidden
continuation.

Directory creation through `Bash` is blocked with guidance to use `Write`
instead. The `Write` tool creates parent directories automatically, so `mkdir`
is not part of the normal file creation path.

The only compound command form intentionally recognized by `Bash` policy is:

```text
cd <workspace path> && <local read/script/build command>
```

The tail command is reclassified on its own. Extra chaining or dangerous tails
remain blocked.

All file tools and session writes must go through path confinement. Relative
paths are resolved under the workspace root. Parent traversal and symlink escape
are rejected before a tool reads or writes data.

Step execution also carries a step tool policy from the step runner into the
minimal-loop executor. Inspect and report steps are read-only. Verify steps are
no-mutation checks. Setup steps may change setup/config files such as
`package.json`, `tsconfig.json`, `next.config.*`, `tailwind.config.*`, and
`postcss.config.*`, but source route/component edits belong to create, edit, or
repair steps. Repair turns are explicit bounded repair sessions and may mutate
files within the normal file-tool and path-confinement rules.

Tool-call schema failures are execution-contract failures. After a provider or
XML fallback parser has produced a tool call, the minimal-loop executor rejects
missing required fields and invalid JSON arguments before any tool mutation.
The step runner classifies structured tool argument failures such as
`tool_args_missing_required_field` and may issue one strict current-step tool
protocol correction before normal repair resumes. Initial turns only receive
that correction for steps that can mutate by contract. Repair turns may also
correct malformed `Write` or `Edit` calls while fixing a failed verifier, such
as a `verify` step whose build check failed, because the repair turn itself is
an explicit mutation-allowed session. The correction prompt names the failed
tool, missing or invalid argument, required fields, and a deterministic target
path when the step contract provides one: missing expected paths are preferred,
otherwise a single step `expected_paths` entry can be used as data. Repeated
schema failures stop explicitly and are reported in repair packets and eval
summaries. This is not provider policy, profile verification, dependency setup,
or verifier success.
The step runner first normalizes deterministic protocol failures into a common
payload, then selects one bounded correction action. The action may allow only
the failed tool, require a `Read` before retrying a stale edit, require
repository-evidence tools after prose-only output, or reject unsafe/provider
parse failures with explicit stop. The minimal loop receives this as a narrow
execution envelope and an allowlist of tools; it still does not plan future
recovery work.

Search tools walk the workspace deterministically and skip hidden paths by
default. Search output is bounded so a tool result cannot flood the next model
turn.

## State and Logs

State lives under `.commandagent/`, including plans, repairs, and sessions.
Sessions are stored at `.commandagent/sessions/<id>/session.json`. The MVP
supports save/load and `--resume` plumbing, but does not migrate historical
state directories.

LLM request/response observations are stored as JSON Lines at
`.commandagent/logs/llm-io.jsonl`. The logger records provider, model, planner
metadata, tool-call mode, and payload. Secret-bearing keys such as API keys,
authorization headers, and tokens are redacted before writing.

## Step Runner Boundary

The step runner owns planning, linting, verification, and bounded repair. The
minimal loop owns single-step execution. Profiles add small contracts and
verifiers, not full domain-specific agents.

Step plans use a small CommandAgent-owned YAML schema: goal, profile, style,
intent, required final artifacts, and ordered steps with kind, instruction,
expected result, expected paths, and verifier commands. Plan files are a public
contract boundary for built-in and external planner surfaces. The reader
accepts ordinary scalar forms for known fields, including block scalars for
long goals or instructions, then normalizes into typed plan structs before
schema validation and linting. Known long text fields accept `|`, `|-`, `|+`,
`>`, `>-`, and `>+`; canonical rendering may still use quoted scalar strings.
The writer emits a stable canonical form for saved plans. This keeps planning
bounded without making external planners match one incidental line shape.
Missing fields in older plan files are defaulted on read and normalized on
save.

Plan linting is a separate pass. It rejects obvious schema-contract mistakes:
non-file `expected_paths`, JSON/property selectors, alternative paths, glob
patterns, version strings, path escape, and steps that clearly mix
file-changing setup with final verification. Workspace-aware lint may check
whether named paths already exist, but it is limited to shallow existence checks;
it does not read file contents or force a framework-specific project structure.
Plan lint also owns deterministic step-decomposition checks when artifact roles
are known: setup steps may name setup/config artifacts, create/edit/repair
steps may own source/test/docs artifacts, and verify/report/inspect steps must
not claim mutation-owned artifacts. Execution tool policy repeats these
boundaries as a final guard, but it should not be the first place a bad
decomposition is discovered.

Ultra plans are one level higher: goal, profile, style, intent, required final
artifacts, and ordered phases. Each phase is later turned into a step plan.
Ultra planning does not run tools by itself; it only creates bounded phase
contracts under `.commandagent/plans/ultra-plan-*.yaml`.

Ultra execution is phase-oriented. For each phase, CommandAgent builds a
phase-local step-planning prompt with a freshly collected bounded workspace
snapshot, a data-only phase workspace contract, and the selected profile
contract, then delegates to a step-plan executor. The phase contract contains
generic facts such as visible root entries, lockfiles, package scripts, final
required artifacts, profile-projected summary lines, and profile-projected
obligations. Profile obligations are deterministic fact lines, not executable
workflow. They may be used by step-plan lint to reject a generated phase plan
that edits a contract-bearing file such as `package.json` while omitting a
required profile literal such as the requested Next.js dev port. For Next.js,
the same lint path may reject a generated create/edit/repair source step that
creates an explicit UI/game artifact but does not mention the selected route in
the step instruction or `expected_paths`. This is a narrow profile contract
projection, not a generic artifact graph or framework workflow. The normal
bounded plan-correction path handles rejected plans. The phase contract is also
rendered as an active step contract before each executable step. The runtime
refreshes current profile facts from disk immediately before the step prompt
and before verifier-repair prompts, then asks the model to preserve those facts
while doing only the current step. This is prompt context and repair evidence
only; it does not mutate files, retry secretly, or turn the profile into a
workflow engine. The phase contract does not choose a framework-specific
workflow or mutate files. A phase failure stops the run and returns a readable
phase report instead of continuing with stale context. If the failed phase has
already mutated files, read-only profile verification may still run at that
failed boundary to report any profile drift alongside the step failure.

When plan lint rejects a generated step plan with deterministic contract facts,
the rejection may carry structured contract correction evidence into the
existing bounded plan-correction prompt. For example, a Next.js package step
that omits `react-dom` from a dependency obligation can render the failed step,
violated contract, required literals, and missing literals. This evidence is a
prompt payload only: the original lint guard reruns unchanged, correction
budgets do not increase, and no provider/model-specific branch is introduced.

The evidence pipeline has four responsibilities:

- producers detect deterministic failures;
- the common evidence payload carries exact bounded facts;
- consumers render those facts into existing correction or repair prompts and
  packets;
- orchestration reruns the original guard or verifier under the existing
  bounded rules.

The current common producers are plan-lint/profile-obligation evidence,
provider transport parse failures, tool-protocol schema failures, read-only
step-policy violations, verifier failures, and profile verification failures.
Provider transport evidence is limited to shared response-parser diagnostics
such as malformed XML fallback or JSON tool-call payloads; it must not become
provider/model-specific behavioral policy. Verifier evidence may include a
stable failure signature, failure kind, diagnostic code, affected command,
candidate artifact paths, a single repair target, a related source excerpt,
and a bounded repair-attempt ledger when those facts come from the verifier
result. Profile verification uses the same evidence payload inside its
existing profile repair packet; for Next.js, this currently covers selected
route integration, missing explicit integration artifacts, script/dependency
drift, Tailwind/PostCSS contract drift, TypeScript alias/root drift, and mixed
app-root failures. Dependency setup is not a standalone producer; if one
approved setup attempt runs and the verifier still fails, the setup result is
attached as diagnostic context to the verifier evidence.

Verification is deterministic. It runs only commands accepted by the local Bash
policy, detects dependency-missing cases before fake success is possible, and
compresses failures into bounded diagnostics plus nearby source excerpts when a
file/line reference is present. Next.js source verification should stay on
`npm run build`; `npx` verifier commands are rejected at the plan-contract
boundary because they may perform dependency setup and cannot participate in
verifier-owned setup recovery.

When every verifier failure is `dependency_missing` and expected paths are
present, the step runner checks the setup policy before repair. With `--yes`
and online mode, it selects one deterministic setup command from lockfiles,
runs it once with a bounded timeout, writes setup logs under
`.commandagent/setup/`, reruns the original verifier once, and continues only
if the evidence improves. Without approval, in offline mode, or after exhausted
setup recovery, it stops with a clear blocker instead of creating a repair
packet.

Repair is bounded and evidence-driven. Repair prompts include structured
contract evidence, verifier failures, missing expected paths, changed-file
evidence, and the active profile contract facts collected for the current step
so repairs can preserve contracts such as a selected Next.js app root or dev
port. When deterministic evidence is specific enough, the repair prompt renders
a `Recovery task` section before repair focus and evidence provenance. That
section states what to fix, which paths are in scope, which actions are
disallowed, which execution envelope applies, and which original verifier,
profile check, tool schema, or step policy will judge the result. For read-only
step-policy recovery, the repair turn uses `StepToolPolicy::ReadOnly` and
`RepositoryEvidenceRequired`, so prose-only answers are rejected while actual
read evidence can satisfy the turn. For setup steps that attempted source
mutation, the repair turn uses `StepToolPolicy::SetupMutationOnly`, so the
next turn can only change setup/config paths or report a blocker. A
repair-focus block may still summarize
the current blocker, failure signature, repair target, candidate artifacts, and
required action from the same deterministic evidence. The default budget allows
two file-changing attempts. A structured
tool-argument schema failure can spend one separate current-step
protocol-correction flag before ordinary verifier repair continues; it does
not increase the verifier-repair budget or create a retry-until-success loop.
Read-only step-policy violations, provider transport parse failures, tool
protocol failures, and verifier failures are rendered into the same
repair/replan evidence path so standalone repair has the failed parser/tool,
command, contract, target, source excerpt, and bounded diagnostic instead of
only prose. When repair is exhausted,
CommandAgent writes a short replan packet under `.commandagent/repairs` and
suggests an explicit
`/ultra-plan-run --profile <profile> "$(cat ...)"` command instead of hiding an
unbounded retry loop. That suggested command starts a standalone repair plan;
it is not reported as completion of the original ultra plan unless the original
plan is explicitly resumed or replanned and finishes.

## Profile Boundary

Profiles are intentionally small. They provide profile text, optional verifier
commands, optional protected path prefixes, profile-specific planning
guidance, artifact classification, read-only fact summaries, profile
obligations, profile-specific plan lint, and profile verification. They do not
own workflow selection, edit files, execute package managers, or run
domain-specific agents. Profile obligations and profile lint are projected into
common step-plan lint and active step/repair prompt facts; profile verification
can fail a phase with explicit diagnostics and a bounded standalone repair
packet, but it does not auto-repair or auto-resume the original ultra plan.

The current profile set is MVP-sized: `generic`, `nextjs`, `python`, `rust`,
`investigation`, `docs`, `data-analysis`, and `data-pipeline`. A new profile
must justify why the generic contract plus explicit user instructions are not
enough.

## Minimal Loop

The minimal loop owns one coding-agent session:

- build the system/user/tool context
- call the active chat provider
- execute tool calls
- append tool observations
- finish only when the assistant returns a completed final answer

Provider transport is injected through a small `ChatClient` trait. This keeps
Ollama, Gemini, and OpenAI transport details outside the loop. Native tool calls
and XML fallback are both represented through the shared `ToolCall` type.

The minimal loop should not decide what a failed repair task is. Its input must
already define the task clearly enough to execute. For repair, that means the
step runner, verifier, or profile layer owns the recovery task contract when a
deterministic failure can identify the blocker, target, required action, and
disallowed actions. The minimal loop executes that bounded task and the
original guard/verifier/profile check remains the success authority.

Malformed tool-call parsing in native mode downgrades the session to XML
fallback mode. The parser feedback shows the XML format example only after this
mode transition.

The loop has three narrow completion guards:

- future-action feedback: a no-tool response that says it will create, edit,
  read, run, or verify something is not accepted as a final answer on the first
  occurrence
- completion-without-write feedback: a no-tool completion before any Write/Edit
  receives one neutral reminder that file-changing tasks require tools
- requested-artifact feedback: configured expected paths are checked before
  completion, and missing paths receive one direct reminder

These guards do not inspect task semantics. They only react to observable
session facts and are capped to avoid unbounded repair behavior.

## Removed Legacy Surface

CommandAgent has no legacy engine, sidecar route, case memory, anti-pattern
retrieval, Photon/PAM advisory layer, or old repair job system. If one of those
ideas becomes necessary, it must be reintroduced through the admission rule in
`docs/philosophy.md`, with a narrow trigger and an eval plan.
