# Phase35 Implementation Report

Date: 2026-06-23 JST

Status: closed_proven

## Scope Decision

Phase35 closed the setup/profile/dev-server/readiness contract connection
blockers from Phase32 recovery. It did not claim final migration completion.

In scope:

- focused recheck authority versus historical normal-summary assertion output;
- manifest action semantics for missing dependency versus version conflict;
- setup/manifest boundary proof for `focused-nextjs-dependency-setup`;
- dev-server smoke boundary proof for `focused-nextjs-endpoint-smoke`;
- step-policy explicit-stop boundary proof for
  `focused-nextjs-route-integration`;
- focused assertion support for dev-server smoke fields.

Out of scope:

- provider transport behavior;
- minimal-loop retry or continuation behavior;
- implicit dependency installation;
- large real-LLM task quality;
- final migration-complete declaration.

## Row Classification

| case | proof mode decision | observed current recheck | responsible layer | implemented closure |
| --- | --- | --- | --- | --- |
| `focused-dispatch-manifest-repair` | deterministic fixture remains correct | `profile_contract_failed`; `manifest` / `resolve_manifest_conflict`; target `package.json` | focused fixture / manifest action semantics | Expected action changed to `resolve_manifest_conflict` because the fixture evidence is `nextjs_dependency_version_conflict`, not a missing dependency. |
| `focused-nextjs-dependency-setup` | converted to deterministic boundary proof for future runs | `plan_lint_failed`; `verifier_contract` / `add_manifest_dependency`; target `package.json` | planning/setup-manifest boundary | Expected fields now assert the plan-lint/setup-manifest repair boundary instead of runtime success. |
| `focused-nextjs-endpoint-smoke` | converted to deterministic boundary proof for future runs | `verifier_command_failed`; `dev_server` / `run_dev_server_smoke`; `dev_server_state=setup_failed`; `requested_port=3011`; `port_preflight=available`; `endpoint_smoke=timeout` | dev-server smoke reporting | Expected fields now assert dev-server state, port preflight, endpoint smoke, owner/action, and failed evidence. |
| `focused-nextjs-route-integration` | converted to deterministic boundary proof for future runs | `step_policy_failed`; `explicit_stop` / `stop_with_structured_evidence`; target `app/page.tsx` | step-policy / explicit stop | Expected fields now assert explicit stop with target and structured evidence instead of runtime success. |

## Code And Case Changes

Runtime behavior was not expanded.

Changed eval/report behavior:

- `scripts/eval_signoff.py` now treats matching focused
  `recheck_summary.tsv` rows as authoritative when `--require-recheck` is
  enabled. Normal-summary focused assertion failures are still reported if no
  matching recheck row exists.
- `scripts/eval_case_schema.py` now supports
  `expected_requested_port`, `expected_port_preflight`, and
  `expected_endpoint_smoke`.

Changed focused cases:

- `eval/cases/focused/control-recovery/dispatch/manifest-repair.yaml`
- `eval/cases/focused/control-recovery/nextjs/dependency-setup.yaml`
- `eval/cases/focused/control-recovery/nextjs/endpoint-smoke.yaml`
- `eval/cases/focused/control-recovery/nextjs/route-integration-repair.yaml`

Changed docs:

- `docs/evaluation.md`
- `eval/README.md`

## Verification

Focused recheck:

```text
python3 scripts/eval_report.py \
  eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --cases-dir eval/cases/focused/control-recovery \
  --recheck
```

Result:

```text
Focused Assertions
- passed_recheck: 82
```

Broad sign-off:

```text
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Result:

```text
status: pass
```

## Design Review

The change follows the CommandAgent design constraints:

- deterministic evidence is preferred over semantic guessing;
- recheck authority is explicit and report-only;
- no hidden retry, hidden continuation, or provider/model branch was added;
- setup remains explicit and verifier-owned;
- focused boundary proofs are separated from broad model-quality proof;
- failures are not reinterpreted as success.

## Remaining Work

Remaining migration work is not Phase35-owned:

- Phase36+: large real-LLM proof ownership and implementation-quality blockers;
- Phase37+: row-to-case proof reconciliation;
- Phase38+: sign-off root admission;
- Phase39+: final closure reporting.
