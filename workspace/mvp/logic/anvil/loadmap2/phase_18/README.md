# Phase 18 Plan: Focused Sign-off Recovery

## Objective

Phase 18 closes the focused sign-off blockers assigned by Phase 17.

Phase 18 is complete only when the focused control-recovery sign-off has:

- zero failed expected assertions;
- no raw `rc:*` diagnostics;
- no unowned focused failure;
- all Phase18 ledger rows marked `closed_proven`.

This phase does not address large local LLM evidence gaps. Those remain Phase
19 work.

## Source Of Truth

Phase18 work is derived from:

- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`

Assigned ledger rows:

| Ledger row | Sign-off findings | Case |
| --- | --- | --- |
| P17-F001 | S001 | `focused-docs-literal-mismatch` |
| P17-F002 | S002 | `focused-nextjs-dependency-setup` |
| P17-F003 | S003, S005 | `focused-nextjs-endpoint-smoke` |
| P17-F004 | S004 | `focused-nextjs-route-integration` |

## Scope

Phase18 owns focused rows only:

- docs literal mismatch ownership and expected assertion alignment;
- Next.js dependency setup completion / setup-owned stop;
- Next.js endpoint smoke plan-lint/raw diagnostic recovery;
- Next.js route integration owner/action alignment.

Out of scope:

- large local LLM timeout behavior;
- large missing evidence fields;
- large generic source fallback;
- final migration-complete declaration;
- broad sign-off pass. Phase18 may improve focused sign-off only.

## Recovery Strategy

Phase18 uses the Phase17 accounting chain:

```text
sign-off finding
  -> ledger row
  -> coverage responsibility
  -> focused case
  -> targeted proof command
  -> full focused sign-off
```

Each focused blocker must be handled in this order:

1. Reproduce the blocker from the recorded Phase16 root or a targeted rerun.
2. Decide whether the mismatch is:
   - stale expected assertion;
   - report/projection bug;
   - runtime contract bug;
   - plan/profile/recovery behavior bug.
3. Apply the smallest responsible-layer fix.
4. Rerun the targeted focused case or case family.
5. Rerun the full focused control-recovery matrix.
6. Rerun the broad sign-off checker and confirm focused findings are gone.

## Horizontal Rollout

Phase18 focused fixes must not be one-off patches for a single YAML row.

| Area | Horizontal rule |
| --- | --- |
| Docs/data focused rows | Literal/schema mismatch failures must use common eval assertion and recovery ownership fields. |
| Next.js focused rows | Dependency setup, endpoint smoke, route integration, manifest repair, and port conflict must share the same setup/profile/recovery vocabulary. |
| Python/Rust focused rows | Phase18 must not regress existing passing Python/Rust focused rows. |
| Fixture rows | Deterministic fixture rows remain proof support and must not be counted as runtime success. |

If a fix requires profile-specific logic, it must be behind a common contract
field such as active job, recovery owner, target role, evidence binding, or
completion evidence.

## Documentation Updates

Update documentation only when behavior or interpretation changes:

- `docs/evaluation.md`
  - if focused sign-off or raw diagnostic rules change.
- `docs/known-limitations.md`
  - if a focused blocker remains as an accepted limitation.
- `docs/eval/`
  - add a Phase18 focused recovery report after implementation/eval.
- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`
  - update statuses from `open` to `closed_proven` only after proof commands
    pass.

Do not update architecture/ADR docs unless Phase18 proves that the current
Planning / Execution / Recovery Task / Profile contracts cannot represent the
required focused behavior.

## Design Alignment

- The minimal loop remains the only execution loop.
- Phase18 must not add hidden retries or rerun-until-green behavior.
- Each fix must belong to one layer: eval/reporting, planning, setup/profile,
  recovery task, step policy, or runtime contract projection.
- A failed proof command keeps the ledger row open.
- CI pass is required but never sufficient.

## Stability And Complexity Controls

- Prefer targeted focused reruns before the full matrix.
- Do not change large eval behavior in Phase18.
- Do not weaken expected assertions just to pass.
- Do not broaden source repair fallback when a setup/profile/eval owner is more
  accurate.
- Split a blocker if one row hides multiple root causes.
- If the same ledger row fails twice after targeted fixes, attach a design
  review note before another implementation attempt.

## Architecture And Extensibility

Phase18 should improve common focused recovery boundaries:

```text
focused failure
  -> normalized observation
  -> active job / owner / action
  -> target/evidence fields
  -> focused assertion
```

The intended architecture is not a Next.js-specific workflow. It is a reusable
focused sign-off path that can later absorb new profile rows by adding cases
and coverage responsibilities, not new control loops.

## Acceptance Criteria

- P17-F001, P17-F002, P17-F003, and P17-F004 have targeted proof commands.
- Targeted proof commands pass or the row is split with rationale.
- Full focused control-recovery local LLM summary has no failed expected
  assertions.
- Focused sign-off output has no `focused_assertion_failed` or
  `raw_undiagnostic_rc`.
- Phase17 blocking ledger statuses for Phase18 rows are updated to
  `closed_proven`.
- A Phase18 eval report records commands, roots, findings before/after, and
  remaining blockers.

## Review Result Reflected

Review concern:

```text
Phase18 could become another broad "fix focused eval" bucket and repeat the
Phase1-16 process problem.
```

Reflected changes:

- Phase18 scope is limited to P17-F001 through P17-F004 and S001 through S005.
- Each row has an owner, proof command, and closure condition.
- Large/broad evidence work is explicitly deferred to Phase19.
- Expected assertions cannot be weakened without first classifying the row as a
  stale expectation and recording that in the Phase18 report.
