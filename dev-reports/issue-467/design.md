# Issue #467 design

## Baseline and authority

Read the complete Issue using `gh issue view 467 --repo Kewton/CommandAgent
--json number,title,body,state,url`, the worker skill, and development guardrails.
The required `git fetch --no-tags origin develop` succeeded outside the sandbox.
Initial HEAD and fetched origin/develop: `e99f1ebe1e777fb792a9343407736ca754e41bdf`.
Inspected committed #465/#466 changes and their passed verification reports.
Fast-forwarded the dedicated branch to `612ec3574716663161c707f17e2be17ea7a03c8c`,
which contains #465 `6c49222db4f282b75070d923479e6d41209b132a` and latest develop;
all three ancestor checks succeeded. No conflicts or initial local edits.

Read-only evidence: the original develop worktree's
`workspace/tmp/0912/recovery-analysis-01/report.md`, `discussion-resolution.md`,
and their referenced audit, plus T3 session
`01a091c9-77b8-72e2-83cb-83023974ab39`. Historical product revision
`9549f746692d4dd263bdc5088572655eb867cc06` is a reproduction reference only.
Historical first write remains unknown. Historical interaction_success=false
and persistence_not_evaluated:no_mutation_observed remain failures/unobserved.

## Smallest change

1. Replay the unchanged T3 writer and a structural minimum directly through
   the current recognizer before modifying it, including a pid-literal control.
   Existing rename destination support already exists. Test the specific
   hypothesis that an opaque `${process.pid}` taints the process binding used
   by process.cwd(). If confirmed, recognize only this exact side-effect-free
   interpolation in a lexer leaf; keep its output opaque and retain all
   binding, filesystem, source/config/private/protected and cwd exclusions.
2. Move control checking into a preflight leaf that wraps every result after
   checkpoint capture. Always audit control, including preparation failures,
   isolated mutation rejection and observer errors. Preserve existing reason
   names for compatibility; add an explicit audit event with actual restore
   invocation/result and before/after hashes. Verify restoration against the
   original checkpoint; observation/restore failures still stop Recovery.
3. Record isolated effects at meaningful observer/verifier/acceptance boundaries
   using the existing snapshot inventory and exact path/hash changes. These
   observations add evidence and never replace the source-mutation gate.
4. Add positive/negative executable corpus fixtures and audit-path tests. Re-run
   the real T3 writer/app in fresh temporary isolation to distinguish build,
   startup, GET and business-operation stages; retain unknown for historical
   timing and for anything the new run cannot observe. Original artifacts,
   control, evidence, live .anvil and shared dependency caches remain unchanged.

## Verification plan

Run focused recognizer and preflight/audit tests, the isolated T3 stage replay,
then recovery regression, corpus/protection/guardrails, fmt, all-target clippy,
full cargo test, release build, version and SHA-256. Save implementation and
verification reports, mark passed only after every required check passes, and
commit explicit Issue-owned paths. No push, PR, external merge, Issue mutation,
shared service operation, or other-worker message is authorized.

## Parent steering before implementation

The parent supplied independent stage measurements in
`workspace/management/runs/20260912-recovery-next-inputs/issue467-independent-observations.md`
and `20260912-recovery-t3-stage-replay-01/` in the original develop worktree.
Read their report and repository-identity note. Reuse this new, already measured
build/start/GET/POST replay instead of repeating identical app commands, as
requested. Focus new execution on the current product recognizer and audit paths.
The parent promises not to use 60302 during worker product testing; no existing
listener may be stopped. Keep historical timing unknown and distinguish the
parent's new replay from the worker's current-product tests.

The parent reviewed the initial restoration code and identified that checking
only the return digest would allow an altered snapshot to overwrite control
before rejection. Added a pre-copy namespace/original-digest validation leaf,
kept the post-copy original-digest check, and separated validation refusal from
actual copy failure in the executable matrix. Invalid-source controls require
unchanged post-observer control bytes and `restore_invoked=false`.

A second parent review requested explicit identity on the added events. Bind
both audit/effects to the checkpoint workspace relative path as observation_id;
verify distinct 0/64/128 observations even when the original control hash is
identical. This is evidence correlation only and does not change Recovery counts.
Final broader verification must run serially against this final implementation.
