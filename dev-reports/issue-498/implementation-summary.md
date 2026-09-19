# Issue #498 implementation

The saved L2 proposal now stops at its first capacity rejection. It preserves
the full 2786-character augmented instruction and records the 2766-character
minimum implied by its independent 529-character model and 2237-character host
duties. No second model request or application/fallback execution occurs.

`bounded_repair.rs` is an admission leaf containing a closed infeasibility proof
and a linear-time Unicode-scalar overlap calculation. Capacity and provenance
failures are distinct types. The proof checks first-attempt acquisition,
unambiguous IDs, exact retained scope, original-to-preservation duty identity,
package source/contract hashes, marker acquisition state, matching normalized
multi-path Implement/pass scopes and the registered package-only split gate.
Unknown shapes and lower bounds at or below 2500 retain normal admission/retry.
Goal canonicalization does not change the captured duty identity; both goal
views remain available in event evidence.

The package split source predicate is shared with its existing admission code;
its candidate validation, ordering requirements and lint behavior are unchanged.
The new proof never performs the downstream ordering decision. No runner or
minimal-loop code, guardrail baseline, generated application, historical evidence
or live `.anvil` state was changed.

The existing admission event and mandatory fields remain readable. Legacy
`classification=proposal_repairable` is retained. Explicit
`terminal_reason=bounded_repair_infeasible`, `retry_allowed=false` and
`status=stopped` take precedence for new consumers. The event retains two
remaining planner attempts and an unchanged recovery budget, and includes the
proof, preservation view and marker evidence alongside existing frozen sources,
uncut original scope, package evidence and registered contract. The final error
has no repairable/exhausted cause.

Tests cover independent saved-fixture hashes/arithmetic, containment and both
overlap directions, Unicode boundaries, normalized aliases and path ordering,
duplicate/fragmented/partial owners, acquisition/provenance negatives, unknown
three-attempt exhaustion, initial bounded readiness and overlap repair to Ready.
Existing package split, Recovery, configured-contract, template, model-only
truncation and fallback controls remain covered. Saved duty-loss checks now call
preservation directly; schema/empty/mixed failures use a reachable overlapping
fixture and assert that their relevant admission/fallback boundaries were reached.

The corpus now records the terminal expectation without changing the saved plan
or host text. A single-proposal replay was also run in a new SSD directory; see
`workspace/tmp/issue-498-worker-replay-20260919/report.md`. No live provider call
or raw-response replay is claimed. Full check results are in `verification.md`.
