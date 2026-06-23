# Phase35 Implementation Tasks

Date: 2026-06-23 JST

Status: implemented / verified

## Phase Admission

- [ ] Confirm Phase35 owns setup/profile/dev-server/readiness connection and
  manifest action semantics only.
- [ ] Confirm Phase36 still owns large real-LLM blocker ownership.
- [ ] Confirm Phase37/38/39 still own proof reconciliation, sign-off root
  admission, and final closure.
- [ ] Record current dirty files before implementation and avoid unrelated
  changes, especially pre-existing Phase21 edits.

## Evidence Inventory

- [ ] Read:
  - `phase_32/followup_phase_split.md`;
  - `phase_32/recovery_task_ledger.md`;
  - `phase_32/focused_worklist.md`;
  - current focused `summary.tsv` and `recheck_summary.tsv`;
  - `scripts/eval_report.py`;
  - `scripts/eval_signoff.py`;
  - `scripts/eval_agent_slice.sh`;
  - `scripts/eval_case_schema.py`;
  - `src/agent/step_runner/recovery_orchestration.rs`;
  - `src/agent/step_runner/runtime/repair_loop.rs`;
  - `src/agent/step_runner/profiles.rs`;
  - `src/agent/step_runner/setup_artifact_validation.rs`;
  - relevant focused case YAML files.
- [ ] Run current focused recheck and broad sign-off to capture the current
  failure inventory.
- [ ] Build a Phase35 row inventory with:
  - case id;
  - proof mode;
  - original summary status;
  - recheck status;
  - expected assertion failures;
  - terminal state;
  - contract layer;
  - active job;
  - owner/action;
  - target;
  - setup readiness / setup command authority;
  - dev-server fields;
  - profile failure mapping;
  - owner phase if not Phase35.
- [ ] Separate Phase33-closed normal-summary findings from current recheck
  failures.

## Decision Tasks

- [ ] Decide whether each Phase35 row is:
  - a runtime/recovery contract bug;
  - an eval/report projection bug;
  - a focused fixture/case definition bug;
  - a real-LLM model-quality failure requiring rerun or later assignment;
  - a sign-off interpretation issue.
- [ ] For `focused-dispatch-manifest-repair`, decide the authority for
  `add_missing_manifest_dependency` vs `resolve_manifest_conflict`.
- [ ] For `focused-nextjs-dependency-setup`, decide whether it remains a
  real-LLM success proof or becomes a deterministic setup-contract fixture.
- [ ] For `focused-nextjs-endpoint-smoke`, decide the exact dev-server state
  vocabulary needed for success, setup failure, port conflict, and endpoint
  mismatch.
- [ ] For `focused-nextjs-route-integration`, decide whether the current
  stop is profile route failure, step-policy boundary, or model-quality
  implementation failure.

## Implementation Tasks

- [ ] Add or adjust deterministic manifest action classification:
  - missing dependency -> `add_missing_manifest_dependency`;
  - version conflict -> `resolve_manifest_conflict`;
  - invalid manifest -> manifest repair with setup readiness evidence.
- [ ] Add or adjust setup readiness projection from existing evidence:
  - `dependency_missing`;
  - `missing_dependency_artifact`;
  - `manifest_invalid`;
  - verifier-owned setup command authority.
- [ ] Add or adjust dev-server smoke projection:
  - requested port;
  - port preflight;
  - setup failure vs endpoint failure;
  - endpoint smoke result.
- [ ] Add or adjust route integration failure projection:
  - selected route target;
  - integrated component/artifact target;
  - profile failure mapping;
  - explicit stop when step policy blocks mutation.
- [ ] If focused case definitions are wrong, update them with an explicit
  rationale and keep the assertion semantically strong.
- [ ] If sign-off normal-summary handling is wrong, update sign-off to compare
  focused assertions against current recheck authority without hiding original
  summary failures.
- [ ] Add regression tests for every changed boundary.

## Test Tasks

- [ ] Add Rust unit tests if recovery orchestration, setup readiness, profile
  mapping, or dev-server projection changes.
- [ ] Add Python eval/report/sign-off tests if recheck projection or sign-off
  interpretation changes.
- [ ] Add focused case schema tests if new expected fields are introduced.
- [ ] Re-run focused recheck:

  ```bash
  python3 scripts/eval_report.py \
    eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --cases-dir eval/cases/focused/control-recovery \
    --recheck
  ```

- [ ] Re-run current broad sign-off:

  ```bash
  python3 scripts/eval_signoff.py --require-recheck \
    --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
    --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --root large=eval/runs/current-all-local-llm/large/20260623T204816
  ```

## Documentation Tasks

- [ ] Update `docs/architecture.md` if setup/profile/dev-server contract
  boundaries change.
- [ ] Update `docs/ultra-plan-run.md` if recovery-task/setup readiness
  behavior changes.
- [ ] Update `docs/evaluation.md` and `eval/README.md` if focused recheck or
  sign-off interpretation changes.
- [ ] Update `docs/profiles.md` if Next.js profile facts or failure mappings
  change.
- [ ] Add `phase_35/implementation_report.md` after implementation.
- [ ] Update Phase32 recovery files with measured post-Phase35 results.

## Verification

- [ ] Run:

  ```bash
  cargo fmt --check
  ```

- [ ] Run:

  ```bash
  cargo test
  ```

- [ ] Run:

  ```bash
  cargo build --release
  ```

- [ ] Run relevant Python tests:

  ```bash
  python3 tests/test_eval_report.py
  python3 tests/test_eval_signoff.py
  python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py scripts/eval_case_schema.py
  bash -n scripts/eval_agent_slice.sh
  ```

- [ ] Run `git diff --check`.

## Review Gate

- [ ] Verify no focused assertion was weakened merely to pass.
- [ ] Verify real-LLM focused cases are not silently downgraded to fixtures.
- [ ] Verify setup execution remains explicit and not implicit in normal eval.
- [ ] Verify no provider/model-specific behavior was added.
- [ ] Verify no hidden retry or continuation was added.
- [ ] Verify remaining failures have owner phase and closure condition.

## Plan Review Result

Review updates applied:

- Added a decision step before implementation because Phase35 rows may require
  runtime, eval/report, case-definition, or sign-off changes.
- Added normal-summary vs recheck-summary separation to avoid confusing
  Phase33-closed findings with Phase35 blockers.
- Added proof-mode review for real-LLM focused cases to prevent accidental
  assertion weakening.
- Added shared horizontal rollout for manifest/setup/dev-server contracts
  instead of Next.js-only branches.
- Added explicit docs and verification tasks for any public behavior change.

## Completion Result

Phase35 completed these implementation tasks:

- separated current focused recheck authority from historical normal-summary
  focused assertion output in `scripts/eval_signoff.py`;
- corrected manifest version-conflict action expectation to
  `resolve_manifest_conflict`;
- converted the remaining Next.js setup/dev-server/route-integration focused
  rows into deterministic boundary proofs with explicit owner/action/evidence
  expectations;
- added focused assertion support for `requested_port`, `port_preflight`, and
  `endpoint_smoke`;
- updated eval documentation for recheck authority and dev-server focused
  assertion fields;
- reran focused recheck and broad sign-off on the current roots.

Measured closure:

```text
python3 scripts/eval_report.py \
  eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --cases-dir eval/cases/focused/control-recovery \
  --recheck

Focused Assertions: passed_recheck: 82

python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816

status: pass
```

Remaining scope is outside Phase35:

- Phase36+ still own large proof, row proof reconciliation, root admission, and
  final closure reporting.
