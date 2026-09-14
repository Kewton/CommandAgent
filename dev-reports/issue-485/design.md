# Issue #485 design

Implement the reviewed regression on `0a8222643683c764fa5f9ac8bd3e5b2b47ef7728`,
which equals freshly fetched `origin/develop` and includes #484 commit
`7125ce939122b3226e10cbf364ba277876578791` (PR #486). The worktree was clean.
Read the revised Issue, preparation resolution, saved R0 source/engine audit,
execution summary and review resolution, the worker skill and development
guardrails before implementation. #484 forms the saved package-script check;
this regression starts at the failed-core-plan Recovery boundary and does not
require that corrected proposals continue to fail admission.

Add an isolated test module beside the existing #448/#428 Recovery tests and a
corpus under `tests/corpus/apps/issue485-scaffold-completion/`. Retain the exact
saved page and completion contract, reduced plan/handoff and observation inputs,
plus source session, file/event hashes and original line numbers. Saved events
are expected-output provenance, never a replay API. No historical evidence is
modified and tests must run without its absolute paths.

Exercise the production scaffold classifier, runtime acceptance, real
`RunnerRecoveryDriver` preflight and bounded `drive` suppression, control audit
and process terminal projection. Use deterministic build/HTTP/interaction input
responses through the existing test transports; no live model or GUI capability
evaluation. Keep the original Next.js contract requirements. The failed-core
outcome is a frozen boundary input, not a rerun of historical model planning.

Separate controls: saved scaffold with successful registered observations;
route-bound project/task implementation with all contract evidence; that same
implementation with independent missing/failed observations. Record completion
evaluation arrival independently of runtime and final acceptance. If useful,
include an unrelated API as a separately classified artifact while preserving
the incomplete goal's remaining gates. Do not add a general scaffold detector.
Expect Recovery 0/2 and unchanged control hashes with restore never invoked for
the saved inconsistency. CurrentSuccess is protection, not internal completion
or external R0 success. Exercise ordinary final acceptance independently for
the controls, requiring all its existing gates before expecting pass.

Prefer test-only wiring and existing functions; no gate, schema, budget,
baseline, runner/loop behavior, live `.anvil/`, root WIP or skill changes.
If production mispromotion is reproduced, report it and a minimal fix before
changing behavior or expectations.

Verification: focused #485, #448, #428, #474, #475, #479, #484; related corpus and
guard checks; `cargo fmt --all -- --check`, all-target Clippy with warnings
denied, and full `cargo test`. Record actual commands/results and a stable tested
code tree. PR, exact-HEAD CI/UAT, release and live R0 remain parent work and are
explicitly unperformed here. Commit only explicit Issue-owned paths.

## Implementation review resolutions

Applied the parent's design review (no blocking issue) and partial-layer review.
The existing test-only browser command override is carried into disposable
observers so the actual HTTP socket can use an ephemeral port while the original
60302 contract/observer remains unchanged. Test transport failure is distinct
from acceptance failure. The real TUI stop emitter precedes process stop as it
did in R0; historical terminal expectations are retained.

The matrix records VerificationReport, runtime acceptance, final/release fields
and task projection separately. Missing probe input can leave verification pass
with partial qualification; a failed mandatory build still fails verification
and the task. These observed layer distinctions do not authorize gate changes,
general partial prohibition or R0 qualification changes. Selected actual events
are emitted as `ISSUE485_RECORD` JSON for the parent's later exact-HEAD UAT.
