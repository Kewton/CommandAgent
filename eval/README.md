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
  --root focused-fixture=<fixture-root> \
  --root large=<large-root>
```

The sign-off script is report-only. It reads existing summary artifacts and
flags unowned failures, raw diagnostics, failed focused assertions, and large
failures that are missing owner/action/target/evidence fields.
