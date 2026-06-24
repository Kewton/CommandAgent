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
- `mode`: optional, `plan-run` or `ultra-plan-run`; defaults to `plan-run`
- `intent`: broad task intent
- `prompt`: user-facing task prompt
- `expected_artifacts`: concrete repository-relative files
- `verify`: deterministic local commands when available
- `success_check`: post-run check contract
- `fixture`: optional repository-relative directory copied into each run
  workspace before execution. Use this for modification cases that need an
  existing project.

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
- `small`: future small/medium regression cases
- `large`: six MVP large-task cases covering Next.js, FastAPI, and Rust
- `focused/control-recovery`: focused E2E cases for contract recovery paths

Large cases should usually set `mode: ultra-plan-run`. Modification cases should
use fixtures instead of expecting the model to invent an existing project.

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
case set exactly: 3 smoke rows, 82 focused control-recovery rows, and 6 large
rows. `small` is optional while the current manifest has zero small cases.
Supplemental roots such as `focused-fixture` are not counted toward current
case coverage, and a duplicated root path under another label fails admission.
Historical smaller roots cannot satisfy final-current sign-off.

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
