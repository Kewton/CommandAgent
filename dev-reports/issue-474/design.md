# Issue #474 design

## Scope and baseline

Read the full GitHub Issue #474 (including the approved dispatch decision),
repository guardrails, and the prior remaining-work investigation/discussion.
Fetched origin/develop on 2026-09-13 JST. This clean isolated worker branch starts
at `e43b2d86760a77b5250afbb260106ea21abba947`, equal to origin/develop.
No predecessor was assigned. Existing #439 implementation and fixtures are the
regression baseline; #465 outer Recovery is distinct from final acceptance repair.

## Findings before implementation

The saved `node -e` command executes three independent negative `includes`
predicates against the same `src/app/page.tsx`, throwing on missing primary,
input, or state hooks. Its failure behavior is real. The generated-hook
recognizer only accepts exact single-predicate normalized grep forms. The
conservative general Node recognizer does not accept throw as Test evidence,
so the compound command becomes `node_smoke_without_assertion`.

Final contract verification executes its registered commands separately from
static runtime evidence classification. The existing registry/recovery authority
retains path-specific hook checks. Final repair target selection consumes missing
evidence and general path buckets; investigate the weak command association and
actual recorded repair selections before declaring an independent target bug.

## Proposed smallest change

Add a leaf recognizer for the closed compound shape: literal fs import/read,
one safe relative source path, all three ordered hook predicates with uncaught
throws, and the terminal success print. Accept only a single literal Node eval
command and match its complete script. Preserve literal content, path, predicates,
and propagation. Classify this shape as StaticSyntax through the existing hook
authority; never as business Test. Keep the original executable command in the
registry and run it again at final acceptance.

Compared with splitting/normalizing into three commands, this avoids changing
command identity, registry provenance, and stored contracts. Generation-only
constraints would not repair the saved command path. Do not broaden general
throw/assertion recognition, shell syntax, or success gates.

## Verification and investigation

Create a minimal corpus from the saved command and a synthetic page, without
model regeneration. First run a regression against the unchanged classifier.
Then test actual command execution for all three missing predicates, missing
file, alternate path, swallowed exceptions, shell masking, unknown/display-only
commands, unrelated assertion artifacts, registry retention, stale passes, and
deletion attempts. Confirm no business evidence is supplied by structural checks
and preserve genuine Test registration and #439 regressions.

Exercise the final acceptance boundary and compare unchanged weak evidence after
unrelated API edits with a relevant correction followed by fresh verification.
Consult Codex2 via cmate-delegate and save its reply and disposition here. Split
target selection only if independently reproduced; record reproduction and owner
handoff without creating an Issue or changing lifecycle state.

Run focused Rust/corpus tests, fmt, clippy, full cargo test, and a local release
build/version/hash check. Parent owns targeted UAT, PR/CI orchestration, merge,
and the integrated release check. Worker results are local acceptance evidence,
not live model capability evaluation. Original evidence, generated app paths,
live runtime state, and historical run records remain read-only.

## Investigation addendum: independent diagnostic contract

After the classifier fix, actual missing hooks still select the correct page.
Stronger execution checks exposed a separate, reproducible diagnostic defect:
the final command failure summary loses the actual Node `Error: missing ...`
behind the long echoed eval source, and the generic attribute detector selects
state merely because that literal appears anywhere in the compound command.
Changing the shared command-output summarizer and attribute attribution is a
different contract from structural classification and file-target selection.
Keep this patch bounded; record a model-free reproduction and an explicit parent
handoff for that follow-up. This does not relax the failure/acceptance gates or
claim the inaccurate predicate guidance fixed.
