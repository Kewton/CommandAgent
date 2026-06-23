# Phase34 Implementation Tasks

Date: 2026-06-23 JST

Status: implemented / verified

## Phase Admission

- [x] Confirm Phase34 owns raw diagnostic classification and sign-off
  admission only.
- [x] Confirm Phase35 owns setup/profile/dev-server/readiness contract
  connection.
- [x] Confirm Phase36 owns large real-LLM blocker ownership after raw
  diagnostic classification.
- [x] Record current dirty files before implementation and avoid unrelated
  changes, especially pre-existing Phase21 edits.

## Evidence Inventory

- [x] Read:
  - `phase_32/followup_phase_split.md`;
  - `phase_32/recovery_task_ledger.md`;
  - `phase_32/focused_worklist.md`;
  - current `smoke`, `focused`, and `large` `recheck_summary.tsv` roots;
  - `scripts/eval_signoff.py`;
  - `scripts/eval_report.py`;
  - `scripts/eval_failure_observation.py`;
  - `docs/evaluation.md`.
- [x] Run current broad sign-off and save the Phase34-owned finding inventory.
- [x] Build a raw diagnostic inventory with:
  - family;
  - case id;
  - run;
  - reason;
  - diagnostic code;
  - terminal state;
  - contract layer;
  - active job;
  - owner;
  - action;
  - target path / selected target;
  - available repair packet fields;
  - available workspace/profile artifact hints;
  - owner phase if not Phase34.
- [x] Confirm current Phase34-owned raw diagnostic rows are limited to
  `large-rust-app-new`, or record additional rows if recheck changes.
- [x] Split normal-summary focused assertion failures away from Phase34 unless
  they are raw diagnostic admission failures.

## Diagnostic Classification Design

- [x] Define a deterministic raw diagnostic classifier for existing evidence:
  - `minimal_loop_max_iterations`;
  - `blocked_bash_command_policy`;
  - `turn_error`;
  - fallback `unknown_verifier_failure` only when evidence is genuinely
    insufficient.
- [x] Define diagnostic precedence:
  1. structured `diagnostic_code` from repair packet or meta;
  2. explicit tool-policy / blocked Bash evidence;
  3. minimal-loop max-iteration evidence;
  4. command/verifier diagnostic excerpt;
  5. raw `rc_*` fallback.
- [x] Define target admission source precedence:
  1. explicit `target_path` / `repair_target`;
  2. selected target / admitted cluster target;
  3. verifier-mentioned path;
  4. profile entrypoint/integration artifact that exists in workspace;
  5. explicit non-target disposition for tool-policy/provider/eval boundary
     only.
- [x] Define how sign-off should treat a tool-policy or loop-boundary row with
  no source target:
  - accepted only if active job, owner, action, attempt outcome, and explicit
    stop / boundary reason are present;
  - otherwise remains `missing_target`.
- [x] Keep the classifier profile-independent and case-id-independent.

## Implementation Tasks

- [x] Add a focused helper in eval/report code, not in provider or runtime
  transport.
- [x] If implementation belongs in `scripts/eval_failure_observation.py`,
  keep it as deterministic field extraction.
- [x] If implementation belongs in `scripts/eval_report.py`, keep it limited
  to recheck projection from existing run artifacts.
- [x] If implementation belongs in `scripts/eval_signoff.py`, keep it limited
  to admission of already-classified rows; do not make sign-off infer broad
  semantic repairs.
- [x] Add regression tests for:
  - raw `rc:1` plus `minimal loop reached max iterations` maps to useful
    diagnostic instead of `rc_1`;
  - blocked Bash policy evidence maps to tool-policy diagnostic;
  - target admission does not invent a target when no deterministic target
    source exists;
  - existing workspace/profile entrypoint can be admitted only when the
    diagnostic owner/action supports that target;
  - sign-off still fails raw `rc_1` rows that lack useful evidence.
- [x] Re-run current large recheck and broad sign-off after implementation.
- [x] Assign all non-Phase34 sign-off findings to Phase35+ with owner layer and
  proof command.

## Documentation Tasks

- [x] Update `docs/evaluation.md` if raw diagnostic classification becomes
  public eval behavior.
- [x] Update `eval/README.md` if broad sign-off admission semantics are
  clarified.
- [x] Add `phase_34/implementation_report.md` after implementation.
- [x] Update `phase_32/recovery_task_ledger.md` P32-R007 with measured result.
- [x] Update `phase_32/followup_phase_split.md` only if Phase34 changes the
  remaining phase split.
- [x] Do not mark Phase32 complete from Phase34 alone.

## Verification

- [x] Run:

  ```bash
  python3 tests/test_eval_report.py
  ```

- [x] Run:

  ```bash
  python3 tests/test_eval_signoff.py
  ```

- [x] Run:

  ```bash
  python3 -m py_compile scripts/eval_report.py scripts/eval_failure_observation.py scripts/eval_signoff.py
  ```

- [x] Re-run current large recheck:

  ```bash
  python3 scripts/eval_report.py \
    eval/runs/current-all-local-llm/large/20260623T204816 \
    --cases-dir eval/cases/large \
    --recheck
  ```

- [x] Re-run current broad sign-off:

  ```bash
  python3 scripts/eval_signoff.py --require-recheck \
    --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
    --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --root large=eval/runs/current-all-local-llm/large/20260623T204816
  ```

- [x] Run `git diff --check`.

## Review Gate

- [x] Verify no focused expected assertion was weakened or removed.
- [x] Verify no runtime/minimal-loop/provider/profile/setup behavior changed.
- [x] Verify no hidden retry, hidden continuation, or provider/model-specific
  branch was added.
- [x] Verify every raw diagnostic closure is backed by deterministic evidence.
- [x] Verify target admission is deterministic and not inferred from task
  intent alone.
- [x] Verify remaining sign-off findings are not hidden; they are assigned to
  Phase35+.

## Implementation Result

Phase34 closed its owned current-root findings:

- before: `large-rust-app-new` had `diagnostic_code=rc_1`, no target, and
  sign-off findings `raw_undiagnostic_rc` plus `missing_target`;
- after: `large-rust-app-new` has
  `diagnostic_code=blocked_bash_command_policy`,
  `target_path=src/main.rs`, and `target_admission_status=admitted`;
- remaining broad sign-off failures are focused assertion failures assigned to
  later phases.

The final repository verification pass completed with `git diff --check`
passing.

## Plan Review Result

Review updates applied:

- Added a mandatory raw diagnostic inventory before implementation so Phase34
  cannot over-claim from one observed row.
- Added diagnostic and target source precedence to keep the mechanism
  deterministic and bounded.
- Split sign-off admission from runtime repair behavior to avoid introducing a
  hidden control loop.
- Added tests that preserve failure behavior for evidence-poor raw `rc` rows.
- Added documentation tasks only for public eval/sign-off semantics, avoiding
  unnecessary design churn.
