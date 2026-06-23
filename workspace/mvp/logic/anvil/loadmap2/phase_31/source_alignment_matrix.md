# Phase31 Source Alignment Matrix

Date: 2026-06-23 JST

Status: completed / closed_proven

Phase31 is a ledger-proof phase, not a coverage-row source-port phase. The
alignment below maps evidence sources, eval scripts, and prior phase reports to
the `P17-L001` proof responsibility.

| row | evidence/source | current responsibility | Phase31 use | adopted behavior | omitted behavior | proof method |
| --- | --- | --- | --- | --- | --- | --- |
| P17-L001 | `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md` | Original timeout blocker definition. | Source of the row, owner, and closure requirement. | Keep row-level blocker identity. | Do not broaden into all large quality failures. | Reconcile row into Phase31 ledger. |
| P17-L001 | `docs/eval/loadmap2-phase19-large-recovery-20260623.md` | Proves large timeout ownership/evidence attribution. | Baseline that owner/action/evidence exists. | Reuse attribution proof. | Do not treat attribution as completion proof. | Compare Phase31 root against Phase19 result. |
| P17-L001 | `eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149` | Old large timeboxed root. | Historical root only. | Use only as prior evidence. | Do not use as fresh non-timeboxed proof. | Report/recheck/sign-off only as baseline. |
| P17-L001 | `scripts/eval_large_tasks.sh` and `scripts/eval_agent_slice.sh` | Large eval runner and per-case timeout implementation. | Candidate proof runner. | Add explicit eval-only no-timeout support if needed. | Do not change runtime behavior or retry policy. | Dry-run, fresh large root, recheck. |
| P17-L001 | `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624` | Fresh large proof root. | Closure authority for Phase31. | Use root with `timeout_mode=none`, recheck, and broad sign-off. | Do not treat large task success rate as migration completion. | `closed_proven`. |
| P17-L001 | `scripts/eval_report.py` and `scripts/eval_signoff.py` | Recheck and broad sign-off over recorded roots. | Phase31 proof checker. | Use to verify owner/action/evidence or closed proof. | Do not make sign-off the sole row proof. | Recheck summary and sign-off result. |

## Review Notes

- There is no Anvil runtime source to port in Phase31. The row is about the
  quality of proof for already migrated responsibilities.
- Eval script changes are admissible only if they express proof mode more
  honestly. They must not weaken semantic checks or hide timeouts.
