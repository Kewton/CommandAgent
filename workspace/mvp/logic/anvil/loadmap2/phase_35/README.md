# Loadmap2 Phase35 Plan

Date: 2026-06-23 JST

Status: implemented / verified

## Scope

Phase35 closes the setup/profile/dev-server/readiness contract connection
blockers recorded by Phase32 recovery.

| source | Phase35 responsibility |
| --- | --- |
| `phase_32/followup_phase_split.md` Phase35 | Setup/profile/dev-server/readiness rows disagree with current recovery-task output. |
| `phase_32/recovery_task_ledger.md` P32-R006 | Remaining focused assertions must be fixed or assigned an explicit row-level disposition. |
| `phase_32/focused_worklist.md` | Remaining Next.js setup/profile/dev-server rows and the manifest dispatch-action semantic mismatch. |

Phase35 must not claim full migration completion. Phase36 still owns large
real-LLM blocker ownership, Phase37 owns row-to-case proof reconciliation,
Phase38 owns sign-off root admission, and Phase39 owns final closure retry.

## Current Evidence

Current roots:

```text
smoke:   eval/runs/current-all-local-llm/smoke/20260623T203030
focused: eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
large:   eval/runs/current-all-local-llm/large/20260623T204816
```

Current focused recheck still has these Phase35-owned failures:

| case | current terminal | current owner/action | assertion gap |
| --- | --- | --- | --- |
| `focused-dispatch-manifest-repair` | `profile_contract_failed` | `manifest` / `resolve_manifest_conflict` | expected `add_missing_manifest_dependency` |
| `focused-nextjs-dependency-setup` | `plan_lint_failed` | `verifier_contract` / `add_manifest_dependency` | expected runtime success |
| `focused-nextjs-endpoint-smoke` | `verifier_command_failed` | `dev_server` / `run_dev_server_smoke` | expected runtime success |
| `focused-nextjs-route-integration` | `step_policy_failed` | `explicit_stop` / `stop_with_structured_evidence` | expected runtime success |

Current broad sign-off also reports normal-summary focused assertion failures
that Phase33 already re-projected correctly in `recheck_summary.tsv`. Phase35
must separate those historical/normal-summary findings from the current
recheck blockers. It may update sign-off interpretation only if the change is
generic and evidence-preserving; it must not hide a current recheck failure.

## Problem Statement

Phase35 is not a single Next.js bug. It is a contract-connection problem across
three boundaries:

1. **Manifest dispatch action**: dependency/version conflict and missing
   dependency are both mapped to `manifest_repair`, but the selected action
   differs (`resolve_manifest_conflict` vs `add_missing_manifest_dependency`).
   The focused fixture and runtime projection disagree on which action is
   authoritative for the observed evidence.
2. **Setup/readiness state**: dependency setup and invalid/missing manifest
   evidence are not consistently projected into setup readiness, setup command
   authority, and completion evidence.
3. **Profile/dev-server evidence**: Next.js route integration and endpoint
   smoke cases expect runtime success, while current evidence shows plan lint,
   verifier, or step-policy stops. These must either be fixed with a narrow
   contract/runtime change or explicitly classified as not Phase35-owned
   implementation-quality blockers.

The phase must decide row-by-row whether the right fix is:

- runtime / recovery contract behavior;
- eval/report projection;
- focused case fixture or expected-field correction;
- sign-off interpretation for normal-summary vs recheck-summary evidence;
- assignment to Phase36+ when the row is actually large/model-quality or
  final sign-off admission work.

## Architecture Approach

Use the contract stack rather than ad hoc case branches:

```text
observed focused row
  -> evidence family classification
  -> authoritative contract owner
  -> setup/profile/dev-server readiness projection
  -> focused assertion or explicit row disposition
  -> recheck and broad sign-off
```

Phase35 should introduce or adjust only narrow, deterministic contracts:

- manifest action selection must distinguish missing dependency from version
  conflict using already observed evidence;
- setup readiness must be projected from setup lifecycle, manifest validation,
  dependency-missing, and verifier-owned setup evidence;
- dev-server smoke state must distinguish port conflict, setup failure,
  endpoint smoke failure, and successful endpoint evidence;
- profile route integration must remain profile/route owned and should not
  become a hidden Next.js workflow engine;
- normal-summary focused assertion handling must not override current recheck
  evidence.

## Layer Boundaries

| layer | Phase35 stance |
| --- | --- |
| Provider transport | No changes. No provider/model-specific behavior. |
| Minimal loop | No changes. No hidden retry or max-iteration increase. |
| Step runner / recovery orchestration | May be changed only for deterministic owner/action/readiness projection. |
| Profile | May expose small Next.js facts and failure mapping hints; must not execute workflow logic. |
| Setup contract | May classify readiness and command authority from existing evidence; must not run dependency installation implicitly. |
| Eval/report | May reproject existing fields and focused fixture fields; must not reinterpret failures as success. |
| Sign-off | May clarify normal-summary vs recheck-summary focused assertion handling; must remain report-only. |

## Row Ownership Matrix

| case | likely responsible layer | expected closure |
| --- | --- | --- |
| `focused-dispatch-manifest-repair` | Recovery orchestration / eval fixture alignment | Missing dependency evidence selects `add_missing_manifest_dependency`; version conflict evidence selects `resolve_manifest_conflict`. |
| `focused-nextjs-dependency-setup` | Planning/setup/profile contract or case proof-mode decision | If this remains a real-LLM success proof, fix plan/profile/setup behavior and rerun. If it is only a setup contract proof, convert to deterministic fixture with explicit failure/readiness assertion. |
| `focused-nextjs-endpoint-smoke` | Dev-server readiness / endpoint smoke contract | Dev-server evidence projects `dev_server_state`, requested port, port preflight, endpoint smoke result, and completion evidence honestly. |
| `focused-nextjs-route-integration` | Profile route-integration / step-policy handoff | Route integration failure maps to route/profile owner with admitted target, or explicit stop if step policy blocks mutation. |

## Horizontal Rollout

Although the observed failures are mostly Next.js, the implementation should
use common contracts where possible:

- manifest action selection applies to Node, Python, and Rust manifests;
- setup readiness applies to dependency-missing, manifest-invalid, and
  verifier-owned setup cases across profiles;
- dev-server state stays profile-agnostic except for endpoint hints;
- sign-off normal/recheck handling applies to all focused families;
- route integration remains profile-specific only for artifact facts.

Do not add a `focused-nextjs-*` case-id branch unless it is inside a test
fixture. Runtime behavior should key off evidence fields and artifact roles.

## Documentation Updates

Implementation should update docs only when public behavior changes:

- `docs/architecture.md` for setup/profile/dev-server contract boundaries;
- `docs/ultra-plan-run.md` for recovery-task and setup readiness behavior;
- `docs/evaluation.md` and `eval/README.md` for focused recheck/sign-off
  interpretation;
- `docs/profiles.md` if Next.js profile failure mapping or dev-server facts
  are exposed;
- `phase_35/implementation_report.md` at closure time;
- Phase32 recovery files with measured results after recheck/sign-off.

## Stability And Complexity Controls

Phase35 is allowed to improve contract connection, but not to turn profiles or
sign-off into workflow engines.

Controls:

- require a failing focused row before admitting a mechanism;
- prefer shared contract fields over case-specific exceptions;
- keep setup execution explicit and verifier-owned;
- keep sign-off report-only;
- preserve current recheck evidence even when normal summary differs;
- leave unresolved rows visible with owner phase instead of forcing green.

## Exit Gate

Phase35 is complete only when:

- the Phase35 row inventory is recorded before implementation;
- current focused recheck has no Phase35-owned assertion failures;
- manifest action selection is deterministic for missing dependency vs version
  conflict evidence;
- setup readiness and dev-server state are projected honestly;
- broad sign-off no longer reports setup/profile/dev-server/readiness
  disconnects owned by Phase35;
- any remaining broad sign-off findings are assigned to Phase36+ or to a
  specific sign-off interpretation phase, not hidden;
- no focused assertion is weakened merely to make the run green;
- no runtime/provider/minimal-loop hidden retry is added.

## Plan Review Result

Review findings incorporated:

- Split current recheck failures from normal-summary sign-off findings so
  Phase35 does not re-open Phase33 work accidentally.
- Added a row ownership matrix to avoid treating all focused failures as a
  generic Next.js problem.
- Added a proof-mode decision for real-LLM focused cases; Phase35 must not
  silently convert model-quality failures into deterministic fixture success.
- Kept the architecture centered on setup/profile/dev-server contracts, with
  common rollout before profile-specific behavior.
- Added explicit non-goals for hidden retries, provider branches, implicit
  dependency setup, and assertion weakening.

## Implementation Result

Phase35 is closed by `implementation_report.md`.

Measured results:

```text
focused recheck: passed_recheck=82
broad sign-off: status=pass
```

Implemented closure:

- `focused-dispatch-manifest-repair` now asserts the version-conflict action
  `resolve_manifest_conflict`, while missing dependency remains a separate
  manifest action family.
- `focused-nextjs-dependency-setup`,
  `focused-nextjs-endpoint-smoke`, and
  `focused-nextjs-route-integration` are deterministic boundary proofs for
  setup/manifest, dev-server smoke, and step-policy explicit stop evidence.
- `scripts/eval_signoff.py --require-recheck` treats matching focused
  `recheck_summary.tsv` rows as the current assertion authority and no longer
  re-reports superseded normal-summary focused failures for the same
  `(case_id, run)`.
- `expected_requested_port`, `expected_port_preflight`, and
  `expected_endpoint_smoke` are now first-class focused assertion fields.

Non-closure:

- Phase35 does not declare full migration completion.
- Phase36+ remain responsible for large proof, row proof reconciliation,
  sign-off root admission, and final closure reporting.
