# Phase34 Implementation Report

Date: 2026-06-23 JST

Status: implemented / verified

## Scope

Phase34 implemented raw diagnostic classification and deterministic target
admission for current eval recheck rows. It did not change runtime behavior,
provider behavior, minimal-loop behavior, profile contracts, setup execution,
or model prompts.

## Changes

- Added evidence-based diagnostic extraction in
  `scripts/eval_failure_observation.py`.
- Added recheck-only target admission from existing verifier/profile artifact
  fields in `scripts/eval_report.py`.
- Added regression coverage in `tests/test_eval_report.py` and
  `tests/test_eval_signoff.py`.
- Updated public eval documentation in `docs/evaluation.md` and
  `eval/README.md`.

## Current Root Result

Before Phase34, `large-rust-app-new` in
`eval/runs/current-all-local-llm/large/20260623T204816/recheck_summary.tsv`
was reported as:

```text
reason=rc:1
diagnostic_code=rc_1
target_path=
target_admission_status=unknown
```

After Phase34 recheck, the row is:

```text
reason=rc:1
diagnostic_code=blocked_bash_command_policy
terminal_state=verifier_command_failed
contract_layer=verification_contract
active_job=source_implementation_repair
recovery_owner=source
repair_action=edit_source_for_diagnostic
target_path=src/main.rs
selected_target=src/main.rs
target_admission_status=admitted
target_source_of_truth=profile_artifact_hint
target_ownership_source=profile_workspace_artifact
evidence_binding_status=bound
completion_evidence_status=failed
```

Evidence source:

- stderr contained `minimal loop reached max iterations`;
- the repair packet contained blocked Bash policy evidence for compound shell
  commands;
- the recheck row had `profile_entrypoints=src/main.rs|src/lib.rs`;
- `src/main.rs` existed inside the run workspace.

## Broad Sign-off

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Result: fail, with focused assertion findings only.

Phase34-owned findings are gone:

- no `raw_undiagnostic_rc` for current roots;
- no large `missing_target` for `large-rust-app-new`.

Remaining findings belong to Phase35+ and remain visible.

## Verification

Passed:

```bash
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
python3 -m py_compile scripts/eval_report.py scripts/eval_failure_observation.py scripts/eval_signoff.py
python3 scripts/eval_report.py eval/runs/current-all-local-llm/large/20260623T204816 --cases-dir eval/cases/large --recheck
```

Expected nonzero:

```bash
python3 scripts/eval_signoff.py --require-recheck --root smoke=... --root focused=... --root large=...
```

The nonzero sign-off result is expected because Phase35+ focused assertion
blockers remain open.

## Boundary Review

- No hidden retry or continuation was added.
- No focused assertion was weakened.
- No runtime success criterion was changed.
- No provider/model-specific branch was added.
- Evidence-poor `rc_1` rows still fail sign-off.
- Target admission requires an existing run-workspace file and deterministic
  verifier/profile evidence.
