# Phase 19 Concrete Work Plan

## Step 0: Confirm Baseline

Run the current broad sign-off command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Expected baseline:

- no focused findings;
- large findings only:
  - `missing_evidence_binding`;
  - `missing_completion_evidence`;
  - `generic_source_fallback`;
  - `missing_target`.

Record this output in the Phase19 report before implementation.

## Step 1: Add Sign-off Fixture Coverage

Files to inspect:

- `scripts/eval_signoff.py`
- `scripts/eval_runtime_job_report.py`
- `scripts/eval_failure_observation.py`
- `tests/test_eval_report.py`
- any existing sign-off tests under `tests/`

Add fixtures/tests for these rows:

1. Provider timeout row:
   - terminal state: `provider_transport_failed`;
   - active job: provider/eval boundary blocker;
   - owner: provider/eval boundary;
   - action: explicit bounded stop;
   - target: not applicable;
   - evidence fields: field-sensitive not applicable;
   - expected sign-off: no missing owner/action/evidence/target findings.
2. Provider timeout row with no owner/action:
   - expected sign-off: still fails.
3. Profile dependency/version conflict row:
   - terminal state: `profile_contract_failed`;
   - diagnostic code: `profile_verification:nextjs_dependency_version_conflict`;
   - active job: manifest/setup repair;
   - target: `package.json`;
   - target role: `setup_manifest`;
   - expected sign-off: no generic source fallback and no missing target.
4. Profile dependency/version conflict row still mapped to source:
   - expected sign-off: generic source fallback.
5. Failed large row with blank evidence fields:
   - expected sign-off: missing evidence findings.
6. Failed large row with justified non-applicable evidence fields:
   - expected sign-off: accepted only when owner/action/terminal state justify
     the semantics.

Do not update runtime behavior until these tests pin the expected sign-off
semantics.

## Step 2: Implement Field-sensitive Missing Rules

Target:

- `scripts/eval_signoff.py`

Replace broad missing interpretation with field-aware checks:

```text
is_missing_value(field, row)
```

Rules:

- `""`, `unknown`, and `none` remain missing for all required fields.
- `not_applicable` remains missing by default.
- `not_applicable` is accepted for target only when terminal state is provider
  transport/parse failure or another explicit targetless boundary condition.
- `not_applicable` is accepted for evidence fields only when:
  - terminal state is provider/eval boundary;
  - owner/action are present;
  - attempt outcome records blocked/external/explicit stop semantics.
- `not_applicable` is not accepted for profile/setup/verifier/source failures
  that have repairable targets.

This keeps sign-off strict while allowing real external blockers.

## Step 3: Project Timeout Runtime Job Fields

Targets to inspect:

- `scripts/eval_failure_observation.py`
- `scripts/eval_runtime_job_report.py`
- runtime event/report field producers if the fields originate there

Desired projection for timeout rows:

```text
terminal_state=provider_transport_failed
contract_layer=execution_contract or provider/eval boundary equivalent
active_job=provider_transport_blocker
recovery_owner=provider_transport or eval_boundary
selected_action=stop_for_provider_timeout
repair_action=stop_for_provider_timeout
target_admission_status=not_applicable
target_path=not_applicable
target_role=not_applicable
evidence_binding_status=not_applicable
completion_evidence_status=not_applicable
attempt_outcome=blocked_external
explicit_stop_reason=provider_transport_timeout
```

If the existing schema uses different canonical names, use the nearest
existing values and document them in `docs/evaluation.md`.

Unit tests must prove:

- timeout row is not source repair;
- timeout row has owner/action/attempt outcome;
- sign-off accepts target/evidence not-applicable only for this class.

## Step 4: Route Profile Dependency Conflicts To Manifest/setup

Targets to inspect:

- `scripts/eval_failure_observation.py`
- `scripts/eval_runtime_job_report.py`
- `src/agent/step_runner/runtime/repair_loop.rs`
- profile verification and profile failure mapping modules under
  `src/agent/step_runner/`

For diagnostic:

```text
profile_verification:nextjs_dependency_version_conflict
```

Project:

```text
active_job=manifest_repair
recovery_owner=setup or manifest
repair_action=add_missing_manifest_dependency or align_manifest_dependency
selected_action=add_missing_manifest_dependency or align_manifest_dependency
target_path=package.json
target_role=setup_manifest
target_admission_status=admitted
target_source_of_truth=profile_verification
```

Keep the diagnostic mapping deterministic. Do not ask the model to infer this
from prose. The profile diagnostic already says this is a manifest/setup
problem.

Add tests that verify the previous large Next.js row no longer produces:

```text
active_job=source_implementation_repair
recovery_owner=source
target_path=<blank>
```

## Step 5: Complete Evidence Field Projection

Targets:

- `scripts/eval_runtime_job_report.py`
- `scripts/eval_report.py`
- `src/agent/event_protocol.rs` or event adapters if report fields originate
  in runtime events

For failed rows:

- If evidence exists and failed, project `failed`.
- If evidence should exist but is absent, project `missing`.
- If evidence is not applicable due to provider/eval boundary, project
  `not_applicable` plus explicit owner/action/attempt outcome.
- If the row is blocked externally after ownership is complete, project
  `blocked_external` or a documented equivalent.

Do not leave failed large rows as blank or `unknown`.

## Step 6: Complete Target Projection

Targets:

- target admission/report projection modules;
- profile failure mapping;
- `scripts/eval_runtime_job_report.py`;
- `scripts/eval_signoff.py`.

Rules:

- Provider/eval timeout: target optional with explicit target not-applicable.
- Profile dependency/version conflict: target `package.json`,
  role `setup_manifest`.
- Missing deliverable/profile route/source failure: target expected path when
  present in reason, diagnostic payload, candidate artifacts, or profile
  failure mapping.
- Unknown target on target-applicable row remains a sign-off failure.

Add tests for target-applicable and target-not-applicable rows.

## Step 7: Run Local Verification

Run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
bash scripts/eval_smoke.sh
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
```

If `tests/test_eval_signoff.py` does not exist, add or extend the existing
sign-off test location. Do not rely only on a manual sign-off command.

## Step 8: Targeted Large Proof

Use the cheapest proof that exercises the repaired behavior:

1. report/sign-off fixtures for P17-L001/P17-L003;
2. profile failure fixture or targeted large Next.js modify rerun for
   P17-L002/P17-L004;
3. broad sign-off over the fixture roots.

If a live model rerun is required:

```bash
bash scripts/eval_large_tasks.sh \
  --runs 1 \
  --out eval/runs/loadmap2-phase19-large-local-llm \
  --provider ollama \
  --model qwen3.6:27b-coding-nvfp4 \
  --binary target/release/commandagent \
  --timeout-secs 1200
```

Then:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase19-large-local-llm/<root> \
  --cases-dir eval/cases/large

python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase19-large-local-llm/<root> \
  --cases-dir eval/cases/large \
  --recheck
```

## Step 9: Broad Sign-off Proof

Run:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=<phase19-large-root-or-fixture-root>
```

Expected Phase19 result:

- pass, or fail only for explicitly recorded external blockers that already
  carry owner/action/evidence;
- no missing evidence binding;
- no missing completion evidence;
- no generic source fallback;
- no missing target for target-applicable rows.

If the command still reports Phase19-owned findings, Phase19 is not complete.

## Step 10: Update Ledger And Docs

Update:

- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`
- `docs/evaluation.md`
- `docs/architecture.md` if runtime ownership semantics changed
- `docs/known-limitations.md` if external blockers remain
- `docs/eval/loadmap2-phase19-large-recovery-<date>.md`

The Phase19 report must include:

- commit hash and dirty flag;
- proof roots;
- sign-off command;
- final status for P17-L001 through P17-L004;
- any `blocked_external` rationale;
- Phase20 handoff items.

## Step 11: Exit Review

Before marking Phase19 done, verify:

| Question | Required answer |
| --- | --- |
| Are P17-L001 through P17-L004 closed or valid external blockers? | yes |
| Are all large failures owned? | yes |
| Does every target-applicable large failure have a target? | yes |
| Are evidence fields meaningful for all failed large rows? | yes |
| Is `not_applicable` field-sensitive rather than global? | yes |
| Did focused Phase18 proof remain valid? | yes |
| Is migration-complete declaration deferred to Phase20? | yes |

## Completion Result

Phase19 used the existing full large root:

```text
eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Rationale: this root already contained every Phase19-owned blocker
P17-L001 through P17-L004, so rechecking it after the deterministic
projection/sign-off changes directly proves the intended repair surface.
Starting another live large run was not required to prove the four ledger rows
and would add model/environment variance to a reporting-boundary fix.

The final broad sign-off command returned `status: pass`:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

## Review Result Reflected

The plan was reviewed against the recovery rules and adjusted to avoid three
failure modes:

1. Treating timeout as success.
2. Letting `not_applicable` hide missing evidence.
3. Fixing only the sign-off checker while leaving runtime/profile ownership
   projection wrong.

The concrete plan therefore starts with fixture semantics, then fixes
projection, then proves behavior with large sign-off.
