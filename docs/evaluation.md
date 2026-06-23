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
When a child CommandAgent process exceeds `--timeout-secs`, the runner records
the row as `provider_transport:eval_timeout` with `rc=124`, writes bounded
stdout/stderr, and continues the remaining cases. Timeout recording is eval
evidence only; it does not retry the case or change runtime policy.
Use `--no-timeout` only for explicit proof runs that need to show a case was
not stopped by the eval harness. The runner records `timeout_mode=none` and a
blank `effective_timeout_secs` in the run metadata and recheck summary. This is
reporting policy only; it does not change CommandAgent runtime behavior.

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
- `stale_evidence`
- `eval_assertion_failed`
- `repair_exhausted`
- `explicit_stop`
- `unknown`

The observation fields are `terminal_state`, `failure_class`,
`contract_layer`, `violated_contract`, `source`, `source_of_truth`,
`diagnostic_code`, `failure_signature`, `producer`, `guard`,
`actionability`, `explicit_stop_reason`, `command`,
`completion_authority_status`, `completion_source_of_truth`,
`evidence_runner_status`, `evidence_runner_kind`,
`artifact_ledger_status`, `freshness_status`, `missing_evidence`,
`failed_evidence`, `failed_bindings`, `stale_evidence`,
`evidence_binding_kind`, `workspace_scope_kind`,
`workspace_scope_roots`, `artifact_ledger_entries`,
`artifact_ledger_summary`, `artifact_ledger_sources`,
`required_paths`, `artifact_ownership`,
`artifact_ownership_reason`, `artifact_source_of_truth`,
`deliverable_obligation_kind`, `deliverable_obligation_path`,
`deliverable_obligation`,
`rejected_target_reason`, `read_paths`, `changed_paths`, `created_paths`,
`verifier_mentioned_paths`, `scaffold_created_paths`,
`setup_created_paths`, `out_of_scope_paths`, `setup_state`, and `port`,
alongside existing recovery fields. Recovery dispatch fields include
`active_job`, `active_job_lifecycle`, `recovery_owner`,
`loop_control_action`, `dispatch_status`, `dispatch_reason`,
`candidate_jobs`, and `tie_break_reason`. The terminal-state taxonomy is shared with
runtime through `scripts/failure_observation_taxonomy.tsv`; tests should fail
when the Python fallback mapping drifts from the Rust mapping. Runtime job
fields may also include
`runtime_job_kind`, `runtime_job_outcome`, `setup_job_kind`,
`setup_job_state`, `setup_target`, `setup_manifest_kind`,
`setup_manifest_path`, `setup_artifact_validation_status`,
`setup_readiness`, `setup_command_authority`, `setup_attempt_key`,
`setup_manifest_fingerprint`, `setup_stale_reason`, `setup_result`,
`setup_failure_signature`, `setup_command`, `verifier_rerun_result`,
`dev_server_state`, `requested_port`, `port_preflight`, and
`endpoint_smoke`. These setup fields come from setup lifecycle records and are
reporting data; lifecycle rendering does not execute setup. `completion_authority_status` records the authoritative
completion decision. `evidence_runner_status` records whether the completion
evidence path was present, missing, executed, or not required.
Runtime-support parity fields may also include
`phase29_support_rows`, `language_repair_adapter_status`,
`effective_tool_policy`, `effective_tool_policy_status`,
`tool_failure_recovery_status`, `setup_command_classification`,
`command_authority`, `command_classification_reason`,
`workspace_candidate_status`, `workspace_ignored_dir_policy`,
`workspace_candidate_ignored_reasons`, `job_report_status`,
`job_report_owner_action`, `scaffold_contract_status`,
`noncoding_evidence_status`, `answer_work_mode_status`,
`lifecycle_projection_status`, and `provider_boundary_status`. These fields
project already-selected evidence, owner/action state, setup lifecycle, and
provider-boundary facts into reports. They must not execute setup, choose a
new repair owner, retry a tool call, or move behavior policy into providers.
`artifact_ledger_status` records whether the required artifact ledger was
complete or missing a required deliverable. `freshness_status` records whether
completion evidence is fresh, stale, unknown, or not required. The ledger signal fields are
observability fields: they report bounded artifact attribution from workspace
snapshot, tool target records, verifier mentions, and setup/scaffold
provenance. They must not be used by eval to mutate runtime behavior during a
run. For example, `EADDRINUSE` or
`address already in use` on a requested dev-server port should be reported as
`terminal_state=port_in_use` and
`contract_layer=dev_server_port_contract`, not as source implementation
failure.
Old eval roots that do not have terminal observation fields remain readable;
the report backfills conservative values from `reason`.

Contract-conflict reports may include C33 fields when deterministic evidence
shows that implementation, test, docs/API, or verifier contracts disagree:
`contract_conflict_status`, `contract_conflict_sides`,
`contract_conflict_authority`, `contract_conflict_repair_target_side`,
`contract_conflict_selected_action`,
`contract_conflict_safe_stop_reason`,
`contract_conflict_missing_evidence`, and
`contract_conflict_source_of_truth`. These fields explain which side is
authoritative, which side may be repaired, or why the runtime explicitly
stopped. They are report and repair-contract data; eval must not use them to
retry a case or reinterpret an ambiguous stop as success.

Phase 14 eval output also includes a runtime job report projection. The fields
are `lifecycle_stage`, `active_owner`, `selected_action`,
`target_admission_status`, `repair_action_plan_status`, and
`completion_source`, alongside `large_disposition`,
`large_disposition_reason`, `large_disposition_owner_action_status`,
`large_disposition_evidence`, and existing `attempt_outcome`,
`evidence_runner_status`, `verifier_rerun_result`, and
`explicit_stop_reason`. These fields are derived from already observed
runtime, setup, verifier, evidence, and recovery records. They do not run
setup, select a repair owner, retry a verifier, or change success criteria.

Large sign-off also requires failed rows to be attributable. Eval reporting may
project deterministic owner/action/target/evidence fields from already
observed terminal states and diagnostic codes, but only as reporting data. For
example, `provider_transport:eval_timeout` is projected to a provider boundary
blocker with `attempt_outcome=blocked_external`, and
`profile_verification:nextjs_dependency_version_conflict` is projected to a
manifest repair target of `package.json`. This projection must not retry the
case, change runtime repair policy, or turn a failure into success.

Failed large rows must also carry a row disposition. The accepted dispositions
are `closed_owned_failure`, `implementation_blocker`,
`accepted_external_limitation`, and `split_forward`. Broad sign-off accepts
`closed_owned_failure` only when owner/action/target/evidence are internally
consistent. `accepted_external_limitation` is valid only for provider,
network, or environment boundary evidence. `implementation_blocker` and
`split_forward` remain open sign-off findings until the responsible phase
closes them. A large disposition is an attribution decision; it is not a claim
that the user task succeeded.

For failed large rows, blank, `unknown`, and `none` remain missing field
values. `missing`, `failed`, and `blocked_external` are meaningful evidence
states because they preserve the failure. `not_applicable` is field-sensitive:
it is accepted for target/evidence only when the row is a provider/eval
boundary failure with an explicit owner, action, and attempt outcome. It is not
accepted for profile, setup, verifier, route, or source failures that have a
repairable target.

`lifecycle_stage` is the report funnel value. It may be `planning`, `running`,
`setup`, `verifying`, `repairing`, `rechecking`, `completed`, `failed`,
`blocked`, `explicit_stop`, or `dry_run_placeholder`. `completion_source`
keeps success provenance separate from success itself. It may be
`runtime_success`, `existing_success`, `dry_run_placeholder_success`,
`evidence_only_success`, `recheck_success`, `recheck_failure`, `none`, or
`unknown`. Dry-run placeholder success must never be interpreted as runtime
implementation success.
When runtime evidence includes verifier diagnostic fields, eval observation
prefers those fields over a raw process-code reason such as `rc:1`. For
example, a failed `cargo check` can remain
`terminal_state=verifier_command_failed` while reporting
`diagnostic_code=rust_compile_error`,
`source_of_truth=original_verifier_diagnostic`, and the verifier command.
This keeps the verifier failure honest without losing the actionable
diagnostic.
When a report can only identify a raw process-code reason such as `rc:1`
without a diagnostic code or known terminal state, it should list the row under
unknown/raw failure coverage defects. That is an observation gap, not a reason
to weaken the verifier or retry more.
Recheck reporting may also derive a non-raw diagnostic from stable run
evidence already present in stdout, stderr, or repair packets. For example,
`minimal loop reached max iterations` maps to
`diagnostic_code=minimal_loop_max_iterations`, and blocked Bash policy evidence
for compound commands maps to `diagnostic_code=blocked_bash_command_policy`.
This projection is report-only: it does not rerun the model, change tool
policy, or treat the failed run as successful.
When a failed recheck row has a useful diagnostic but no explicit target,
reports may admit an existing target from deterministic verifier/profile
artifact fields such as `verifier_mentioned_paths`, `profile_entrypoints`, or
`profile_integration_artifacts`, but only when that path exists inside the run
workspace and the row is already in a source or route repair context. Reports
must leave the target blank when no such evidence exists.
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
`recheck_summary.tsv` without overwriting the original summary. Recheck rows
also project the runtime job report fields and set `lifecycle_stage` to
`rechecking`. A recheck pass is reported as `completion_source=recheck_success`;
a recheck miss is `completion_source=recheck_failure`. This shows whether the
recheck validated existing evidence rather than implying that the original
runtime job succeeded.
Focused expected-field assertions are also re-evaluated for recheck rows, but
the recheck projection has its own lifecycle vocabulary. `rechecking` is
accepted as the recheck-stage equivalent for the original lifecycle expectation,
`recheck_success` is accepted for an original `runtime_success`, and
`recheck_failure` is accepted for an original `none` completion source. For
failed rows, a non-`ok` recheck `contract_layer` is treated as a recheck
projection of the original non-`ok` contract layer rather than a new runtime
contract decision. An original expected `contract_layer=ok` must still remain
`ok` after recheck.
When broad sign-off is run with `--require-recheck`, focused assertion findings
from `summary.tsv` are superseded by matching `recheck_summary.tsv` rows for the
same case and run. This prevents historical assertion output from overriding
current recheck evidence while still reporting any original focused row that was
not rechecked.

For deterministic fixture and report fixture rows, recheck projection preserves
explicit structured fields before deriving defaults. Field precedence is:
`fixture_fields`, explicit top-level `meta.json` fields, parsed failure
evidence, then reason/return-code derived defaults. This precedence applies to
terminal observation fields and runtime job report fields such as target
admission, repair action planning, completion source, evidence status, and
attempt outcome. Recheck must not overwrite explicit fixture/meta evidence
with a generic verifier failure, generic `failed`, or derived target admission.

The report also includes a `Contract Layers` section derived from the failure
reason. This is a coarse layer map for triage, not a new success criterion. It
helps distinguish failures in planning, execution/tool protocol, profile,
setup bootstrap, verification, eval success checks, and unknown boundaries.
New eval runs also write `failure_category`, `contract_layer`, and the recovery
report fields into `summary.tsv` and each run's `meta.json`. The recovery
fields are `active_job`, `active_job_lifecycle`, `recovery_owner`, `loop_control_action`,
`dispatch_status`, `dispatch_reason`, `candidate_jobs`, `tie_break_reason`,
`target_path`, `target_role`, `selected_failure_cluster`,
`semantic_failure_kind`, `preferred_repair_role`, `weak_verifier_reason`,
`admitted_cluster_targets`, `repair_action`, `tool_policy`,
`attempt_outcome`, `completion_authority_status`,
`completion_source_of_truth`, `evidence_binding_status`,
`evidence_binding_kind`, `completion_evidence_status`,
`evidence_runner_kind`, `freshness_status`, `missing_evidence`,
`failed_evidence`, `failed_bindings`, `stale_evidence`,
`patch_validation_status`, `patch_validation_source`,
`patch_validation_outcomes`, `patch_validation_rejected_paths`,
`mechanical_adapter`, `mechanical_adapter_status`,
`mechanical_adapter_action`, `rollback_admission_status`,
`rollback_reason`, `profile_project_kind`, `profile_manifest_artifacts`,
`profile_entrypoints`, `profile_integration_artifacts`,
`profile_completion_evidence`, `profile_failure_mapping`,
`profile_adapter_families`, `profile_capability_status`, and
`explicit_stop_reason`. The runtime job report fields are written into the same
artifacts so `summary.tsv`, `recheck_summary.tsv`, and generated reports share
one lifecycle vocabulary. When a runtime repair packet contains richer
contract evidence, the eval runner extracts those fields. When a failure is
detected only by the eval success contract, the runner derives a conservative
recovery classification from the deterministic reason and target path.
Setup lifecycle extraction is intentionally explicit. Reports may include
`setup_job_kind`, `setup_target`, `setup_manifest_kind`,
`setup_manifest_path`, `setup_artifact_validation_status`, `setup_readiness`,
`setup_command_authority`, and `setup_failure_signature` so manifest repair,
setup bootstrap, stale setup, and blocked setup policy do not collapse into
generic verifier or source failures.
Task Contract projection fields are also recorded when present:
`task_contract_kind`, `task_contract_status`, `task_contract_lifecycle`,
`task_contract_request_signals`, `task_contract_constraints`,
`task_contract_completion_evidence`, `behavior_obligation_codes`,
`behavior_obligation_status`, `behavior_obligation_owners`,
`behavior_obligation_paths`, and `artifact_role_projection_status`. These are
observability fields for planning/recovery attribution. They do not change eval
success criteria and do not authorize runtime retries. The
`task_contract_completion_evidence` name is intentionally not prefixed with
`expected_` so focused assertion fields can use
`expected_task_contract_completion_evidence` without colliding with observed
column names.

Profile parity fields are recorded when present so Next.js, Rust, Python,
docs, and data profiles can be compared through the same schema. They report
profile project kind, manifest artifacts, entrypoints, integration artifacts,
completion evidence requirements, failure mapping hints, adapter families, and
capability status. Eval treats these as observability data. A missing or
partial value identifies a profile-contract coverage gap; it does not authorize
profile-owned workflow execution or hidden retry.

Artifact completion reports must keep four states distinct:

- `missing_deliverable`: a required artifact is absent from the artifact
  ledger.
- `missing_evidence`: the artifact exists, but no completion evidence path was
  observed.
- `completion_evidence_failed`: the evidence runner or verifier executed and
  failed.
- `evidence_binding_failed`: evidence exists, but is not bound to the required
  proof path.
- `stale_evidence`: evidence exists, but it was produced before the current
  deliverable state or setup/source change that it claims to prove.

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

Focused control-recovery cases may also declare optional `expected_*` fields,
such as `expected_terminal_state`, `expected_contract_layer`,
`expected_active_job`, `expected_repair_action`, or
`expected_runtime_job_kind`. Focused cases that prove artifact boundaries may
also assert `expected_target_role`, `expected_workspace_scope_kind`,
`expected_artifact_ownership`, `expected_artifact_source_of_truth`, and
`expected_rejected_target_reason`. Target/verifier/patch cases may also assert
target candidate/admitted/rejected counts, current excerpt availability,
priority components, patch validation source/outcomes/rejected paths,
mechanical adapter status/action, and rollback admission status/reason. These
fields are assertions against observed eval output only. They are not passed
to runtime prompts, do not authorize repair, and do not change the command sent
to CommandAgent. The
runner records
`expected_assertion_status`, `expected_assertion_count`, and
`expected_assertion_failures` in `summary.tsv` and `meta.json`. Dry-run focused
assertions are reported as `skipped_dry_run` because dry-run workspaces do not
contain runtime evidence.

Focused cases may also declare `matrix_row` and `proof_mode`. `matrix_row`
names the control path being proved, independent of the case id.
`proof_mode=real_llm` means the row is proved by an actual CommandAgent run.
`proof_mode=deterministic_fixture` or `proof_mode=report_fixture` means the
row is an eval-only deterministic failure/report fixture. Fixture rows are
allowed for malformed tool-call shape, stale edit targets, generated-test
weakening rejection, explicit stops, no-progress ledgers, port conflicts,
out-of-scope target rejection, setup manifest validation, and verifier
diagnostic payload extraction. They must not be used to claim model task
quality or runtime implementation success. They exist to prove that already
observed deterministic evidence is classified, projected into recovery/report
fields, and asserted consistently.

Fixture rows may provide `fixture_reason`, `fixture_success`, `fixture_rc`,
and `fixture_fields`. The runner uses these fields only to populate eval
observations and reports for the focused matrix. They are not sent to
CommandAgent prompts, do not add retries, do not run setup, and do not mutate
runtime behavior. The generated report includes a `Focused Matrix` section so
reviewers can see how many rows were proved by real LLM execution versus
deterministic fixtures. Use `scripts/eval_agent_slice.sh --proof-mode
deterministic_fixture` to run only deterministic fixture rows when validating
report assertions without invoking a local model.

## Broad Migration Sign-off

Broad migration sign-off checks that local LLM runs stop with owned,
actionable evidence across smoke, focused, and large case sets. It is not a
hidden runtime controller and it is not a rerun-until-green policy.

Required roots for broad sign-off should include:

- smoke local LLM
- focused control-recovery local LLM
- focused deterministic fixtures
- large local LLM

Each root should have both the original `summary.tsv` and a rechecked
`recheck_summary.tsv`:

```bash
python3 scripts/eval_report.py <root> --cases-dir <cases-dir>
python3 scripts/eval_report.py <root> --cases-dir <cases-dir> --recheck
```

Use `scripts/eval_signoff.py` to apply the shared gate to existing summaries:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=<smoke-root> \
  --root focused=<focused-root> \
  --root focused-fixture=<fixture-root> \
  --root large=<large-root>
```

The sign-off checker reads only `summary.tsv` / `recheck_summary.tsv`. It does
not run CommandAgent, execute setup, mutate workspaces, rerun verifiers, or
change runtime behavior.

The checker fails on:

- unknown terminal states
- `unknown_contract` without an explicit stop reason
- raw `rc:*` failures without a useful diagnostic code
- failed focused expected assertions
- large-case generic source fallback when setup, profile, planning,
  tool-protocol, step-policy, provider, evidence, or completion ownership is
  more accurate
- large-case failures missing active job, owner, repair action, target when
  applicable, evidence binding, completion evidence, or attempt outcome
- large-case failures missing `large_disposition`, disposition reason,
  disposition evidence, or owner/action consistency
- `accepted_external_limitation` without provider, network, or environment
  boundary evidence
- `implementation_blocker` or `split_forward` dispositions that still require
  a later closure phase

Remaining failures can be accepted for migration sign-off only when they are
owned, actionable, and explicitly recorded as owned failed rows or valid
provider/environment limitations. A broad pass rate by itself is not the
sign-off criterion, and a sign-off pass is not the same as large task success.

Focused case directories are discovered recursively so a case set can be
organized by contract layer without changing runner invocation. Use this for
small, targeted E2E matrices such as
`eval/cases/focused/control-recovery`.

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

For a non-timeboxed proof run, pass `--no-timeout` explicitly:

```bash
scripts/eval_large_tasks.sh --no-timeout
```

Use this mode only when a loadmap or sign-off task requires a fresh large proof
root that cannot be satisfied by the normal bounded timeout.

## Verifier Failure Shape

Each verifier failure records:

- `command`: the local command that was attempted
- `reason`: `command_failed:<status>`, `dependency_missing`, or `blocked:<class>`
- `stdout_excerpt` / `stderr_excerpt`: bounded raw output
- `diagnostic_excerpt`: lines likely to matter for repair, such as type errors
  or failed compile messages
- `source_excerpt`: when output references a source location, nearby source
  lines are included with the failing line marked
- `diagnostic_code`: deterministic verifier diagnosis such as
  `rust_compile_error`, `python_import_missing`, `typescript_type_error`,
  `port_in_use`, or `weak_source_grep`
- `diagnostic_failure_kind`: the semantic kind derived from the diagnostic,
  such as `assertion_mismatch`, `compile_or_type_error`,
  `dependency_missing`, or `verifier_contract_failure`
- `observed_expected`: bounded observed/expected pairs when the verifier output
  exposes them
- `affected_cases`: test case or command names affected by the failure
- `candidate_artifacts`: deterministic artifact candidates mentioned by the
  verifier, profile, setup, or contract evidence
- `preferred_repair_role`: implementation, setup, route integration,
  verifier contract, dev server, or another role derived from the diagnostic
- `weak_verifier_reason`: why a verifier command should repair the verifier
  contract instead of source code
- `admitted_cluster_targets`: targets admitted for the selected semantic
  failure cluster
- `unknown_diagnostic_count`: count of verifier failures still classified only
  as `unknown_verifier_failure`

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
session. Tool protocol reporting should also record the normalized failure
source, selected correction action, failed tool, missing field, required
fields, and whether the bounded correction was already spent or exhausted.
Eval reports should keep `tool_args_*` separate from `dependency_missing`,
`profile_verification:*`, semantic checks, and app-quality failures, including
cases where protocol correction succeeds and a later verifier or app-quality
failure remains.

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
- `active_job_lifecycle`
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
- `repair_attempt_count`
- `attempt_outcome_reason`
- `before_signature`
- `after_signature`
- `exhausted_targets`
- `exhausted_roles`
- `exhausted_clusters`
- `no_progress_strategy`
- `repair_state_status`
- `safe_stop_payload`
- `patch_validation`
- `patch_validation_status`
- `patch_validation_outcomes`
- `mechanical_adapter`
- `mechanical_adapter_status`
- `rollback_admission_status`
- `rollback_reason`
- `eval_report_fields`
- `proposed_targets`
- `admitted_targets`
- `rejected_targets`
- `target_source_of_truth`
- `target_ownership_source`
- `target_workspace_scope`
- `target_evidence_freshness`
- `focused_edit_status`
- `current_excerpt_available`
- `target_priority_components`
- `target_conflict_reason`
- `repair_brief`
- `selected_failure_cluster`
- `repair_brief_status`
- `action_envelope_status`
- `allowed_change_kind`
- `allowed_tool_category`
- `repair_root_cause`
- `repair_hypothesis`
- `expected_improvement`
- `target_confidence`
- `must_preserve`
- `disallowed_actions`
- `success_check`
- `repair_plan_rejection_reason`

These fields explain why a repair packet or setup recovery path was selected.
They are not proof that the original ultra plan completed.

Target-admission reports should preserve the funnel: proposed candidates,
admitted candidates, rejected candidates with reasons, selected target,
selected target role, selected failure cluster, repair brief status, and action
envelope status. A wrong target rejected before the model turn should be
reported as a target-admission result, not as a generic verifier failure.

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
- tool protocol details, such as source, selected correction action, failed
  tool, missing field, required fields, correction spent, and correction
  exhausted;
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
  reason instead of treating it as a model-quality failure. A rejected action
  envelope means the selected recovery job/action/tool category did not satisfy
  the deterministic recovery contract and the repair turn was not admitted.

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
