# Phase38 Root Admission Report

Date: 2026-06-24 JST

Status: completed

## Admitted Current Roots

| family | root | observed cases | expected cases | result |
| --- | --- | ---: | ---: | --- |
| smoke | `eval/runs/current-all-local-llm/smoke/20260623T203030` | 3 | 3 | admitted |
| focused | `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236` | 82 | 82 | admitted |
| large | `eval/runs/current-all-local-llm/large/20260623T204816` | 6 | 6 | admitted |
| small | none | 0 | 0 | optional |

Current case coverage:

```text
91/91
```

## Positive Sign-off

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Result:

```text
root_admission_status: pass
root_admission_reason: current_roots_admitted
family_case_counts: focused=82, large=6, small=0, smoke=3
current_case_coverage: 91/91
status: pass
```

## Negative Admission Proof

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root focused-fixture=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Result:

```text
root_admission_status: fail
status: fail
duplicate_root_path
```

This proves the earlier duplicated focused root cannot be interpreted as final
sign-off proof.

## Closure

Phase38 closes root admission only. It does not declare migration completion.
Phase39 still owns final closure retry/reporting using this admitted-root
proof.
