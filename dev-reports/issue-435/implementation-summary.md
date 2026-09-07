# Issue #435 implementation

Generated Next.js/generic completion contracts now exclude file-existence and
file-printing inspections from final-success command registration. Build and
test commands remain registered. The admitted StepPlan and empty-contract
Recovery handoff use the same evidence classification boundary, with the
existing exact Next.js configuration assertion exclusions retained.

Compound commands are normalized and all segments validated before filtering;
`test -f package.json && npm test` retains `npm test`, and invalid substantive
segments still reject registration. An inspection-only Next.js failure handoff
uses the existing product build fallback. An unconfigured generic run without
substantive authority remains empty and cannot invent final-success evidence.

Repair classification recognizes evidence diagnostics and repair guidance
before applying package/framework/entrypoint substring heuristics. Scaffold-only
obligations and unobserved mutation/state updates target implementation. Actual
package/framework failures and explicit missing-entrypoint obligations preserve
their existing classifications, including when mixed with evidence failures.

## Regression coverage

- R0's eight required source files and original completion contract are recorded
  under `tests/corpus/apps/issue435-artifact-only-registration`, with source
  SHA-256 provenance. Recorded browser verdict inputs exclude local tool paths
  and process logs. Four phase completions and R0/S3/E3 acceptance diagnostics
  are projected into a dedicated fixture with original event line numbers.
- The R0 test reproduces the old artifact-only rejection, runs production
  contract initialization/StepPlan registration/refresh, retains the original
  paths, capabilities, evidence and obligations, and verifies acceptance passes.
  Reintroducing `test -f`, `test -e` or `cat` still fails; replacing the page with
  the engine scaffold also fails despite recorded browser success.
- Next.js/generic tests cover inspection-only and mixed checks, compound checks,
  validation failure without registry mutation, and empty-contract handoff
  fallback. Historical R0 diagnostics no longer select package configuration;
  S3/E3 and E3 treatment diagnostics select implementation while staying failed.
- #425 tests use substantive synthetic checks where registration is exercised;
  their authority, failure, Recovery-start and zero-bind-failure assertions are
  preserved. Negative-result/setup/inspect/report exclusion tests retain
  substantive commands so the new inspection filter cannot mask kind gating.

## Boundaries

No event names or schemas changed. Existing/configured and data contracts are
not rewritten to excuse weak evidence; source, release and promotion gates remain
in force. `test -e` now receives the same weak-evidence rejection as `test -f`.
No growth baseline, chokepoint, historical record, live `.anvil` namespace,
Python harness, external service or remote repository state changed.

This is a deterministic regression replay using recorded R0 observations, not
a fresh Next.js build, browser run or live nine-run campaign. The fixture does
not establish full business correctness of the historical apps or fix #420's
separate implementation-step short-circuit behavior. See `verification.md` for
the final check results.
