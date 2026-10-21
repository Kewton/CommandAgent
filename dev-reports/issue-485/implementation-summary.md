# Issue #485 implementation

The saved failed-core scaffold now has a regression through the production
classifier, runtime acceptance, isolated Recovery preflight, bounded driver,
control audit and terminal emitters. A successful registered build/generic
interaction cannot turn this scaffold into implementation evidence. The test
observes `missing_required_evidence:implementation_artifact`,
`verification_inconsistency`, Recovery suppression at 0/2, unchanged control
hashes, no restore, and failed TUI/process termination.

## Change and scope

- `auto_recovery/issue485_tests.rs` adds three tests covering source provenance,
  the saved failed-core boundary, and four independent completion controls.
- `tests/corpus/apps/issue485-scaffold-completion/` preserves the exact saved
  page, eight required files plus Next.js type references, original UltraPlan
  and completion contract. It includes a reduced failed-core boundary,
  interaction observation, expected event projections, and a small synthetic
  business implementation. The manifest records original session/file/event
  paths, hashes, one-based event positions and reduction mappings. Tests never
  read those historical absolute paths or use saved events as replay input.
- Existing source files receive only test wiring: the test module declaration;
  a `cfg(test)` final-acceptance leaf wrapper; crate visibility on the existing
  test-only TUI emitter; and a `cfg(test)` copy of the existing browser probe
  command override into disposable Recovery observations. Production behavior,
  budgets, gates, schemas and guardrail baselines are unchanged.

## Real decisions and scripted inputs

The original failed core plan is a saved boundary input. The actual failed
`AttemptOutcome` goes to `drive` with `RunnerRecoveryDriver`, not a custom
preflight implementation. Classifier, verification dispatch, acceptance,
snapshot/control audit, suppression and terminal code all execute normally.
The first two historical event projections explain the input boundary; subsequent
projections match newly emitted events in order. Timestamps/session coincidence
are not assertions. The original TUI-before-process stop path is exercised.

Build exits, HTTP responses and interaction inputs use deterministic test
transports. They do not measure a compiler, browser, model or business capability.
Every build response first checks the case's exact source bytes. The original
contract bytes/hash, package scripts and registered observer retain port 60302;
the existing browser command override assigns only the test HTTP child an
ephemeral port, with `require_build=true`. This resolves test competition without
touching any shared listener, GUI, CommandMate or Ollama process.

## Independently observed controls

| Case | Implementation artifact | Preflight | Verification report | Final gate / task |
| --- | --- | --- | --- | --- |
| Saved scaffold | Missing | VerificationInconsistency | Registered observations pass | Final not checked; failed |
| Business, all observations | Present, all required evidence strong | CurrentSuccess | Pass | full_success / complete |
| Business, missing interaction | Present | Unavailable | Pass | partial / partial |
| Business, failed build | Present | Build observation unavailable | Fail; external contract false | Task failed |
| Scaffold plus unrelated API, missing interaction | API present; page remains scaffold | Unavailable | Fail; implementation obligation still missing | incomplete / failed |

The final-acceptance entry point emits multiple layers. Missing interaction
retains `release_gate_status=partial`, `assurance_level=partial` and
`task_status=partial` even when its lower VerificationReport passes and command
status is completed. For a failed build, the release-derived
`final_acceptance_status` can be `full_success` because that release gate is not
applicable; the actual verification fails, `external_contract_ok=false`, and
the real terminal projection keeps the task failed. Neither field alone is the
overall acceptance decision. No new rule forbidding all partial outcomes was
introduced, and no production mispromotion was established by these controls.

`acceptance-observations.json` contains assertion-backed selected output from
`cargo test --lib issue485 -- --nocapture`. Tests emit `ISSUE485_RECORD` JSON
for parent exact-HEAD UAT comparison. Temporary workspace paths are scrubbed
in the committed artifact. Each control independently measures control hashes
and restoration flags; missing/build failures remain separate from scaffold
classification.

## Review and handoff

Applied the parent design feedback and partial-layer resolution from
`20260914-orchestrate-452-484-485-control-01/{design-review-485-01/worker-feedback.md,partial-review-485-01/worker-resolution.md}`.
The implementation remains on #484-integrated parent
`0a8222643683c764fa5f9ac8bd3e5b2b47ef7728`; #484 admission is not forced to fail
again. Required local checks and their results are in `verification.md`.

No live R0/model/GUI capability evaluation, PR, push, merge, Issue mutation,
shared service operation, other-worker dispatch or release was performed.
Historical evidence, live `.anvil/`, root WIP, other worktrees, guardrail documents
and plugin cache are untouched. Parent owns final review, PR/CI, exact-HEAD UAT,
integration/release and #452's external R0 qualification.
