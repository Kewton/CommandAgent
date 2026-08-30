# A14-A13-1 Recovery smoke analysis

## Outcome

A14-A13-1 completed all 10 frozen pairs, but the same report script returned
instrument NO-GO: 27 of 30 checks passed. The run remains immutable diagnostic
evidence and is not eligible for a Recovery population effect claim.

- Repository run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a13-1-smoke-01`
- Product execution root: `/Volumes/SSD_NX/tmp/commandagent_trial/phase6-recovery-v4-20260830-a14-a13-1-smoke-01`
- Exact product/code SHA: `6e35764af212fc66c634de2d0c7b178f13acafa7`
- Report: `recovery-report-v4.json`

| Outcome | Count |
| --- | ---: |
| Attributed improved | 1 |
| Attributed harmed | 0 |
| No Recovery needed | 5 |
| No Recovery executed (dependency sentinel) | 1 |
| Attributed unchanged fail | 3 |
| Unusable | 0 |

The three false checks were `fix_contract_continuity`,
`registered_inner_recovery_verify_commands`, and
`recovery_fix_terminal_completion`. All violations were the three cell-08 nextjs
pairs. Frozen external oracle semantics and effect attribution were valid for those
pairs, and each produced a legitimate fail-to-fail observation with zero harm and
zero regression.

## Profile-contract conflict

Cell-08 is not a recoverable nextjs task. Its completion and operational contracts
require the existing Python project to remain Python and explicitly forbid
conversion to a Next.js project, while the selected product profile is `nextjs`.
The initial run nevertheless requested `package.json` and `src/app/*`, attempted
blocked npm setup, and failed. Recovery inherited the nextjs profile, entered only
its read-only inspection phase, and then stopped on missing offline Next.js
dependencies. It never reached the host-bound final-success phase, so the three
success-path continuity gates correctly lacked completion evidence.

The direct failure is unavailable Next.js scaffolding, but the root cause is the
explicit product-profile/task-contract contradiction. Increasing Recovery count or
weakening the three gates would not resolve that contradiction.

## A14-A13-2 amendment

A14-A13-2 classifies a task as `profile_or_completion_contract` before execution
when its selected product profile is explicitly named by
`operational_constraints.do_not_convert_to`. Such a task is retained as a sentinel
but is configured for and must execute zero Recoveries.

The existing dependency sentinel already behaved correctly: preregistered
`dependency_or_provisioning`, configured 0, executed 0, and no harm or regression.
Cell-08 will now be tested under the same no-Recovery principle. The A14-A14 full
experiment must remove cell-08 from the eligible effect population before it can be
frozen.
