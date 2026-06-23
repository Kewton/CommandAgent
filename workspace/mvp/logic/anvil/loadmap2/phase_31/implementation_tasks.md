# Phase31 Implementation Tasks

Date: 2026-06-23 JST

Status: completed / closed_proven

## Phase Admission

- [x] Confirm KI-010 / `P20-LEDGER-001` is still assigned to Phase31.
- [x] Confirm `P17-L001` is the only selected Phase31 row.
- [x] Record unrelated dirty files before implementation and keep them out of
  Phase31 commits.
- [x] Confirm Phase31 is not responsible for changing runtime behavior or
  declaring final migration completion.

## Source And Evidence Alignment

- [x] Reconcile Phase31 against prior evidence:
  - `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`;
  - `workspace/mvp/logic/anvil/loadmap2/phase_19/*`;
  - `docs/eval/loadmap2-phase19-large-recovery-20260623.md`;
  - `docs/eval/loadmap2-phase20-final-migration-decision-20260623.md`;
  - `eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149`.
- [x] Confirm what Phase19 proved: owner/action/evidence attribution, not
  completion.
- [x] Confirm what Phase31 must prove: fresh large completion root.

## Proof Route Selection

- [x] Determine whether current eval scripts can run a truly non-timeboxed
  large proof.
- [x] If not, choose one eval-only support change:
  - add `--no-timeout`; or
  - document and implement `--timeout-secs 0` as no subprocess timeout.
- [x] Keep no-timeout support explicit and eval-only; do not change runtime,
  provider, minimal-loop, profile, or recovery policy.
- [x] Record the proof route in `row_closure_matrix.md` before running large
  proof.

## Closed-Proven Path

- [x] Build or identify the release binary used for proof.
- [x] Run a fresh large eval root using the selected no-timeout or
  non-timeboxed proof mode.
- [x] Run:
  `python3 scripts/eval_report.py <large-root> --cases-dir eval/cases/large --recheck`.
- [x] Run broad sign-off with the fresh large root and current smoke/focused
  proof roots.
- [x] Mark `P17-L001` `closed_proven` only if the root completes and sign-off
  has no unowned large finding.

## Documentation Updates

- [x] Update `docs/evaluation.md` if no-timeout proof support is added.
- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only if the
  proof status changes the final migration surface.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/README.md`,
  `recovery_plan.md`, and `current_issue_phase_map.md` after proof.
- [x] Add `implementation_report.md` at closure time.

## Verification

- [x] Always run `git diff --check`.
- [x] If eval script behavior changes, run eval script unit tests and a dry-run
  large eval.
- [x] If a fresh large root is produced, run large recheck and broad sign-off.
## Review Gate

- [x] Verify Phase31 did not weaken semantic checks or classify timeout as
  success.
- [x] Verify no unbounded runtime retry, hidden continuation, or provider/model
  policy was added.
- [x] Verify Phase32 can consume the Phase31 result without reinterpreting
  prose.

## Plan Review Result

Review updates applied:

- Added explicit proof-route selection before any eval work.
- Added eval-only no-timeout support as a possible prerequisite rather than
  pretending the current timeout option is non-timeboxed proof.
- Removed `blocked_external` as a completion branch for this phase; failed
  proof attempts keep Phase31 open.
