# New-R0 create Recovery integration (#459)

This corpus starts from the archived generated create artifact at HEAD
`222feef77a4bfd1e98a60a4bd5744a287e645b48`, campaign
`20260909-issue452-resume-0618-standard10`, session
`01a084df-17e3-7d31-87cc-18888b045a05`. Original files are reused from
`../issue456-create-recovery/original`; diagnostics, the frozen lock and contract
repairs are reused from `../issue457-nextjs-contracts`. `provenance.json` binds
the inputs by SHA-256. No archived source, event or previous fixture is replaced.

`replies.json` contains authored deterministic provider replies. The historical
failure has no successful model response to replay. `tests/issue459_recovery.rs`
calls the public automatic-Recovery entry point on the normally compiled library,
so Browser/test-only result overrides are absent. Real create verification fails,
the product captures its typed child, binds inspection, executes actual ranged
Reads and Edits, runs registered checks, performs final acceptance, and executes
production finish/adoption. No execution outcome, successful event or Browser
result is injected; repaired bytes enter treatments exclusively through Edit.

The original create artifact is pre-materialized, rather than regenerating its
earlier 487-event live conversation. The actual create failure and subsequent
Recovery are newly executed. Toolchain links and identical `experimental.cpus: 2`
settings are fixture infrastructure. The original goal/port, required paths,
capabilities, evidence and obligations stay intact.

## Source and functional matrix

| Case | Type diagnostics | Build | Assignment oracle |
| --- | ---: | --- | --- |
| original | 33 | fails | not executed |
| role-only | 32 | fails | not executed |
| UI-only local member fallback | 33 | fails | not executed |
| missing-list | 0 | passes | no selectable member |
| wrong-field | 0 | passes | PATCH 400; prior assignment survives reload |
| aligned (#457 exact repair) | 0 | passes | passes |
| aligned-observable | 0 | passes | passes |

Counts describe diagnostics, not independent defects. Both aligned cases check
listing, selection, assignment, reload, reassignment/unassignment, status filters,
deletion and corrupt-store errors/no overwrite. The member source is #457's
documented two-member sample directory, a fixture decision. It does not decide
the member provider for the future live campaign.

`observable` preserves temporary UUID writes, atomic rename and generation checks.
Its real rename branches spell out the two stable destination paths; the fallback
is retained. The application never changes cwd. Thus absolute original paths and
these root-relative destinations refer to the same files. The product's bounded
recognizer admits only a provable rename destination, with existing path and
snapshot restrictions. The unmodified opaque parameter form stays unrecognized.

## Recovery decisions

`replay-cases.json` is the executable expectation table. All scenarios start from
the original failed artifact, never from a manually repaired control.

| Scenario | Starts | Expected decision |
| --- | ---: | --- |
| aligned-after-no-edit | 2 | first no-edit rejected; exact #457 repair passes final acceptance but finish rejects unrecognized JSON output mutation |
| aligned-with-oracle | 1 | functional oracle passes; same output-policy rejection |
| missing-list | 1 | registered functional check fails; control retained |
| wrong-field | 1 | registered functional check fails; control retained |
| oracle-unexecuted | 1 | actual missing oracle module; no functional pass/evidence; control retained |
| no-edit | 2 | typed read-only failure reaches the exact run limit; no edits or promotion |
| provider-stop | 1 | provider error stops; no retry or promotion |
| promoted-after-no-edit | 2 | first no-edit rejected; observable repair passes original contract and is promoted |
| promoted-with-oracle | 1 | observable repair passes original requirements plus registered types/business checks and is promoted |

The original-contract cases preserve the original contract value. The five cases
using the assignment oracle explicitly **add** strict TypeScript and
`./node_modules/.bin/tsc --noEmit --incremental false --pretty false` and
`node --test checks/assignment.test.mjs` commands and protect/require the oracle scripts.
No original requirement is removed. The oracle runs the actual just-built candidate
in a disposable copy; its result hashes must match the actual edited source.
Its authority belongs only to this documented stronger fixture contract.

Assertions inspect actual provider messages separately for every inspection
attempt, Read results, absence of inspection mutations/builds, real Edit output
hashes, registered checks, exact start counts and finish reasons, continuation
ordering, and source/data identity in control and fresh treatments. A positive
result requires actual `decision=promoted` and `recovery_succeeded`, plus byte
identity of promoted control and the checked candidate. Build success alone is
insufficient; the opaque aligned case remains a regression for that distinction.

## Reproduce

From the repository root, with an installed Playwright module and Chromium:

```sh
node scripts/issue459_nextjs_matrix.mjs --work-root /tmp/NEW_TYPES --output /tmp/NEW_TYPES.json --playwright-module /absolute/node_modules/playwright
node scripts/issue459_replay_matrix.mjs --work-root /tmp/NEW_REPLAY --node-modules /tmp/NEW_TYPES/dependencies/node_modules --playwright-module /absolute/node_modules/playwright
cargo test --test issue459_recovery
cargo test --lib issue459_rename
```

The type/build matrix installs the archived lock once. Node 24.1.0, TypeScript
5.9.3, Next.js 14.2.35 and Playwright 1.58.2/Chromium 145.0.7632.6 were used for
the committed verification. Run HTTP/Browser tests outside a sandbox that denies
listening sockets. Never reinterpret ENOTFOUND or listen EPERM as an app defect.
Both scripts refuse existing work/output paths and keep prior evidence intact.
The expensive integration test is ignored by default and must be explicitly run;
the replay-matrix script does this for every registered scenario, sequentially on
the original port 60302. The default test checks the immutable input manifest.

One scenario can be run with `ISSUE459_SCENARIO`, `ISSUE459_WORK_ROOT`,
`ISSUE459_NODE_MODULES` and `ISSUE459_PLAYWRIGHT_MODULE` set, followed by
`cargo test --test issue459_recovery issue459_installed -- --ignored --nocapture`.
Per-scenario result files retain provider requests, bound contexts, candidate
hashes, actual oracle results and decision evidence. The matrix summary retains
the actual success-completion event. Development reports retain the checked
results, selected screenshots and a compact inspection trace; raw logs stay in
the disposable run directories.

## Handoff to #452

These results establish deterministic integration, not model efficacy. A new
authorized live R0 must retain the original goal/oracle/order, record its selected
member source, use a verified binary/toolchain, and execute build/type, assignment
interaction/reload, final acceptance and candidate adoption. If original opaque
storage remains, the documented honest output-policy rejection can still occur;
this narrow recognizer does not solve arbitrary atomic helpers. Check the new
R0's actual source and registered evidence before declaring that prerequisite met.
Only then begin the original nine comparisons and B-1. Keep #451 Browser backend
availability separate from application failures. No live campaign, 4-axis success
claim, provider communication or historical evidence rewrite is part of #459.
