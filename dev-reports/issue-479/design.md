# Issue #479 design

Base: `0dfc6e570b843eb081934a5dc9e4a67fe5930f4b` (#478), parent and
freshly fetched origin/develop: `fdc98a9b7d865b4c2115138765d3ff7b32612dae`.
The predecessor is retained. Only this worktree is edited; parent owns review,
exact-commit UAT/CI, integration and live evaluations.

## Selected design (before implementation)

Measure all five saved commands with the real classifier, planner admission and
final acceptance. Include the three exact package checks retained by #478 in the
resulting inventory. The expected baseline is Build + four Weak commands; app
edits cannot change the classification of the two pure inline imports.

At preclosure admission, require a retry for immutable weak inline checks within
the existing three proposals. Retain every original model/host instruction,
output, result, check and owner boundary. A small host-validated replacement
record may project a strengthened import back to its original check solely for
the existing preservation comparison. Accept only a closed executable grammar:
the identical local dynamic import followed by a direct strict assertion on a
named export (or a call of that export with literal JSON arguments), compared
with a literal JSON expected result. No catch, success override, unrelated
assertion or arbitrary equivalence metadata is accepted. The model supplies the
target-specific assertion; the host does not invent a business requirement from
the filename. Unsupported/type-only proposals without an appropriate check must
exhaust honestly. The actual returned and subsequently registered command is
the stronger check, and final acceptance still executes it. Tests must run both
the original-import failure and the added assertion failure.

Recognize the saved API existence loop and page includes predicates using
closed, complete structural grammars. Preserve their exact strings and failure
propagation; classify them StaticSyntax, never business Test. Keep package
checks byte-identical and audit their classification/registration separately.

After closure, never rewrite a command or contract/hash. Add command-local
classification, reason and repairability diagnostics. Distinguish a weak inline
command whose evidence cannot change with the app, a file-backed verifier that
can legitimately be created, an existing frozen weak verifier, and executable
Test/Build/structural checks whose runtime failures depend on their inputs.
Use this diagnosis at Recovery admission/confirmation to avoid assigning an
impossible inline-evidence repair to unrelated app owners. Preservation and
candidate refusal gates remain authoritative.

## Rejected alternatives and validation

Appending an unrelated stronger check leaves the original Weak obligation
unresolved. Blanket Test promotion, deleting checks, accepting LLM equivalence
claims and rewriting frozen contracts are rejected. General JavaScript control
flow analysis, new Profile obligations and extra runtime budgets are out of
scope. Leaf modules carry the behavior; growth baselines stay unchanged.

Record the expected red-phase reproduction before production fixes. Add a
source-only corpus and focused actual-product positive/refusal tests, including
final inventory, immutability, missing scripts and genuine Test/structure
controls. Run #478/#466/#465 and setup policy regressions, corpus, guardrails,
fmt, clippy, full cargo test, release build/version/hash and diff checks. Report
worker-local results separately from pending parent gates. Rebuild clean
committed HEAD and return its release version/hash in the final handoff.

## Parent finding: actual type-only target

The parent identified the preserved 786-byte `src/lib/types.ts`, SHA256
`cbc67d6d031f3c029e3379e622471e0b1b1f47ac769a561e3bbc287a31530793`:
Project, Task, TeamMember, CreateProjectRequest, CreateTaskRequest,
UpdateTaskStatusRequest and ApiError are interfaces, with no runtime exports.
The runtime-value control above is insufficient for this saved target. Honest
exhaustion remains a refusal control, not successful formation of this target.
Include this exact source as a new immutable-input corpus copy. Investigate a
second closed strengthening that preserves the original dynamic import and
then type-checks the identical TypeScript target with a real compiler; classify
this as structural evidence and test an erased but invalid type reference as
the added-condition failure. Do not add synthetic runtime exports or claim
business Test evidence from type compilation. This refinement is under local
investigation while the parent owns the ongoing Codex2 discussion.

Parent/Codex2 resolution selects a smaller candidate: the admitted proposal
explicitly supplies a literal sorted runtime export-name set, empty for the
saved interface-only module. Preserve the identical import, then compare
`Object.keys(actual).sort()` with that set using strict deep equality and
propagate rejection. This remains StaticSyntax. The host does not derive the
expected set from a filename and does not claim interface-field validation.
The investigated compiler extension is rejected as unnecessary scope and
removed before verification. Pin a validated replacement's complete command
and expectation on subsequent proposals, including proposals that still need
other owner corrections. Test the exact saved source, import failure,
unexpected export, changed expectation, wrong target, catch, vacuous assertion
and unregistered replacement; preserve #478's full scopes and package checks.

Measured correction from the parent: the original package has no `type` field.
Under Node v24.1.0, the exact saved interface-only bytes import through CommonJS
interop with namespace `["default","module.exports"]`. That explicit set
replaces the unmeasured empty-set assumption for the saved-context positive.
Copy the exact package manifest into the corpus and preserve its module mode.
An empty-set expectation is exercised only in a separately labeled explicit
ESM variant. Neither expectation establishes interface fields or business logic.

## Diagnosis controls and existing gate semantics

Parent/Codex2's attached-flag and frozen-script cases are included. Measured Node
v24.1.0 executes `--eval=...`, rejects glued `-e...`/`-p...` with exit 9, and
does not execute the value attached to `--print=`. One bounded argv discriminator
keeps these forms diagnosable without crediting unexecuted assertions. Known
immutable Weak evidence prevents app-only pending when the fixed contract's
existing source-evidence gate requires it; missing registered scripts retain
their real owner/creation/confirmation path.

The #465 configuration-wrapper regression exposed a necessary limit: a fixed
contract with no capabilities/evidence/obligation requirements relies on actual
command execution and does not enable the evidence-quality gate. Reuse the
existing `source_first_completion_authority_required` predicate for post-freeze
control, rather than adding that gate to such contracts. The wrapper's genuine
related-source repair and configuration-bypass refusal must both keep passing.
New frozen-weak controls explicitly declare implementation evidence, matching
the affected R0 contract. Classification remains Weak in either context; this
is existing gate selection, not Test promotion or relaxation.

The default Rust controls now exercise Node's actual TypeScript stripping and
module-mode-dependent namespace. Declare Node v24.1.0 in every full Rust test workflow (CI, acceptance and release),
matching the saved context and existing Next.js-domain workflow, instead of
depending on an unspecified runner-preinstalled Node. This is a test runtime
prerequisite; app commands, module mode and runtime budgets remain unchanged.
