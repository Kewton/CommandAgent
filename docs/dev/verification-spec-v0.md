# VerificationSpec v0 shadow contract

`commandagent.verification_spec.v0` and prompt
`commandagent.verification_spec.prompt.v0` are frozen Phase 1 formats. Changes
to required fields, intent meaning, claim identity, strength order, binding
semantics, lifecycle, or result meaning require a new version and dual-read
window. Additive readers must not reinterpret a v0 field.

## Authority boundary

VerificationSpec is opt-in shadow data. `StepPlan.verify`, CompletionContract,
the 81-key completion event, adjudication, assurance, and terminal projection
remain authoritative. Generation and validation return a side observation;
schema errors, empty claims, timeouts, unavailable providers, and policy
rejections cannot alter an existing verdict. The only persistence helper writes
`verification-spec-shadow.json` below a caller-selected directory. It does not
write events or change `.anvil/` state layout.

The caller supplies the original goal and explicit intent separately from the
provider response. Accepted output always uses that goal. Both the original
goal hash and provider-goal hash are recorded, together with whether they
matched, so a provider rewrite is visible but never authoritative. The raw
response hash, prompt version, provider, model, request ID, intent, and profile
complete generation provenance.

## Claims and existing contracts

Claim IDs use `[A-Za-z0-9_.-]` and are unique. Every claim has an original
goal byte range or a post-hoc reference to existing fix/investigation evidence,
a normalized requirement, required/optional status, kind, and one or more
oracle IDs.

- create claims use original-goal byte ranges.
- fix claims reference `before_fails`, `after_passes`, or `no_regression` with
  artifact path, stage, expected polarity, lineage, and epoch. VerificationSpec
  does not replace F1/F2/F3.
- investigate claims reference `reproducer_fails` or `diagnosis_bound` with
  artifact path, binding ID, stage, lineage, and epoch. VerificationSpec does
  not replace I1/I2.
- unknown/composite is not a v0 intent. Its golden response is rejected and the
  caller's existing unverified projection remains unchanged.

Oracle bindings repeat their claim ID, and both directions must agree. The
validator deterministically sorts and deduplicates duplicate IDs, missing
references, unmatched bindings, orphan oracles, and unbound claims.

## Oracle and safety contract

Bindings use closed strategies for command/fixture/exit-code/stdout/stderr/file,
HTTP, DOM, interaction, or existing fix/investigation evidence. Setup commands
are argv arrays, never a free-form shell-only value. Inputs and observations
have tagged closed forms. Bindings record expected polarity, minimum and
observed evidence strength, bounded timeout, lifecycle (`proposed`, `validated`,
`bound`, `executed`, `blocked`, `unverified`), and result (`pass`, `fail`,
`partial`, `unverified`, `blocked`, `oracle_error`). Phase 1 does not execute
these candidates.

Paths are workspace-relative and reject absolute paths, NUL, and parent
traversal. HTTP routes must be rooted local paths without authority or parent
traversal. Argv elements reject empty, control-character, oversized, and
over-count values. Fixture and binding hashes are 64 hexadecimal characters.
Proposed and concretized bindings each carry a hash and an explicit semantic
equivalence decision. Existing syntax-only `VerifyCommandOracleRepair` kinds
may be recorded in `repair_kind`; a changed required claim, polarity,
observation, or strength is not semantically equivalent.

## Frozen limits

- provider JSON: 65,536 bytes
- original/provider goal: 8,192 bytes each
- claims/oracles: 64 each; oracle references per claim: 16
- ID/profile: 64 bytes; normalized requirement/expected text: 2,048 bytes
- path: 1,024 bytes
- argv: 32 elements, 4,096 bytes per element
- timeout: 1 through 300,000 ms

The JSON Schema and provider prompt snapshots live under
`eval/goal_verify/v0/`. Golden proposals live under
`tests/fixtures/verification_spec_v0/`.
