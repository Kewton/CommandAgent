# Issue #449 design

Base: `3952f99ff3b2afde40b8d4e88380141044815836` (Issue #448). Its committed
fixture, measured build matrix and Recovery rejection tests have been inspected.
The integration AGENTS.md, including Browser reliability, and R0 stop/review
records are read-only inputs. No predecessor branch remains to integrate.

## Investigation and bounded implementation

Trace the saved R0 diagnostics, repair prompts/plans, target selection,
write_required activation, tool calls/results, changed paths and revalidation.
Keep line references and hashes, distinguish saved prompts from complete
provider requests, and record unavailable observations as `unknown`.

Use #448 source and diagnostics to exercise both the missing export and Promise
error through production targeting, prompt construction, read-only exhaustion
and Recovery handoff. Check that the original goal, necessary source targets
and registered `npm run build` survive. Check legitimate Write/Edit permission
and retain no-change/UI-only failure and existing verification/promotion gates.

Candidate inconsistencies to test before changing production: compact compile
prompts omit supplied goal/verification context; Recovery phase prompts may
omit diagnostic targets needed by the initial inspection phase. These are
hypotheses, not established causes of R0. Fix only a reproduced inconsistency
with a failing-before/passing-after focused test, in a leaf module with minimal
wiring. If none is reproduced, report investigation complete and improvement
unconfirmed without speculative product edits.

No budget increase, acceptance weakening, schema migration, generated app edit,
integration-worktree write, live runtime change, model/browser run or external
GitHub action is included. Fixture improvement cannot establish model success
rate; actual effect requires a separate new campaign/B-1.

## Verification

Run focused Issue #449 and #448 tests, existing read-only/root/weak-verification
and promotion checks, corpus and growth/protection audits. For production Rust
changes run formatting, Clippy and full cargo test. Record exact outcomes in
verification.md and commit only task-owned paths.

## Confirmed implementation boundary

Read the complete GitHub Issue body and parent preparation/subrun contract.
Two focused tests failed before production edits: the fresh compact session
lost the supplied original goal; the saved read-only handoff lost the missing
export diagnostic. Legitimate target Write/Edit and root/no-op checks already
passed. The Promise case also exercises both paths in the passing matrix.

Retain supplied goal, relevant paths, registered verification, remaining profile
failures and previous changed paths in compact/regeneration prompts using one
new leaf renderer. Preserve the exhausted session objective as quoted failure
context in the read-only handoff, leaving the authoritative contract goal and
verify commands separate. No targeting priority or tool policy change is needed
for these reproduced failures. Do not change the initial inspect phase based
on its wording alone; the actual R0 repair objective expressly requested an edit.

The original phase handoff's missing API diagnostics are documented separately;
these changes do not claim to reconstruct those lost observations or solve that
earlier transition. The tests supply scanner-derived diagnostic paths explicitly
and verify their retention, rather than claiming a new target-extraction fix.

Parent reports #448's original-contract promotion coverage is receiving a
follow-up. #449 stays pinned to its supplied #448 commit and does not edit
auto_recovery.rs or issue448_tests.rs. Development verification here does not
declare dependency, merge, original business acceptance or campaign readiness.
