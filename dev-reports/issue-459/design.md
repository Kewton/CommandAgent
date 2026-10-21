# Issue #459 design

Written before implementation. Read the complete GitHub Issue #459, this
worktree's AGENTS.md, `$codex-issue-worker`, development guardrails, the approved
develop-root investigation/review documents and #451/#452 cause analyses, and
the parent's `20260909-456460-coordination-01/issue459-parent-review-plan.md`.
Verified committed predecessor reports and inspected their changes: #456
`32f997f4`, #457 `f2253cb7`, #458 `008f4f5d`. Imported their linear history by
fast-forward only; implementation parent is `008f4f5d4c1eab6697f015c6d304401e055beb3e`.

## Implementation

Add a separate new-R0 integration corpus, reusing the byte-identical #456
archive and #457 frozen lock/contract overlays by explicit hash references.
Retain original, role-only, UI-only, missing-directory, wrong-update-field and
aligned cases. Counts 33 and 32 describe compiler diagnostics, not independent
defects. The sample member directory is the documented #457 fixture decision;
it does not decide the directory provider for a new live R0.

Use model-protocol reply replay with actual Read and Edit/Write tools. Trace a
create failure into production Recovery capture/start, generated inspection
binding, actual ranged definition reads, repair, registered checks, final
acceptance and production finish/adoption. Reuse no injected successful events,
execution results, browser results or post-run source replacements. Keep the
original Next.js contract's capabilities/evidence; any fixture-only resource
setting or additional oracle contract is explicitly identified. An independent
business oracle supplements original authority, never silently replaces it.

Add an installed-toolchain integration test, explicitly invoked in verification,
with the same TypeScript/Next lock throughout. Test a real no-edit failure,
eligible continuation/second attempt and terminal insufficient-evidence stop.
Verify source identity before repair and after rejection, fresh treatment on
retry, retained diagnostics/definitions in provider requests, ordered production
events and final candidate decisions. A passing build alone must not imply
promotion. Positive reply sequences apply #457 repairs through real tools.
Exercise actual page/API/store listing, selection, assignment and reload in an
isolated browser; unavailable or unexecuted operations cannot pass the oracle.

Keep implementation in test/fixture leaf modules and scripts where possible.
No guardrail baseline, stop threshold, verification/acceptance condition, event
schema or runtime namespace changes are planned. Existing #448/#449/fix fixtures
remain distinct. Source archives, earlier evidence and other worktrees stay
read-only. Save reproducible commands, dependency/source hashes, observed results
and limitations in the new corpus and required development reports.

## Verification and handoff

Run focused corpus/replay checks first, installed-toolchain and real browser
checks explicitly, predecessor regressions, corpus/guardrails/protection audit,
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test`. Run Node syntax checks for new JavaScript harnesses; Python changes
would additionally require Ruff and focused Python tests. Release build/version
is required only if release-sensitive product code changes become necessary.

Replay success establishes deterministic integration, not improved model repair
ability. #452 must run a fresh authorized R0 from an immutable original goal and
oracle, decide/record its member source, establish build/type/functional/final
acceptance prerequisites, and only then run the original ordered comparisons
and B-1. This worker starts no live campaign. Commit locally on the dedicated
branch; push/PR/merge/Issue lifecycle actions remain with the parent.

## Observed adoption gap and bounded refinement

Outside-sandbox replay-03 passed real Browser and final acceptance, then real
finish rejected `preflight_source_mutation_rejected_and_restored`. Its output
policy was empty: the archived store atomically writes UUID temporary files and
renames them through an opaque parameter; it also uses path bindings referenced
inside template expressions. The existing bounded recognizer deliberately cannot
prove these destinations. Retain this exact #457 aligned candidate as a negative
adoption control. All-rejected traces do not establish positive adoption.

The parent's follow-up explicitly permits a semantically equivalent fixture
repair and a minimal, proven product recognition fix. Add recognition only for
the **destination** of a real Node fs rename whose complete second argument is
already a provable immutable literal/path expression under the existing lexical
rules. Do not infer opaque helper parameters, register arbitrary paths, relax
configuration/protected/source/symlink checks, or allow temporary UUID files.
Both source deletion and unregistered additional mutations remain checked by the
existing snapshot policy. Add focused dynamic/shadowed/escaped/protected/config
and source-mutation controls.

For the positive fixture, retain atomic temporary-write/rename and generation
checks. Route the two known final destinations through explicit rename branches
for `data/projects.json` and `data/tasks.json`; preserve the existing fallback.
All calls in this application use those two paths, and its cwd never changes.
This exposes actual final filesystem effects, without dummy writes or metadata
assertions. Keep original #457 bytes and its unrecognized-helper negative case.
Verify the extra candidate's strict types, build, assignment/reload and corrupt
store/no-overwrite semantics under the same toolchain, then apply it through real
Edit and require actual production promotion. The original contract requirements
remain intact; the additional business-oracle contract is explicit and stronger.

This product leaf change additionally requires the full Rust suite, protection
and output-recognition regressions, release build and version check. No growth
baseline changes or runner chokepoint changes are needed.

## Registered verifier execution form

The first complete replay attempt identified weak evidence for direct `node`
invocations: installed tsc is outside source evidence, and the business oracle's
resource cleanup/template/process constructs exceed the conservative inline
assertion recognizer. Keep that classifier unchanged. Register the identical
installed compiler via `./node_modules/.bin/tsc` with the same strict flags, and
execute the actual asynchronous assertion oracle as a named `node:test` test
using `node --test checks/assignment.test.mjs`. Import, setup, assertion and
cleanup errors still exit nonzero. Positive, missing-list, wrong-field and
unexecuted-oracle controls use the same commands; original `npm run build`,
capabilities and evidence remain required. This is an execution-form change,
not a relaxation of verification content or a declaration of business success.
