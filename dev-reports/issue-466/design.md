# Issue #466 design

## Base and evidence

Initial `git fetch --no-tags origin develop` succeeded after the filesystem
sandbox retry. Initial HEAD and fetched origin/develop were both
`e99f1ebe1e777fb792a9343407736ca754e41bdf`. Inspected #465's committed changes
and passed verification, then fast-forwarded this branch to
`b60ae3e63a0eb23cdf40cf52057b995bde08f8ca`. Both ancestor checks passed; the
worktree was clean. No conflict or edit to the original develop worktree occurred.

Read the complete Issue with `gh issue view 466`, repository guardrails, worker
skill, the parent's issue466-base-observation.md, and the read-only analysis
report and discussion-resolution.md. Latest base has no verifier_obligations
leaves. The historical `9549f746` belongs to the independent repository at
`CommandAgent-develop/workspace/tmp/0911/issue452-candidate-source-obligation-02`,
not the current object database. Reading the inspection leaf with `git show`
there succeeds; its SHA-256 is
`06d36b720ad0a4621aa736e45e158918dd0b798f18bb3017a2d86da69fe6d782`, matching
the original investigation. The initial lookup from this worktree was not
evidence that the historical commit lacked the file. Historical E
partial-existence and S1/T2 single-owner findings
are attributed to the saved analysis, not reproduced current code behavior.
Historical complete proposals and model/host contribution remain unknown.

## Intended change

Add leaf modules for host-owned verifier formation, immutable source obligations,
binding classification and bounded planning feedback. Keep driver wiring small.
Use existing in-process generated-contract provenance for registration, then
carry obligations through the host inspection context and continuation.

Before a newly generated plan closes, identify direct registered script checks
and require dedicated Implement ownership. Mixed application/configuration and
verifier scopes receive a concrete correction request, including model and
post-host scopes. Do not guess how to split arbitrary natural-language duties.
Accept a corrected partition only while retaining the original outputs,
instructions, expected results and checks across its owners. Formation shares
the existing three-attempt planner loop, including lint/schema retries.
Register only admitted plans, never a rejected proposal or configured contract.

During Recovery, preserve every closed producer. Fully present producers are
not new creation authority. Missing producers can bind to one compatible
Implement or be restored from their registered source when no owner exists.
Reject partial existence in a closed group without shrinking its expected_paths.
Reject protected/configured verifier ownership, redirected/aliased paths and
unsafe ambiguity. A single wrong kind, incomplete owner or overlapping multiple
owners is proposal-repairable; original inconsistency and unsafe failures stop.
Use clone/validate/commit so a failed bind never half-mutates a plan. Preserve
original producer checks and require the full final host contract.

Record additive events with original scope/provenance, candidate owners,
pre/post host scope, classification and existing planner attempt/remaining cap.
No new Recovery attempts, relaxed gates or event-schema changes.

## Verification plan

Add a source-only corpus with the formation/existence/owner/protection matrix,
historical closed E analogues explicitly labeled as such, and missing-proposal
unknowns. Focused tests must exercise real Runner generation feedback followed
by execution, plus refused plans that never execute; include host augmentation,
closed partial existence, exhaustion, aliases/duplicates and transactional bind.
Then run fmt, all-target clippy, corpus/guardrails and full cargo test, release
build, version and SHA-256. Record all results before the Issue-scoped commit.
No push, PR, merge to develop, Issue mutation, services or live API campaign.

## Design refinements and predecessor correction

The parent notified a test-only Linux correction for #465. Inspected its exact
diff and passed verification, then fast-forwarded to
`6c49222db4f282b75070d923479e6d41209b132a` without conflicts or changes to the
ongoing #466 work. The replacement proves Write denial with structured events;
no #465 production behavior or test gate was removed. The parent's latest
GitHub CI was still running when that notification arrived; this report does
not assert its remote acceptance result.

Formation and registration now share the existing command normalization and
admission function, including compound expressions; only Implement/Verify pass
commands contribute authority. Each output's actual owner must retain the
original instruction and expected result. A split group's complete original
checks execute in a Verify after all corresponding owners. Verify expected_paths
are references removed by the existing normalizer; the output owners and generated
run contract retain the full required-path union. No inspection/check budget or
natural-language meaning is reduced to accept a partition.

Every return after the generation loop starts validates retained formation scope,
including valid-plan degradation and setup fallback. Invalid deterministic
templates stop with the concrete scope diagnosis. Model and post-host Recovery
ownership are both checked, so generic duplicate-owner normalization cannot hide
a rejected raw proposal. Execution rechecks the admitted binding and final host
checks before running any step.

Historical source context producer arrays were read from the saved E1/E2/E3
treatments and copied exactly into a new corpus fixture, with SHA-256 for each
complete original context. Other runtime/diagnostic fields are excluded. The
tests replay these closed producer arrays, reject partial existence, and reject
attempts to turn a reduced unsealed legacy scope into new host authority.
Formation success is measured separately from these historical refusals.
