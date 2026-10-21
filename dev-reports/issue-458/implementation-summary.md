# Issue #458 implementation

An eligible failed Recovery treatment can now be rejected while retaining a safe
continuation for the remaining automatic-run budget. A second treatment starts
from unchanged control bytes, receives the latest diagnosis and original
create/fix inspection provenance, and is promoted only after the original
registered observation and completion acceptance pass.

## Changes

- Separate the control-retention transaction from continuation classification.
  Preserve interruption; stop unknown/configuration/provider failures. Recheck
  observer authority, control drift and protected-path changes before either
  retry or promotion. Existing rollback/adoption checks remain active.
- Validate the actual child YAML and its resume metadata before rebuilding.
  Preserve missing/corrupt/review/drift/confinement stop codes. Normalize targets
  against both treatment and control, including canonical symlink targets and
  private/protected/`.env` refusals. Discard unverified completed-artifact claims
  and bind verification commands and goal to the original control contract.
- Build post-observation continuations only for supported failed observations.
  Next.js spawn/early-exit/port/timeout/environment/unknown failures are
  unavailable; typed compiler diagnostics or an actual matching failing HTTP
  response establish supported readiness failures. Verification inability,
  inconsistent acceptance, authority changes and promotion failures stop.
- Keep canonical-plan cycle checks and add a separate semantic observation
  identity. Normalize known observation-root paths and the verifier's elapsed
  time header while retaining command output/status, compiler codes/messages
  and individual profile failures. Latest readable diagnostics remain in the
  saved plan/events. Changed semantic diagnoses at the same command/location
  can continue; a repeat with only timing/path noise stops.
- Allocate fresh preflight checkpoints (0 initially, 64 + consumed count for
  continuations) separately from treatment boundaries. Check the run limit
  before another observation; do not increase limits or local repair budgets.
- Publish the additive `recovery_continuation_prepared` event only after safe
  control retention and plan validation. Limit/cycle terminal projections select
  the latest control-owned manual YAML; success clears old handoff fields.

The decision table and explicit eligibility list are in `design.md`. No
verification, acceptance, evidence, release or promotion condition was weakened.
Runner chokepoints, growth baselines, existing event names/fields and the runtime
namespace are unchanged. The only existing test harness adaptation is the new
consumed-count argument to preflight in the #425 helper.

## Deterministic evidence and scope

Ten new tests include data-driven production-driver sequences, terminal finish
cases, invalid-child controls, real registered failing commands with varied
timing/cwd and changed semantic output, compiler/profile identity controls,
readiness classification, a real early-exit readiness probe with a declared
scripted npm fixture, and direct/aliased `.env` controls in both workspaces.
The sequence harness injects execution results/file effects and delegates all
observations, YAML/resume checks, snapshots, finish and adoption to production.
Existing #456 real model-protocol/tool replays run in the Recovery regression.

The new corpus records bounds, safety decisions and ordered event boundaries.
Ordinary rejection retains identical source hashes; rejected-only files are
absent from the next treatment/control. Permission-denied promotion remains a
terminal failure. An unconfigured observer is rejected by real preflight/startup
before the defensive finish branch is reachable; tests do not manufacture
observer authority to bypass that gate.

This establishes deterministic retry and gate behavior, not model repair rate.
No live model, Browser campaign or #452 evaluation was run. Arbitrary application
output numbers are retained as semantic data rather than guessed to be timing;
the configured maximum remains the bound for changing/unstructured output.

## Provenance and handoff

Verified predecessor commits #456 `32f997f4` and #457 `f2253cb7` were inspected
and imported by fast-forward. The designated develop-root analyses and archive
were read only. The archived central event hash still equals
`f7a30626804db6072de3f0f332259cf40f6a36a2ab932d847e7e5d55b236fe38`.
Historical evidence, other worktrees and live `.anvil/` state were not modified.
Work is committed on `feature/issue-458-cli-recovery`; push/PR/merge/Issue state
changes remain with the parent.
