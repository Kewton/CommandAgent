# Evaluation Cases

Evaluation cases are repository-local YAML files used by CommandAgent eval
scripts. They are designed to be explicit enough for repeatable checks without
turning the benchmark into a pile of hidden heuristics.

## Case Schema

```yaml
id: smoke-docs-readme
title: Update README
profile: docs
style: default
mode: plan-run
intent: docs
prompt: "Create README.md with a short usage note."
evaluation_purpose: task_success
expected_artifacts:
  - README.md
verify:
  - cat README.md
success_check:
  type: semantic
  required_paths:
    - README.md
  must_include:
    README.md:
      - usage
```

Required fields:

- `id`: stable case id
- `profile`: one of the MVP profiles
- `style`: `default`, `tdd`, or `test-hardening`
- `mode`: optional, `minimal`, `plan-only`, `ultra-plan-only`, `plan-run`,
  `ultra-plan-run`, or `run-plan`; defaults to `plan-run`
- `intent`: broad task intent
- `prompt`: user-facing task prompt
- `evaluation_purpose`: optional reporting purpose. Supported values are
  `task_success`, `expected_failure_classification`, `contract_fixture`, and
  `provider_smoke`; defaults to `task_success`.
- `expected_artifacts`: concrete repository-relative files
- `verify`: deterministic local commands when available
- `success_check`: post-run check contract
- `fixture`: optional repository-relative directory copied into each run
  workspace before execution. Use this for modification cases that need an
  existing project.
- `gold_plan_fixture`: required for `mode: run-plan`. The eval runner copies
  this repository-relative step-plan YAML into the run workspace and passes it
  to `/run-plan`, bypassing planner generation while still using the normal
  minimal loop executor.
- `component`: optional reporting dimension. Defaults from `mode` and focused
  path: `minimal_loop`, `planner`, `worker`, `recovery`, `full_agent_plan`, or
  `full_agent_ultra`.

## Semantic Check Policy

Avoid line-count-only checks for large tasks. Prefer semantic checks that
combine:

- required artifact existence
- verifier command success
- required file content signals
- absence of known fake-success patterns

Line count can be used only as a weak auxiliary signal, not as the primary pass
criterion for MVP sign-off.

## Case Sets

- `smoke`: fast cases for runner wiring
- `small`: small/medium task-success regression cases
- `large`: six MVP large-task cases covering Next.js, FastAPI, and Rust
- `large-gold`: the same six large-task surfaces driven by checked-in step
  plans under `eval/gold_plans/large`
- `minimal`: direct one-shot minimal-loop cases that do not pass through a
  slash command
- `planner`: planner-only cases using `/plan-steps` or `/ultra-plan`
- `focused/control-recovery`: focused E2E cases for contract recovery paths

Large cases should usually set `mode: ultra-plan-run`. Modification cases should
use fixtures instead of expecting the model to invent an existing project.
Gold-plan cases should set `mode: run-plan` and are used to separate
planner/plan-lint quality from worker/tool-interface behavior.

Minimal cases should set `mode: minimal`. The runner passes `prompt` directly
to the CommandAgent one-shot minimal loop instead of rendering `/plan-run` or
`/ultra-plan-run`.

Planner-only cases should set `mode: plan-only` or `mode: ultra-plan-only`.
The runner renders `/plan-steps` or `/ultra-plan` and treats a saved plan under
`.commandagent/plans` as the artifact under evaluation. These cases do not
require final task artifacts to be created.

Worker cases should usually use `mode: run-plan` with `gold_plan_fixture`.
Recovery cases live under `focused/control-recovery` and are reported with
`component=recovery` unless they are specifically planning-focused.

## Run Artifacts

Each run writes compatibility files plus normalized trace files:

- `summary.tsv`: headline-compatible tabular report.
- `command.json`: exact subprocess command and eval trace environment.
- `events.jsonl`: versioned runtime events from `COMMANDAGENT_EVENT_JSONL`.
- `model_io.jsonl`: eval-only model request/response trace from
  `COMMANDAGENT_MODEL_IO_JSONL`.
- `plans.jsonl`, `steps.jsonl`, `model_calls.jsonl`, `tool_calls.jsonl`,
  `artifacts.jsonl`, `verifier_runs.jsonl`, and `recoveries.jsonl`: derived
  event slices.
- `workspace_before.json`, `workspace_after.json`,
  `artifact_changes.jsonl`, and `changes.patch`: workspace delta evidence.
- `runtime_result.json`: observed runtime/event-derived causal summary.
- `evaluator_result.json`: evaluator success checks, staged success levels,
  plan quality, and derived attribution.
- `recheck_result.json` and `recheck_delta.json`: written only by
  `eval_report.py --recheck`; recheck is derived analysis and does not change
  runtime facts.

Root-level `manifest.json` records case hashes and eval script hashes.
`cases.snapshot/` preserves the exact case YAML set used for the run.

## Causal Fields

`summary.tsv` keeps old columns and adds causal/debug columns:

- `component`
- `first_actionable_divergence`
- `first_divergence_event_id`
- `first_divergence_phase_id`
- `first_divergence_step_id`
- `last_successful_contract`
- `last_successful_action`
- `last_successful_artifact`
- `planner_requests`, `worker_requests`, `model_requests`
- `tool_calls`, `artifact_changes`, `verifier_runs`, `recovery_attempts`
- `input_tokens`, `output_tokens`
- `observed_*`, `derived_*`, and `rechecked_*` fields for key owner/target
  and attempt values

`observed_*` values come from runtime events. `derived_*` values come from eval
projection. `rechecked_*` values come from `--recheck` and must be treated as
analysis, not runtime source of truth.

## Plan Quality Fields

For `plan-only`, `ultra-plan-only`, `plan-run`, and `ultra-plan-run`, the
runner inspects generated YAML under `.commandagent/plans` and records plan
quality fields in `summary.tsv` and `evaluator_result.json`.

- `plan_quality_responsibility_score`: whether requested final artifacts are
  owned by a step `expected_paths` or ultra phase `owned_artifacts`.
- `plan_quality_clarity_score`: whether step instructions and phase goals are
  present and concrete enough to inspect.
- `plan_quality_granularity_score`: whether steps/phases avoid overly broad
  ownership units.
- `plan_quality_verifier_separation_score`: whether heavy build/test
  verifiers are separated from mutation steps.
- `plan_quality_status`: `pass`, `warn`, `fail`, `missing_plan`, or
  `not_applicable`.

These fields evaluate planner output separately from task execution success.
They do not make the runner retry, rewrite plans, or change runtime behavior.

## Focused Case Assertions

Focused cases can declare optional `expected_*` fields. These fields are
eval-only assertions against the observed `summary.tsv` and `meta.json` fields
after a run. They are never passed to `/plan-run`, `/ultra-plan-run`, prompts,
or runtime repair logic.

Supported fields:

- `expected_terminal_state`
- `expected_contract_layer`
- `expected_failure_class`
- `expected_task_contract_lifecycle`
- `expected_task_contract_request_signals`
- `expected_task_contract_constraints`
- `expected_task_contract_completion_evidence`
- `expected_behavior_obligation_codes`
- `expected_behavior_obligation_status`
- `expected_behavior_obligation_owners`
- `expected_behavior_obligation_paths`
- `expected_artifact_role_projection_status`
- `expected_active_job`
- `expected_recovery_owner`
- `expected_dispatch_status`
- `expected_repair_action`
- `expected_recovery_task_started`
- `expected_target_role`
- `expected_target_source_of_truth`
- `expected_target_ownership_source`
- `expected_target_evidence_freshness`
- `expected_focused_edit_status`
- `expected_target_conflict_reason`
- `expected_target_candidate_count`
- `expected_target_admitted_count`
- `expected_target_rejected_count`
- `expected_current_excerpt_available`
- `expected_target_priority_components`
- `expected_workspace_scope_kind`
- `expected_artifact_ownership`
- `expected_artifact_source_of_truth`
- `expected_rejected_target_reason`
- `expected_runtime_job_kind`
- `expected_runtime_job_outcome`
- `expected_setup_state`
- `expected_dev_server_state`
- `expected_requested_port`
- `expected_port_preflight`
- `expected_endpoint_smoke`
- `expected_completion_authority_status`
- `expected_freshness_status`
- `expected_evidence_binding_status`
- `expected_completion_evidence_status`
- `expected_attempt_outcome`
- `expected_explicit_stop_reason`
- `expected_patch_validation_status`
- `expected_patch_validation_source`
- `expected_patch_validation_outcomes`
- `expected_patch_validation_rejected_paths`
- `expected_mechanical_adapter`
- `expected_mechanical_adapter_status`
- `expected_mechanical_adapter_action`
- `expected_rollback_admission_status`
- `expected_rollback_reason`
- `expected_contract_conflict_status`
- `expected_contract_conflict_sides`
- `expected_contract_conflict_authority`
- `expected_contract_conflict_repair_target_side`
- `expected_contract_conflict_selected_action`
- `expected_contract_conflict_safe_stop_reason`
- `expected_contract_conflict_missing_evidence`
- `expected_contract_conflict_source_of_truth`
- `expected_phase29_support_rows`
- `expected_language_repair_adapter_status`
- `expected_effective_tool_policy`
- `expected_effective_tool_policy_status`
- `expected_tool_failure_recovery_status`
- `expected_setup_command_classification`
- `expected_failed_tool`
- `expected_blocked_command`
- `expected_command_class`
- `expected_command_authority`
- `expected_command_classification_reason`
- `expected_workspace_candidate_status`
- `expected_workspace_ignored_dir_policy`
- `expected_workspace_candidate_ignored_reasons`
- `expected_job_report_status`
- `expected_job_report_owner_action`
- `expected_scaffold_contract_status`
- `expected_noncoding_evidence_status`
- `expected_answer_work_mode_status`
- `expected_lifecycle_projection_status`
- `expected_provider_boundary_status`

Reports render `expected_assertion_status`,
`expected_assertion_count`, and `expected_assertion_failures`. Dry runs mark
focused assertions as `skipped_dry_run` because no runtime evidence has been
produced.

Focused cases may also set:

- `matrix_row`: stable name for the control path under evaluation
- `proof_mode`: one of `real_llm`, `deterministic_fixture`, or
  `report_fixture`

Fixture proof modes are eval-only. They can prove deterministic
classification and report projection, but they must not be counted as broad
model-quality or runtime task-completion proof.

Focused case directories are read recursively, so a case set can group files by
contract layer, for example:

```text
eval/cases/focused/control-recovery/
  planning/
  tool-protocol/
  completion/
  nextjs/
  python/
  rust/
  docs/
  data/
  recovery-policy/
```

## Broad Sign-off

Broad migration sign-off should run smoke, focused, focused fixture, and large
case roots with normal and `--recheck` reports. Use:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=<smoke-root> \
  --root focused=<focused-root> \
  --root large=<large-root>
```

The sign-off script is report-only. It reads existing summary artifacts and
flags unowned failures, raw diagnostics, failed focused assertions, and large
failures that are missing owner/action/target/evidence or row disposition
fields.

Final-current sign-off first admits the root bundle before row findings are
interpreted. The current final bundle must provide unique root labels and root
paths, must include `smoke`, `focused`, and `large`, and must cover the current
required case set exactly: 3 smoke rows, 83 focused control-recovery rows, and
6 large rows. `small` roots are optional task-success evidence and are reported
separately from required current-case coverage. Supplemental roots such as
`focused-fixture` are not counted toward current case coverage, and a
duplicated root path under another label fails admission. Historical smaller
roots cannot satisfy final-current sign-off.

Each eval root includes `summary.tsv` and `environment.json`. The environment
file records commit, dirty state, dirty diff hash, binary hash,
provider/model, `OLLAMA_HOST`, best-effort Ollama version/model digest, timeout
mode, and proof-mode filters for reproducibility.

The sign-off output separates control accounting from task completion:

- `control_contract_signoff`: whether root admission, expected focused
  assertions, failure attribution, and large dispositions satisfy the control
  gate.
- `task_completion_signoff`: whether task-success families that were supplied
  completed successfully. `closed_owned_failure` remains a task failure even
  when it is acceptable for control sign-off.
- `smoke_task_success`, `small_task_success`, `large_task_success`, and
  `focused_assertion_pass`: headline counters for interpreting those two gates.

`--recheck` summaries may classify raw process-code failures from existing
stderr/stdout/repair-packet evidence and may admit targets from existing
verifier/profile artifact fields when the file exists in the run workspace.
This is attribution for sign-off; it must not rerun cases, mutate workspaces,
or infer a target from task intent alone.
For focused roots, `--require-recheck` treats a matching
`recheck_summary.tsv` row as the authoritative assertion row for the same
`case_id` and `run`. Original `summary.tsv` focused assertion failures are
still reported when no matching recheck row exists, but they are not reported
again when the current recheck row already carries the latest assertion result.

Large proof runs that are intended to close an eval-timeout blocker may use
`scripts/eval_large_tasks.sh --no-timeout`. This must be called explicitly and
the resulting root must record `timeout_mode=none`; normal broad sign-off should
continue to use bounded runs unless a phase plan requires non-timeboxed proof.

Failed large rows must report one of `closed_owned_failure`,
`implementation_blocker`, `accepted_external_limitation`, or `split_forward`.
Only `closed_owned_failure` with consistent owner/action/target/evidence, or a
provider/network/environment-backed `accepted_external_limitation`, can pass
broad sign-off. `implementation_blocker` and `split_forward` remain open
findings. These dispositions classify why a failed large row is acceptable or
blocked for migration accounting; they do not mean the large user task passed.
