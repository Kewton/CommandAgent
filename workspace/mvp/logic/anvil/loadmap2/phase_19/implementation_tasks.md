# Phase 19 Implementation Tasks

## 1. Rebaseline Phase19 Inputs

- [x] Read `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`.
- [x] Read `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`.
- [x] Read
  `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`.
- [x] Confirm P17-L001 through P17-L004 are the only Phase19 rows.
- [x] Capture the current broad sign-off output using the Phase18 focused root.
- [x] Record the baseline sign-off output under the Phase19 work notes.

## 2. Classify Each Large Blocker Before Editing

For each row, record exactly one primary owner and one primary proof route:

- [x] P17-L001 timeout ownership:
  - primary owner: provider/eval boundary;
  - proof route: timeout fixture or large timeout rerun plus sign-off.
- [x] P17-L002 profile failure generic source fallback:
  - primary owner: profile failure mapping / active job arbitration;
  - proof route: large Next.js modify rerun or profile failure fixture.
- [x] P17-L003 evidence binding/completion evidence missing:
  - primary owner: completion evidence / eval report projection;
  - proof route: report fixture plus broad sign-off.
- [x] P17-L004 missing target:
  - primary owner: target admission / eval report projection;
  - proof route: target admission fixture or large Next.js modify rerun.

If a row has multiple independent root causes, split the row before runtime
edits.

## 3. Add Or Update Test Fixtures

- [x] Add fixture coverage for provider/eval timeout rows.
- [x] Add fixture coverage for profile dependency/version conflict rows.
- [x] Add fixture coverage for failed large rows with evidence fields present.
- [x] Add fixture coverage for target-applicable vs target-not-applicable rows.
- [x] Ensure fixtures exercise `scripts/eval_signoff.py`, not just lower-level
  helper functions.

## 4. Implement Timeout Ownership Projection

- [x] Ensure timeout rows classify as provider/eval boundary failures.
- [x] Project a concrete active job such as `provider_transport_blocker`.
- [x] Project a recovery owner such as `provider_transport` or `eval_boundary`.
- [x] Project a selected action / repair action that means explicit bounded
  stop, not source repair.
- [x] Project `attempt_outcome=blocked_external` or equivalent explicit
  non-success outcome.
- [x] Populate evidence binding and completion evidence with field-sensitive
  not-applicable semantics.
- [x] Ensure sign-off accepts those not-applicable fields only for valid
  provider/eval timeout rows.

## 5. Implement Profile Failure Mapping For Large Rows

- [x] Identify deterministic profile diagnostics that imply manifest/setup
  repair.
- [x] Map
  `profile_verification:nextjs_dependency_version_conflict` to
  manifest/setup owner.
- [x] Project `package.json` as target path.
- [x] Project `setup_manifest` as target role.
- [x] Select manifest/setup repair action instead of
  `source_implementation_repair`.
- [x] Keep the profile-specific diagnostic in profile/profile-adapter code, not
  provider transport.
- [x] Add unit tests for the profile diagnostic mapping.

## 6. Complete Evidence Field Projection

- [x] Ensure failed large rows never leave `evidence_binding_status` blank.
- [x] Ensure failed large rows never leave `completion_evidence_status` blank.
- [x] Distinguish `failed`, `missing`, `not_applicable`, and
  `blocked_external` semantics.
- [x] Update report generation so recheck rows preserve evidence semantics.
- [x] Update sign-off checks so unknown/missing still fails.
- [x] Add tests that prove generic `not_applicable` without owner/action is
  rejected.

## 7. Complete Target Projection

- [x] Define target-applicable terminal states for large sign-off.
- [x] Keep provider timeout target-optional.
- [x] Treat profile, setup, dependency, route, verifier, and source failures as
  target-applicable unless deterministic evidence says otherwise.
- [x] Project target path, role, source of truth, and admission status for
  target-applicable rows.
- [x] Add tests for profile manifest target and provider timeout target
  not-applicable behavior.

## 8. Horizontal Regression Checks

- [x] Run Python eval report tests.
- [x] Run sign-off tests.
- [x] Run Rust unit tests touching recovery/report/eval.
- [x] Run `cargo fmt --check`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo test`.
- [x] Run `cargo build --release`.
- [x] Run `bash scripts/eval_smoke.sh`.
- [x] Confirm focused Phase18 rows do not regress.

## 9. Large Proof

- [x] Run a targeted Next.js large modify proof or equivalent profile fixture.
- [x] Run a timeout/report fixture proof for P17-L001/P17-L003.
- [x] Use the existing full large time-boxed root as the Phase19 proof root
  because it contains all P17-L001 through P17-L004 blockers. A new live large
  rerun is deferred to Phase20 migration-complete sign-off to avoid adding
  model/environment variance to a reporting-boundary fix:

```bash
bash scripts/eval_large_tasks.sh \
  --runs 1 \
  --out eval/runs/loadmap2-phase19-large-local-llm \
  --provider ollama \
  --model qwen3.6:27b-coding-nvfp4 \
  --binary target/release/commandagent \
  --timeout-secs 1200
```

- [x] Generate normal and recheck reports:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase19-large-local-llm/<root> \
  --cases-dir eval/cases/large

python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase19-large-local-llm/<root> \
  --cases-dir eval/cases/large \
  --recheck
```

- [x] Rerun broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=<latest-smoke-root> \
  --root focused=<phase18-focused-root> \
  --root focused-fixture=<latest-focused-fixture-root> \
  --root large=<phase19-large-root>
```

## 10. Documentation And Ledger

- [x] Update `docs/evaluation.md` for large sign-off field semantics.
- [x] Review `docs/architecture.md`; no update required because runtime/event
  semantics did not change.
- [x] Review `docs/known-limitations.md`; no update required because eval
  timeout is already documented as a blocker/evidence boundary.
- [x] Add `docs/eval/loadmap2-phase19-large-recovery-<date>.md`.
- [x] Update Phase17 blocking ledger statuses for P17-L001 through P17-L004.
- [x] Record all proof roots and the final sign-off command.

## 11. Phase19 Closure Review

Before closing Phase19, answer:

- [x] Does every failed large row have terminal state, contract layer,
  active job, owner, action, evidence binding status, completion evidence
  status, and attempt outcome?
- [x] Does every target-applicable large row have target path and role?
- [x] Are provider/model/environment limitations explicitly owned and recorded?
- [x] Does sign-off avoid generic source fallback when a better owner exists?
- [x] Are all remaining findings assigned to Phase20 with rationale, or are
  there no remaining findings?

## If Verification Fails

- Same sign-off finding remains: keep the ledger row open and narrow the
  owner/layer before another patch.
- New sign-off finding appears: add it to reconciliation before continuing.
- Same row fails twice after targeted fixes: write a design review note and
  decide whether the row must split.
- Timeout remains after owner/action/evidence is complete: mark
  `blocked_external` only if the provider/model/environment rationale is
  explicit and sign-off no longer reports missing ownership/evidence.

## Completion Criteria

Phase19 is complete only when:

- P17-L001 through P17-L004 are not `open`;
- no large sign-off row is unowned;
- no target-applicable large row lacks a target;
- no failed large row lacks evidence binding or completion evidence semantics;
- no profile/setup/route/verifier failure falls back to generic source repair
  when a better owner is available;
- Phase19 eval report exists;
- Phase20 is the only remaining phase before migration-complete decision.

## Review Result Reflected

- Added row-by-row root-cause classification before editing.
- Added field-sensitive `not_applicable` rules to avoid hiding evidence gaps.
- Added fixture-first proof before live large rerun.
- Added explicit horizontal checks across Next.js, Python/FastAPI, and Rust.
- Kept migration-complete declaration out of Phase19.
