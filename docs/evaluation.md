# Evaluation

CommandAgent evaluation treats verifier output as first-class triage data.
Failures should show what command ran, why it failed, and the smallest useful
evidence packet for repair.

Eval case YAML lives under `eval/cases`. Case sets are split into `smoke`,
`small`, and `large`. Large cases should use semantic checks based on required
artifacts, verifier commands, and content signals rather than line count alone.

`scripts/eval_agent_slice.sh` runs a case directory with the release binary and
writes a timestamped root containing per-run `meta.json`, stdout/stderr, a
workspace directory, and `summary.tsv`. Use `--dry-run` for offline wiring
checks. The runner records `success_check` in `meta.json` and applies semantic
checks for required paths and required file content signals in addition to the
process return code and expected artifacts. For `success_check.type:
semantic`, `must_include` content signals are case-insensitive; use a future
literal check type when exact casing is the contract being evaluated.

`scripts/eval_report.py <root>` summarizes `summary.tsv` by headline success,
failure category, and case. Report categories are layer-oriented:
`planning`, `provider_transport`, `tool_protocol`, `step_policy`, `profile`,
`verifier`, `setup`, `quality`, `unknown`, and `ok`. The eval slice runner
classifies explicit profile contract stops as `profile_verification:<code>`
when the runtime reports a profile verification failure such as
`nextjs_dev_port_drift`. This keeps profile-contract drift separate from
generic `rc:<status>` failures and from dependency setup boundaries. The runner
also classifies structured tool-call schema failures as
`tool_args_missing_required_field:<field>` or `tool_args_invalid_json` when
stderr or repair packets show that the model emitted invalid tool arguments.
Provider response parser failures such as malformed XML fallback are reported
under `provider_transport`, not under profile or verifier failure. Malformed
native function-call shape is different from an HTTP or response-body failure:
providers may surface it as bounded tool-call parse evidence so the minimal
loop can emit parser feedback and downgrade native tool mode to XML fallback.
Eval reports should distinguish that class from true transport/HTTP/body parse
failures, even when both originate in the provider layer.
Plan-lint step ownership failures, such as a `setup` step owning a source or
route artifact, should be reported under `planning` with the violated
contract, rejected path, observed artifact role, and required correction. Treat
these as planning-contract failures even if the execution layer would also have
blocked the same mutation later.

New eval runs also record a terminal observation for each run. The terminal
observation is reporting data, not a recovery decision. It answers where the
run stopped and which deterministic contract evidence supports that stop. The
primary field is `terminal_state`; the compatible broad fields remain
`failure_category` and `contract_layer`.

Current terminal states are:

- `ok`
- `plan_parse_failed`
- `plan_schema_failed`
- `plan_lint_failed`
- `provider_transport_failed`
- `provider_parse_failed`
- `tool_protocol_failed`
- `step_policy_failed`
- `profile_contract_failed`
- `verifier_command_failed`
- `dependency_missing`
- `setup_failed`
- `port_in_use`
- `missing_deliverable`
- `missing_evidence`
- `evidence_binding_failed`
- `completion_evidence_failed`
- `eval_assertion_failed`
- `repair_exhausted`
- `explicit_stop`
- `unknown`

The observation fields are `terminal_state`, `failure_class`,
`violated_contract`, `source`, `source_of_truth`, `diagnostic_code`, `command`,
`evidence_runner_status`, `artifact_ledger_status`, `setup_state`, and `port`,
alongside existing recovery fields. `evidence_runner_status` records whether
the completion evidence path was present, missing, executed, or not required.
`artifact_ledger_status` records whether the required artifact ledger was
complete or missing a required deliverable. For example, `EADDRINUSE` or
`address already in use` on a requested dev-server port should be reported as
`terminal_state=port_in_use` and
`contract_layer=setup_bootstrap_contract`, not as source implementation failure.
Old eval roots that do not have terminal observation fields remain readable;
the report backfills conservative values from `reason`.
Plan-file failures should distinguish parse, schema, and lint boundaries.
Unsupported or malformed plan-file syntax is a planning parse failure. Missing
or wrongly typed required fields are planning schema failures. Readable plans
that violate step ownership, verifier policy, profile obligations, or workspace
scope are planning lint failures. Ordinary block scalar strings in known long
text fields should not be treated as failures after the plan-file public
contract update.
`scripts/eval_report.py <root> --recheck` rechecks existing workspaces against
current case
`success_check.required_paths` and `success_check.must_include`, then writes
`recheck_summary.tsv` without overwriting the original summary.

The report also includes a `Contract Layers` section derived from the failure
reason. This is a coarse layer map for triage, not a new success criterion. It
helps distinguish failures in planning, execution/tool protocol, profile,
setup bootstrap, verification, eval success checks, and unknown boundaries.
New eval runs also write `failure_category`, `contract_layer`, and the recovery
report fields into `summary.tsv` and each run's `meta.json`. The recovery
fields are `active_job`, `recovery_owner`, `loop_control_action`,
`dispatch_status`, `dispatch_reason`, `candidate_jobs`, `tie_break_reason`,
`target_path`, `target_role`, `repair_action`, `tool_policy`,
`attempt_outcome`, `evidence_binding_status`, `completion_evidence_status`,
and `explicit_stop_reason`. When a runtime repair packet contains richer
contract evidence, the eval runner extracts those fields. When a failure is
detected only by the eval success contract, the runner derives a conservative
recovery classification from the deterministic reason and target path.

Artifact completion reports must keep four states distinct:

- `missing_deliverable`: a required artifact is absent from the artifact
  ledger.
- `missing_evidence`: the artifact exists, but no completion evidence path was
  observed.
- `completion_evidence_failed`: the evidence runner or verifier executed and
  failed.
- `evidence_binding_failed`: evidence exists, but is not bound to the required
  proof path.

These states are observations. They do not grant retry authority and must not
be used to hide continuation after a failed phase.

The eval runner executes cases through the mode declared in each case. Omitted
mode defaults to `/plan-run`; large cases should normally use `/ultra-plan-run`.
Modification cases can declare a fixture directory, which is copied into each
run workspace before execution.

Case `intent` is passed to the slash command as `--intent`. Case
`expected_artifacts` are passed as repeated `--artifact` flags and are also
checked after the run. This keeps the runtime task contract and the success
check contract aligned; expected artifacts are not only post-hoc eval checks.

Large task eval uses:

```bash
scripts/eval_large_tasks.sh
```

The default is `runs=1` for MVP sign-off because each large case can be slow.
Use release-quality mode when comparing stability:

```bash
scripts/eval_large_tasks.sh --release-quality
```

This runs each large case 3 times.

## Verifier Failure Shape

Each verifier failure records:

- `command`: the local command that was attempted
- `reason`: `command_failed:<status>`, `dependency_missing`, or `blocked:<class>`
- `stdout_excerpt` / `stderr_excerpt`: bounded raw output
- `diagnostic_excerpt`: lines likely to matter for repair, such as type errors
  or failed compile messages
- `source_excerpt`: when output references a source location, nearby source
  lines are included with the failing line marked

`dependency_missing` means the verifier could not run honestly because required
local dependencies are absent. For example, `npm run build` with a Next.js build
script requires `node_modules/.bin/next`, and a build diagnostic such as
`Cannot find module '@tailwindcss/postcss'` can also be dependency setup
evidence when that package is declared in `package.json` but absent from
`node_modules`. CommandAgent must not rewrite build scripts to fake success.
In approved online runs, runtime dependency recovery may run one deterministic
setup command and rerun the original verifier once. Otherwise it should stop
with the explicit dependency-missing reason.

Treat `dependency_missing` as a cross-profile environment/setup boundary, not as
a generic implementation failure. Next.js may report missing `node_modules`,
Python/FastAPI may report missing virtualenv packages, and data tasks may report
missing local tooling. Eval reports should keep this category separate so a run
does not look like a code-quality failure when the verifier was unavailable.

When `dependency_missing` is the only verifier failure and expected paths are
present, runtime repair should first consult setup policy. `--yes` approves one
bounded setup attempt when offline mode is false. No `--yes`, `--offline`,
unsupported package manager evidence, ambiguous lockfiles, setup failure,
setup timeout, or repeated `dependency_missing` should stop with a setup
blocker. The runtime should not suggest a repair replan, try alternate local
compilers through `npx`, or continue phases after unrecovered setup failure.
For npm-based Next.js setup recovery, record the exact setup command because
the runtime includes dev dependencies for build tooling such as TypeScript,
Tailwind, PostCSS, and type packages.

Eval reports for dependency-sensitive cases should record whether setup was
allowed, whether `.commandagent/setup/` logs were produced, and whether the
final failure changed from `dependency_missing` to a real build/source error
after setup. This keeps dependency setup separate from implementation quality.

If approved setup runs and fails with a deterministic package-manager resolver
diagnostic, record that as setup evidence rather than collapsing it into source
build failure. For npm peer dependency conflicts, use a class such as
`dependency_setup_failed:npm_eresolve_peer_dependency` and record the setup
command, manifest target, dependent package, required peer range, and observed
package when available. This is a manifest compatibility boundary; it does not
increase setup retry count or imply hidden continuation.

Verifier evidence is deterministic context for the next repair or replanning
step. Repair prompts may also include active profile contract facts collected
from the current workspace, such as a selected Next.js app root or requested
dev port. These facts are evidence to preserve during bounded repair. They are
not a semantic sidecar summary, automatic repair loop, or profile-specific
workflow. When a verifier still fails after one approved dependency setup
attempt, eval reports should treat the setup result as diagnostic context on
the verifier failure, not as a separate recovery loop.

Tool-call schema failures are separate from verifier evidence. If a parsed
tool call is missing a required field such as `Write.path`, CommandAgent
classifies the failure before verifier/profile interpretation. For eligible
file-changing initial steps, the runtime may issue one strict tool protocol
correction using deterministic schema evidence and a target path from missing
expected paths or a single current-step `expected_paths` entry when available.
Repair turns may also correct malformed `Write` or `Edit` calls while fixing a
failed verifier, because the repair turn is an explicit mutation-allowed
session. Eval reports should keep `tool_args_*` separate from
`dependency_missing`, `profile_verification:*`, semantic checks, and app-quality
failures, including cases where protocol correction succeeds and a later
verifier or app-quality failure remains.

Read-only step-policy failures are also separate from verifier evidence. If an
`inspect`, `verify`, or `report` step attempts `Write`, `Edit`, or mutating
`Bash`, the actionable class is `step_policy:read_only_step_mutation`. Eval
reports should record the failed step kind, failed tool, and whether the saved
repair packet carried that structured contract evidence.

For `/ultra-plan-run` cases, eval reports should also distinguish original
ultra-plan completion from standalone repair-plan completion. A suggested
repair command starts a separate explicit repair plan; it does not mean the
original phase list finished. When profile verification is active, reports
should record the selected profile, profile verification result, selected app
root when applicable, and whether failures are contract violations such as
`profile_verification:nextjs_dev_port_drift` rather than build or dependency
failures. For Next.js route-integration failures, reports should record the
selected route and explicit artifact when the repair packet provides them, for
example `app/page.tsx` and `app/hooks/useGame.ts`. When investigating profile
drift, reports should also note whether the step and repair prompts carried
active profile contract facts before the drift occurred.

Profile-classification false positives are a separate profile-contract
diagnostic. For example, a generated framework declaration such as
`next-env.d.ts` may be observed in the workspace, but it should not be reported
as a route-integration artifact. Eval reports should record the artifact kind,
provenance, selected route, and whether the failure came from an explicit
required/expected artifact or only from workspace observation.

Next.js route integration reports should distinguish missing artifacts from
integration drift. `profile_verification:nextjs_integration_artifact_missing`
means the explicit artifact did not exist and the repair target should be that
artifact path. `profile_verification:nextjs_route_not_integrated` means the
artifact exists but is not imported or referenced by the selected route tree.
Reports should record the selected route, the disconnected artifact, and any
bounded route-tree repair target provided by the profile. Do not collapse these
into one route integration category.

Contract Boundary Propagation fields should be recorded when present:

- `active_job`
- `artifact_role`
- `repair_kind`
- `repair_action`
- `tool_policy_projection`
- `target_admission`
- `target_priority`
- `explicit_stop_reason`
- `artifact_graph_summary`
- `setup_implication`
- `rerun_authority`
- `required_action`
- `disallowed_actions`
- `repair_attempt_ledger`
- `attempt_outcomes`
- `patch_validation`
- `eval_report_fields`

These fields explain why a repair packet or setup recovery path was selected.
They are not proof that the original ultra plan completed.

Patch validation failures are integrity failures, not implementation failures.
If a repair attempt weakens or skips a test, the repair loop should record
`patch_validation`, classify the active job as `explicit_stop`, preserve the
specific stop reason, and stop boundedly. Eval reports should surface the
`patch_validation` and `explicit_stop_reason` fields instead of collapsing the
result into a generic verifier `rc:1`.

Plan-correction no-progress should be reported as a planning failure with the
same active job and missing set that repeated. For example, a Next.js Tailwind
plan that keeps saying `Tailwind CSS` while omitting exact literals
`tailwindcss`, `postcss`, and `autoprefixer` should report
`active_job=manifest_repair`, the package step or `package.json` target, the
required exact literals, and the bounded correction attempts. Do not count this
as setup, source implementation, or provider transport failure unless the
runtime evidence says so.

## Recent Recovery Check

The R5/R6 guard subset at
`eval/runs/r5-r6-guard-subset/20260617T213505` was run from clean commit
`8eff913`.

Result:

```text
large-nextjs-app-modify  false  dependency_missing
large-rust-app-modify    false  rc:1
large-rust-app-new       true   ok
```

The key interpretation is that Next.js remains an environment/setup boundary,
Rust modify moved past the prior missing-artifact/no-tool class and now fails on
compile/edit-repair quality, and Rust new passed the current artifact/process
contract. Details are in
`docs/eval/triage/post-8eff913-r5-r6-guard-subset-20260617T213505.md`.

The stale Edit-target evidence check at
`eval/runs/stale-edit-target-rust-modify/20260617T230748` was run from clean
commit `b68b9ed`. The code now classifies stale Edit failures as
`edit_target_not_found`, but that run did not reproduce the stale Edit class.
It failed earlier on repeated no-tool responses while `src/commands.rs` was
still missing. Details are in
`docs/eval/triage/post-b68b9ed-stale-edit-rust-modify-20260617T230748.md`.

The missing expected path step-contract check at
`eval/runs/rust-modify-missing-path-contract/20260617T235542` and
`eval/runs/rust-new-missing-path-contract/20260618T000711` was run from clean
commit `ac4e833`. Rust modify moved past the targeted missing-artifact/no-tool
class and reached a later Rust module compile error. Rust new failed in a
different class: compile error plus stale Edit repair. Details are in
`docs/eval/triage/post-ac4e833-rust-missing-path-contract-20260617T235542.md`.

The R6 repair focus check at
`eval/runs/r6-repair-file-fix-contract-rust-subset/20260618T004917` was run
from clean commit `6f2df38`. Rust new passed the focused smoke. Rust modify
still failed, but moved beyond the original missing-artifact/no-tool class and
now looks like implementation-quality / phase-decomposition residue. Details
are in
`docs/eval/triage/post-6f2df38-r6-repair-focus-rust-subset-20260618T004917.md`.

## Repair Exhaustion

Bounded repair should stop after the configured file-changing attempt budget.
The exhaustion report records missing expected paths, repeated changed files,
and verifier evidence. For explicit replanning, CommandAgent saves a short
repair packet under `.commandagent/repairs` and suggests:

```text
/ultra-plan-run --profile <profile> "$(cat .commandagent/repairs/<file>.md)"
```

The saved packet is intentionally bounded so it can be fed back through the
slash command parser without turning the whole failed session into a new goal.

## Structured Contract Evidence

Some failures are rejected by deterministic guards that already know exact
correction facts. Current producers are plan lint/profile obligations,
provider transport parser checks, tool protocol schema checks, read-only
step-policy checks, verifier failures, and profile verification failures.
Eval reports should distinguish the guard failure from secondary post-run
summaries. For example, `missing:package.json,app/page.tsx` can be a secondary
artifact check while the actionable runtime cause is a
`nextjs_dependencies_required` plan-lint failure; similarly, a blank page after
repair can be secondary if the saved repair packet first shows
`tool_protocol`, `step_policy`, `verifier`, or profile verification evidence
that was not resolved.

Bounded correction and repair may render structured contract evidence into
prompts or repair packets. The evidence should be evaluated as input clarity,
not as a new recovery loop. Record whether the run moved past the exact
violated contract, whether the original guard still failed after bounded
correction, or whether a later independent failure class appeared.

When evaluating evidence changes, distinguish the layer under test:

- producer: the deterministic guard that emitted evidence;
- payload: the exact fields carried, such as missing literals, paths, failure
  signature, failure kind, repair target, candidate artifacts, related source
  excerpt, or prior attempt ledger;
- consumer: the recovery task, prompt, repair packet, or eval report that
  rendered it;
- orchestration: the active job, admitted target, prioritized target, allowed
  repair action, tool-policy projection, and unchanged bounded retry and stop
  behavior.

Do not report a run as fixed merely because evidence became clearer. Report
whether the targeted failure class moved, whether a new independent class
appeared, and whether the post-run artifact summary differs from the actionable
runtime cause.

## Recovery Task Contract Reporting

When a repair prompt or saved repair packet contains a `Recovery task` section,
eval reports should record it separately from raw contract evidence. The
section is the clarified repair task passed to the minimal loop; it is not
another engine and it does not imply the run should continue automatically.

Record these fields when present:

- source, such as `verifier`, `profile_verification`, `provider_transport`,
  `tool_protocol`, or `step_policy`;
- failed step and contract code;
- blocker and required action;
- repair action, when a Recovery Policy Contract selected one;
- recovery owner and active-job priority;
- loop control action, dispatch status, dispatch reason, candidate jobs, and
  tie-break reason when Recovery Orchestration selected or rejected a path;
- repair target or bounded candidate artifacts;
- completion evidence and evidence binding status;
- deliverable obligations and freshness expectations;
- repair action plan, including allowed change kind and expected evidence
  delta;
- semantic failure report, including observed/expected pairs and admitted
  target;
- repair job state, attempt outcomes, and exhausted target/role facts;
- patch validation outcome when a repair attempt is rejected;
- execution envelope;
- tool policy used for the next repair turn;
- evidence requirement, such as file change or repository read evidence;
- evidence-producing tool, if a repair turn satisfied the requirement;
- disallowed actions;
- success check, such as the original verifier command or profile check;
- evidence signature.

Interpretation rules:

- If the same signature repeats after the bounded repair task, report
  non-convergence under the same failure class.
- If the signature changes, report the new independent failure class.
- If the recovery task is absent, report whether the evidence was too broad to
  form a deterministic task.
- If a read-only recovery task is present, report whether the repair turn used
  the read-only envelope and whether prose-only output was rejected for missing
  repository read evidence.
- Do not treat recovery-task clarity as app-quality success. For example, a
  Next.js game can still be visually poor after the repair packet correctly
  identified a build or route-integration task.
- For dependency-sensitive verifier failures, record whether a repair changed
  package-manager manifests and therefore produced a new setup fingerprint. If
  runtime-owned setup ran again, report it as one setup attempt for the changed
  manifest fingerprint, not as model-issued dependency installation.
- If `evidence_binding_repair` appears, report it separately from missing
  artifact creation. The artifact may exist; the failure is that the declared
  evidence path is missing, failed, or unbound.
- If `repair_action_plan` is rejected or explicit-stop, report the rejection
  reason instead of treating it as a model-quality failure.

## Versioned Event And Budget Reporting

For changes that affect runtime events, evidence envelopes, usage records, or
budget enforcement, eval should capture the versioned event stream when
practical:

```bash
COMMANDAGENT_EVENT_JSONL=<run-root>/events.jsonl <command>
```

Reports should record:

- `run_id`, `job_id`, and final projected job status;
- whether event sequence numbers are monotonic;
- whether unknown event or payload variants remain replay-safe;
- evidence payload variant for the actionable failure;
- usage availability, including provider/model and unavailable reason when
  token metadata is absent;
- budget-exceeded event or truncation event when output/context was bounded;
- whether the runtime stopped, compacted, shrank tool output, requested replan,
  or requested approval.

Do not count clearer telemetry as task success by itself. The report should
separate observability improvements from actual movement past the targeted
failure class.
