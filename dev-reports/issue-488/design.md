# Issue #488 design

Baseline: `85dd5fdd12b831deb49e7b3fe9e996c01f1fab67`, clean dedicated
`feature/issue-488-cli-acceptance-d4-inline` worktree. No predecessors assigned.
Read the complete parent-saved Issue body, worker skill and development guardrails.
Parent `e99f1ebe` plus WIP and historical C5 `f0aceed8` are separate contexts;
neither is an implementation source to copy or cherry-pick.
Relevant committed ancestors inspected: `3c814d91` (#479 verifier formation),
`d9ad6f5d` (#480 executed failure predicates), and `7125ce93` (#484 package-script
formation). `HEAD` and the dedicated worktree's `origin/develop` both resolve to
the full baseline SHA above; no parent mixed-worktree changes were assumed merged.

## Existing coverage and reproduction plan

The actual Runner calls Admission after normalization, augmentation and policy
sanitization, before lint, and uses Admission::finish on every fallback return.
#479 already rejects weak inline commands here using the final classifier; #484
adds captured package-script obligations. Those changes are in this baseline.
The three registration paths are register_step_plan_commands,
verifier_obligations::register and complete_generated_verify_commands (handoff).
They validate command policy but do not check immutable weak evidence.
The baseline has no source_verifier_admission or C5 fixed_verifier_routing module.
Existing profile and final-acceptance gates remain authoritative and unchanged.

Before production edits, reproduce exact D4 command classification, Admission
retry, rejected requirement-preserving rewrite and registration bypass with real
functions. Preserve command bytes/hashes from the read-only historical input;
use new reduced marker fixtures, not the disqualified historical argv recorder.

## Smallest change

Keep the existing broad formation rejection. Add a leaf registration guard for
new final-success literal inline Node candidates only when trusted Config says
Next.js/create, current-run provenance owns the generated unconfigured contract,
and normalized trusted requirements make their source-independent weak evidence
gate relevant. Use the final classifier and requirement evaluation, not a second
weakness heuristic. Validate the entire candidate batch before persistence or
producer-obligation updates. Existing configured/closed handling is unchanged.

Extend preclosure projection with a closed whole-program grammar for literal
file marker predicates: ordered literal-regex throw/assert or swallowed includes
assert to direct includes assert. Preserve path, all ordered conditions, messages,
success stdout and expected result; only literal regexes with escaped metacharacters
qualify. No arbitrary JS equivalence. The existing model/host scope projection
must still enforce outputs, responsible owners and execution boundaries. Record
original command, normalized command, source step and checked replacement.
Allow this projection only for owned eligible draft checks, never frozen checks.

## Verification

New corpus and focused Rust tests cover exact four commands on normal, four
single-missing-marker and missing-file fixtures with real Node; final static
classification and actual execution remain separate. Mock-provider tests capture
actual request bodies and returned plans for repair, exhaustion, scope loss and
fallback. Test both mixed candidate orders and all-good batches at all registration
entries; authority/requirements controls; existing profile gates, independent
business failure, compile/missing requirements and identity preservation.
Run focused tests, related corpus, fmt check, clippy with warnings denied and full
cargo test. Record failures honestly. No live model, browser, build-success claim,
parent WIP edit, runtime namespace change, push or external lifecycle operation.

## Baseline dynamic result (before production edits)

`cargo test issue488_baseline --lib -- --nocapture` passed on the baseline with
only corpus/test additions (1 test). Actual Admission returned Retry for O/V1/
SWALLOW, Ready for V2; StepPlan registration accepted all four; final static
acceptance rejected O/V1/SWALLOW and passed V2. The V2 rewrite after a weak first
proposal was rejected by exact-command preservation. Thus normal weak refusal
already exists, but registration defense and supported rewrite formation are
unmet. No C5 routing transplant is needed. First test compilation failed because
the test supplied typed deferred requirements to the string API; corrected the
empty fixture argument before obtaining this result.
