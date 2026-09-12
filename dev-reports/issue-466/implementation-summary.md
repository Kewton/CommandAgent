# Issue #466 implementation

New generated plans close verifier creation obligations only after their output
owners and checks agree. Recovery preserves those obligations and returns only
proposal-correctable ownership failures to the existing three-attempt planner
loop. Closed partial existence and unsafe provenance/path conditions stop before
execution with their original reason.

## Product changes

- `recovery_contract_authority/verifier_obligations.rs` registers dedicated
  Implement/pass producers from admitted generated plans. Compound checks use
  the same normalization and final-success admission as the existing registry.
  Script identification first passes through the shared typed
  `NormalizedVerifyCommand` boundary; the protection coverage audit has no new
  exemption. Inspect, Report and Setup commands cannot extend verification authority.
  Required outputs from the admitted plan are retained in the generated run
  contract. Configured contracts remain closed.
- `recovery_step_plan_binding/admission.rs` diagnoses mixed producer scope before
  closure, records model and post-host proposals, and requires corrected owners
  to retain the original requirements, expected results and outputs. Checks on
  a split group remain together at a Verify after all corresponding owners.
  Unrelated owners carrying the original text cannot discharge those duties.
- The existing three-attempt generation loop handles ownership feedback along
  with schema/lint attempts; the Recovery attempt limit is unchanged. Every
  plan-return path checks retained scope, including setup fallback. Invalid
  deterministic templates stop without discarding their requirements.
  Invalid verification-command syntax remains with the existing lint diagnostics,
  corrective retry and last-valid-plan behavior; it never closes a new registry.
- `recovery_inspection/verifier_obligations.rs` binds each wholly missing producer
  to one compatible Implement or restores its complete registered definition
  when no owner exists. IDs remain stable for compatible proposed owners;
  restored IDs derive from the complete original step. Binding is transactional.
  Original producer checks and required paths also remain on final host Verify.
- Existing verifier artifacts are preserved; proposed write owners must be
  removed. A partially present closed group is original-inconsistent, never
  repaired by deleting its existing paths. Wrong kind, incomplete/mixed output
  ownership and multiple proposed owners receive corrective diagnostics.
  Duplicates, unsafe aliases, symlinks, hard links, protected/private outputs and
  unregistered configured-verifier owners cannot grant creation authority.
- A host-memory seal carries attempt identity, contract hash, exact producer
  definitions and existing verifier fingerprints through the inspection context.
  Missing/replaced/reduced records and changed existing verifiers fail closed.
  Legacy unsealed creation obligations cannot acquire new host authority.
  The #456 continuation test now requires an explicit error when a sealed
  contract is removed, instead of treating its origin as absent.
- Execution checks binding and final verification before starting steps and
  rechecks preservation at existing Runner completion boundaries. Final
  verification, acceptance, adoption and #465 repair confirmation remain required.

Existing event names/fields remain. New events record formation/admission,
source and proposed owner scope, rejection class/reason, and planner attempts.
The optional inspection-context fields are backward-readable; legacy artifacts
are not silently promoted to new creation authority.

## Tests and source-only corpus

`tests/corpus/apps/issue466-verifier-obligations/` contains a 20-case existence /
ownership matrix, an executable failing verifier, a compact legacy-context
envelope, and exact E1/E2/E3 producer arrays with original-context hashes.
Historical full proposals and model/host contribution remain unknown.

Focused Runner tests cover feedback followed by real Write and verification;
budget/unsafe/partial-existence refusal without executing a rejected plan;
pre-execution requirement/check removal; context mutation; protected/configured
paths; parser quoting and aliases; actual host augmentation; compound-command
registration; owner-specific formation; and schema/lint exhaustion leading to
setup fallback, with the ordinary fallback permission control retained.

## Scope and limits

No claim is made that the historical campaign now succeeds, that its missing
proposals were reconstructed, or that a newly created verifier is semantically
correct merely because its instructions were preserved. The implementation
conservatively carries all original text to the responsible owners, preserves
the checks, and still requires actual host verification and final acceptance.
Unformable plans may exhaust the existing budget rather than lose requirements.

No live model/API campaign, external lifecycle action, push, PR, develop merge,
shared-service change, or other-worker message was performed. Existing historical
evidence, live `.anvil`, original develop worktree and plugin cache were not
modified. No README/CHANGELOG, generated app or guardrail baseline was edited.
