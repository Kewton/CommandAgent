# Phase33 Implementation Report

Date: 2026-06-23 JST

Status: completed / projection blocker closed

## Scope

Phase33 fixed eval/report recheck projection only. It did not change runtime,
minimal-loop behavior, provider transports, profile contracts, setup policy, or
large real-LLM execution.

## Changes

| file | change |
| --- | --- |
| `scripts/eval_report.py` | Added deterministic recheck observation input construction and fixture-field row preservation. |
| `scripts/eval_runtime_job_report.py` | Preserves meaningful explicit evidence, completion, attempt, runtime outcome, target admission, and repair-plan status values before deriving defaults. |
| `tests/test_eval_report.py` | Added regression coverage for fixture projection, explicit stop preservation, observed evidence-state preservation, and explicit target-admission preservation. |
| `docs/evaluation.md` | Documented recheck source precedence for fixture/report rows. |

## Verification

| command | result |
| --- | --- |
| `python3 tests/test_eval_report.py` | passed, 50 tests |
| `python3 -m py_compile scripts/eval_report.py scripts/eval_failure_observation.py scripts/eval_case_schema.py scripts/eval_runtime_job_report.py` | passed |
| `python3 scripts/eval_report.py eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 --cases-dir eval/cases/focused/control-recovery --recheck` | passed command; wrote updated `recheck_summary.tsv` |

`git diff --check` is part of the final commit verification and is recorded by
the commit/PR closeout rather than this intermediate report.

## Focused Recheck Result

| metric | before Phase33 | after Phase33 |
| --- | ---: | ---: |
| focused cases | 82 | 82 |
| focused successes | 9 | 9 |
| `passed_recheck` assertions | 47 | 78 |
| `failed_recheck` assertions | 35 | 4 |

Phase33 closed the eval/report projection-caused failures where existing
`fixture_fields` or explicit `meta.json` values were being replaced by generic
derived report values.

## Remaining Failures

| case | owner | reason |
| --- | --- | --- |
| `focused-dispatch-manifest-repair` | Phase34/35 | Dispatch/action semantic mismatch: expected `add_missing_manifest_dependency`, observed `resolve_manifest_conflict`. |
| `focused-nextjs-dependency-setup` | Phase35 | Setup/profile/readiness connection still reports planning/manifest repair instead of success. |
| `focused-nextjs-endpoint-smoke` | Phase35 | Dev-server/profile readiness connection still reports verifier/dev-server repair instead of success. |
| `focused-nextjs-route-integration` | Phase35 | Profile/route integration and step-policy connection still reports explicit stop instead of success. |

## Exit Decision

Phase33 is closed. Phase32 remains open because current focused recheck still
has four non-Phase33 focused assertion failures, and broad sign-off closure is
owned by later follow-up phases.
