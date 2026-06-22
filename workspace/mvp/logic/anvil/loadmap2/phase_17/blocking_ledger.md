# Phase 17 Blocking Ledger

## Purpose

This ledger is the phase gate for recovery after the Phase 16 broad sign-off
failure. A row is not closed by implementation, CI, or documentation alone. It
is closed only by the listed proof command or by an explicit accepted
limitation.

The companion reconciliation file is:

```text
workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md
```

The ledger defines remediation rows. The reconciliation file proves that every
current sign-off finding maps into those rows.

## Status Legend

- `open`: not fixed or not proven
- `in_progress`: implementation underway
- `blocked_external`: cannot be proven without external environment/provider
  change
- `closed_proven`: proof command passed
- `deferred_with_rationale`: intentionally assigned beyond Phase19 with a
  written reason

## Blocking Rows

| id | source_root / family | case | observed failure | expected behavior | owning layer | failed contract | suspected module | phase | proof command | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P17-F001 | focused local LLM root | `focused-docs-literal-mismatch` | Expected source repair, observed explicit stop / step policy failure | Docs literal mismatch should select an admitted docs/source repair or explicit contract-conflict stop with matching expected assertion | recovery task / step policy | recovery task contract | `src/agent/step_runner/*`, eval focused assertion mapping | Phase18 | focused case rerun plus full focused sign-off | expected assertion passes; no wrong owner/action | closed_proven |
| P17-F002 | focused local LLM root | `focused-nextjs-dependency-setup` | Expected completed setup, observed explicit stop | Dependency setup row should complete or stop as setup-owned accepted limitation | setup / recovery task | setup contract | profile/setup mapping and focused expectation | Phase18 | focused case rerun plus full focused sign-off | expected assertion passes; setup completion/stop is correctly owned | closed_proven |
| P17-F003 | focused local LLM root | `focused-nextjs-endpoint-smoke` | Expected ok, observed plan lint failure and raw `rc:1` in recorded root | Endpoint smoke failure should be classified by plan/profile/dev-server contract without raw rc | planning / eval observation | planning contract / eval success contract | `scripts/eval_agent_slice.sh`, plan lint reason extraction, focused case definition | Phase18 | focused case rerun after `plan_lint.invalid_expected_path` extraction | no raw `rc:*`; expected assertion passes or correct owned stop | closed_proven |
| P17-F004 | focused local LLM root | `focused-nextjs-route-integration` | Expected ok, observed manifest repair / plan lint failure | Route integration should be route/profile-owned or manifest-owned only when dependency evidence is the actual cause | profile / planning / recovery task | profile contract / planning contract | Next.js profile mapping, plan lint obligation projection | Phase18 | focused case rerun plus full focused sign-off | expected assertion passes; owner/action matches root cause | closed_proven |
| P17-L001 | large time-boxed root | large non-Next rows | Five large rows timed out as `provider_transport:eval_timeout` under 120s evidence run | Timeout rows should preserve provider/eval boundary evidence and not look like missing migration ownership | provider transport / eval boundary | provider transport contract | `scripts/eval_agent_slice.sh`, `scripts/eval_report.py`, signoff missing-field rules | Phase19 | large time-boxed rerun or synthetic timeout fixture plus signoff checker | timeout rows have explicit owner and not-applicable evidence semantics, or are accepted external blockers | blocked_external |
| P17-L002 | large time-boxed root | `large-nextjs-app-modify` | Profile dependency version conflict mapped to generic source repair in recorded root | Dependency/version conflict should select manifest/setup owner, target `package.json`, and setup/manifest action | profile / active job arbitration | profile contract / active job contract | `scripts/eval_report.py`, runtime profile failure mapping | Phase19 | rerun large Next.js modify or focused profile fixture plus signoff checker | no generic source fallback; manifest/setup owner selected | closed_proven |
| P17-L003 | large time-boxed root | all failed large rows | Missing `evidence_binding_status` and `completion_evidence_status` | Failed large rows should carry evidence binding/completion evidence status or explicit not-applicable reason | completion evidence / eval report | completion evidence contract | `scripts/eval_report.py`, runtime job report projection | Phase19 | large rerun or report fixture plus signoff checker | signoff no longer reports missing evidence binding/completion evidence | closed_proven |
| P17-L004 | large time-boxed root | failed large rows with applicable targets | Missing target path/role where repair target should be known | Failed large rows should include selected/admitted target or explicit target not-applicable reason | target admission / eval report | target admission contract | target admission/report projection | Phase19 | large rerun or target-admission fixture plus signoff checker | signoff no longer reports missing target for target-applicable rows | closed_proven |

## Reconciliation Rule

The Phase 17 ledger is complete when the current `scripts/eval_signoff.py`
findings are either represented by one row above or intentionally split into
new rows with narrower ownership. If sign-off output changes, update this
ledger before starting runtime fixes.

Current reconciliation snapshot:

```text
sign-off findings: 19
ledger rows: 8
unmapped findings: 0
context blockers not emitted by sign-off checker: 1
```

The context blocker is `provider_transport:eval_timeout` in the large
time-boxed root. It remains tracked because timeout evidence explains why the
large root cannot be used as a release-quality migration proof.

Coverage-table reconciliation is still required during Phase17 execution. If a
ledger row cannot be mapped to
`docs/eval/legacy-control-stack-coverage-20260621.md`, the row is a Phase17
coverage gap and must be fixed before Phase18/19 runtime work starts.

## Phase Closure Rule

Phase 17 may close with all rows `open` if and only if every row has an owner,
phase, proof command, closure condition, reconciliation mapping, and coverage
responsibility. Phase18 and Phase19 may not close with any assigned row still
`open`.

## Phase19 Closure Snapshot

Phase19 proof is recorded in
`docs/eval/loadmap2-phase19-large-recovery-20260623.md`.

The large recheck root is:

```text
eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

The broad sign-off command with Phase16 smoke, Phase18 focused, Phase16 focused
fixtures, and the large recheck root returned `status: pass`.
