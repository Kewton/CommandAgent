# Loadmap2 Phase31 Plan

Date: 2026-06-23 JST

Status: completed / closed_proven

## Scope

Phase31 closes `P20-LEDGER-001` / KI-010:

| row | responsibility | Phase31 decision target |
| --- | --- | --- |
| P17-L001 | Large timeout proof remains external-limited unless a non-timeboxed run proves completion. | Convert to `closed_proven` with a fresh non-timeboxed proof root. |

This phase is a proof phase, not a runtime behavior phase. It must not change
large-task semantics, weaken eval checks, hide timeouts, or claim migration
completion. Phase32 owns the final migration decision.

## Problem Statement

Phase19 made large timeout rows attributable: timeout rows now have
owner/action/evidence and broad sign-off can distinguish provider/eval timeout
from missing migration ownership. Phase20 still could not declare migration
complete because `P17-L001` was not pure completion proof. It was valid
`blocked_external` evidence, not proof that the large cases complete.

Phase31 must close this exact remaining proof gap:

- produce a fresh large eval root that is not the old short timeboxed root;
- prove the run was not stopped by the eval harness timeout; and
- close `P17-L001` as `closed_proven` from that root and sign-off evidence.

## Non-goals

- Do not make large eval pass by weakening semantic checks.
- Do not increase retries or add hidden continuation.
- Do not run dependency setup implicitly as part of proof.
- Do not reclassify timeout as success.
- Do not change provider/model behavior policy.
- Do not use broad sign-off alone as proof of completion.
- Do not declare final migration completion.

## Design Alignment

Phase31 follows the established recovery proof boundary:

```text
known timeout ledger row
  -> explicit no-timeout proof mode
  -> fresh large root
  -> recheck/sign-off over recorded artifacts
  -> row disposition for Phase32
```

The minimal loop remains unchanged. Eval owns evidence collection and
reporting. Any no-timeout capability must be explicit, eval-only, and
observable. It must not become runtime retry behavior.

## Proof Strategy

Phase31 uses a single closure path: `closed_proven` from a fresh large proof
root. External limitation is not the chosen completion path for this phase.
The proof was produced at
`eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624`.

Required evidence:

- command used to create the root;
- binary path, commit hash, provider, model, timeout/no-timeout mode;
- large root path;
- `summary.tsv` and `recheck_summary.tsv`;
- `scripts/eval_report.py <root> --cases-dir eval/cases/large --recheck`;
- broad sign-off including the fresh large root;
- no unowned large findings.

Actual proof:

- large root:
  `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624`
- recheck: `recheck_summary.tsv` regenerated from repair evidence
- broad sign-off: `status: pass`
- timeout mode: `none`

## Architecture Considerations

If the current eval harness cannot express non-timeboxed proof, Phase31 should
first add the smallest eval-only mechanism needed to express it, for example:

- a visible `--no-timeout` option in `scripts/eval_agent_slice.sh`; or
- a documented `--timeout-secs 0` convention meaning no subprocess timeout.

This must be limited to eval scripts and docs. It must not alter runtime,
minimal-loop, provider, profile, or recovery behavior.

## Horizontal Expansion

Phase31 should cover the full large set, not only the timeout case that first
exposed the blocker:

- Next.js large cases;
- Python/FastAPI large cases;
- Rust large cases;
- provider/eval timeout rows, which must disappear from the fresh no-timeout
  proof root;
- non-timeout large failures with owner/action/evidence.

The proof package should record whether each failed row is completion failure,
provider/model throughput limitation, setup/profile/verifier failure, or
another already-owned contract stop.

## Documentation Updates

Phase31 implementation should update:

- `docs/eval/legacy-control-stack-coverage-20260621.md` only if the final
  migration proof surface changes.
- `docs/evaluation.md` if a no-timeout eval mode is added.
- `workspace/mvp/logic/anvil/loadmap2/README.md`,
  `recovery_plan.md`, and `current_issue_phase_map.md` after proof.
- `phase_31/implementation_report.md` at closure time.

## Exit Gate

Phase31 is complete only when `P17-L001` is:

- `closed_proven` with a fresh non-timeboxed or no-timeout-equivalent large
  proof root and recheck/sign-off results.

It is not complete if the only evidence is the old Phase16 timeboxed root, a
generic timeout label, broad sign-off without a row-specific proof packet, or
an external-limitation packet.

## Plan Review Result

Self-review findings incorporated into this package:

- The plan separates proof-route selection from runtime behavior so Phase31
  does not become another recovery mechanism.
- The plan does not assume the current eval harness can express no-timeout
  proof; it makes that an explicit eval-layer task if needed.
- The selected completion path is fresh large proof only; external limitation
  remains a blocker note, not a Phase31 completion result.
- Implementation review confirms the selected path completed and Phase32 now
  receives a `closed_proven` handoff.
