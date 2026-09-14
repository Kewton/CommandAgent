# Issue #484 design

Baseline: `b7b65f8fd75793f2075491889c66372b02a06d14`, equal to fetched
`origin/develop`. The committed #478 model/host separation, #479 pure-import
formation and #480 executed failure predicates are present.

Read before implementation: `docs/dev/dev-guardrails.md`, the approved
`20260914-issue484-485-content-review-01/issue-484-revised.md` and
`expected-value-source.json`, and the saved preclosure observation referenced
there. Historical evidence stays read-only.

Implement a separate leaf for the closed `node -p` package-script grammar.
Accept only a root `require('./package.json').scripts.<static identifier>`.
Require an unambiguous fixed string in a saved executable package owner, using
the observed `Update package.json scripts so that dev is '…' and start is '…'.`
grammar. Parse the complete declaration, reject alternatives/conflicts, and
never derive a literal from app files, command output, or a later proposal.
The known host profile port constraint restricts a saved literal; it cannot
supply one. Retain both model and host duties without precedence.

Capture source plans at the profile augmentation boundary, before sanitization,
and retain their source location, JSON pointers, owner, stage and SHA-256 hashes.
Store package expectations and the original registered contract on the first
formation failure. Proposed replacements must read the same target, compare
the exact string with `node:assert/strict`, and print the observed value after
the assertion. The closed whole-command grammar excludes wrappers, dynamic
properties, unrelated assertions, swallowed errors, and output suppression.
Project only validated replacements back to original commands for the existing
model/host scope and ordering checks. Record before/after commands, hashes,
expectation provenance and owners in additive event fields.

Classify these executed comparisons as `StaticSyntax` (structural), preserving
the existing business-Test and implementation-artifact gates. Do not reopen
configured/frozen contracts, change recovery/planner budgets, or grow runner
chokepoints. Limit edits to source leaves/wiring, new tests/corpus and this report
directory.

Verification: replay saved proposals through the real runner/admission without
a live model; test positive formation and owner/order/scope/expectation refusal
controls; run actual Node for matching, missing, mismatched and absent-file
cases, including stdout and failure propagation. Exercise registration/frozen
contract hashes and structural-only final acceptance. Run #478/#479/#480
regressions, the corpus comparison, formatting, all-target Clippy and full Cargo
tests. Parent owns PR, exact-HEAD CI/UAT and integrated release version/hash;
those stages will remain explicitly pending in the worker report.

Parent design feedback `issue484-design-feedback-01.md` incorporated:

- Literal grammar is nonempty ASCII alphanumeric plus space and `_./:=@+-`.
  Quotes, backslash, newline, backtick, dollar expansion, shell operators and
  Unicode are rejected before formation; JSON encoding and outer shell quoting
  remain separate operations. Test both rejected characters and real Node
  roundtrips for every permitted punctuation.
- Resolve each script key as a set of original explicit literals: equal model
  and host values agree; differing values conflict; another key is a separate
  preserved duty. The exact known host profile guidance constrains the saved
  value's port and never selects among the permitted command spellings.
- Real Node controls fix stdout to the original observed string plus newline
  on match, and empty stdout/nonzero exit on comparison failure. Missing script,
  missing file, invalid JSON, catch/wrapper and output suppression are controls.

Keep the existing post-sanitization `original_obligation_sources` for #478 scope
projection. Separately preserve `package_script_formation.sources.plans` at the
earlier profile augmentation boundary for literal provenance; records use JSON
pointers into this immutable snapshot. This avoids reinterpreting legitimate
existing sanitizer transformations as model/host duty loss.

Execution-path findings and final bounded wiring:

- The shared verify policy's substring check interpreted the literal `next dev`
  as a server launch. Only the whole recognized comparison grammar bypasses that
  substring classification; arbitrary Node, shell wrappers and real server
  commands retain their existing rejection. Raw retry proposals are checked
  before a shell fallback can be normalized into the accepted command.
- The saved 547-character model duty plus 2,237-character host duty cannot fit
  the existing 2,500-character step limit. The proposed plan keeps both complete
  in two ordered Implement/pass writers of `package.json`, and moves the entire
  original check group after both. A dedicated scope leaf requires the original
  model ID/text, exact host text on a new ID, exact package scope, no third writer
  (including aliases), and host source verify empty. Separate checked views
  retain the existing #478 preservation checks and original output boundaries.
- Lint recognizes that same concrete ordered package split, including known
  full profile guidance, target-dependent fixed-value comparison and all writer
  checks after both writers. Other duplicate ownership remains rejected.
  Admission additionally proves both duties against their saved sources.
- Preset/runtime setup optimizations preserve explicit package declarations and
  observations instead of retyping their owners or replacing checks with the
  profile subset. Both owners must actually execute. No instruction, attempt,
  Recovery, test or evidence budget is raised.
- Leading/trailing or repeated spaces are excluded from literals because the
  existing command normalizer collapses whitespace. Every accepted spelling
  therefore remains unchanged through registration and execution.

The parent `owner-split-review-01/worker-feedback.md` is covered by
`owner-refusals.json` and `issue484_owner_tests.rs`: model/host text, result and
scope changes; missing/third writers; alias and non-normalized paths; ID reuse;
checks before or between writers; partial groups; nonempty host source verify.

Final code-review feedback (`code-review-484-01/worker-feedback.md`): the caller
captures each proposal at `strengthen`, then calls `check` before lint. A Ready
result may still be followed by a lint retry, so pending sources must be replaced
for the next proposal until the first `retain`. The capture test exercises that
path and proves the retained original and snapshot belong to the same second
proposal; later `strengthen` cannot change it. An explicit caller-state invariant
refuses two `strengthen` calls without an intervening `check`, including fallback.
This does not pin stale sources from an earlier unretained proposal.

The original template prompt regression also remains intact: a generic
Implement instruction that merely confirms a port is not promoted to a new
implementation duty by its weak display command alone. The new optimization
boundary applies to explicit fixed declarations, formed comparisons, and Verify
observations.
