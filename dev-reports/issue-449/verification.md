# Issue #449 verification

- Status: `passed`

## Checks

- `cargo test --lib issue449`: `passed`
- `cargo test --lib issue448`: `passed`
- `cargo test --lib read_only`: `passed`
- `cargo test --test corpus_regression --test generality_guardrails --test protection_coverage_audit`: `passed`
- `python3 -m pytest -q tests/test_issue448_nextjs_r0.py`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `rustfmt --edition 2024 --check src/minimal_loop/stagnation_escalation/issue449_tests.rs`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `git diff --check`: `passed`

## Results

Verified on 2026-09-09 in `feature/issue-449-cli-recovery-read-only`, based on
`3952f99ff3b2afde40b8d4e88380141044815836`. Repository commands used
`login=false`; Python was 3.12.3.

Focused #449: four tests passed, no ignored tests. Both missing-export and
Promise cases exercise legacy/stable compact and regeneration prompts,
read-only handoff after a UI change, original goal/verify/diagnostic-target
retention and actual Write/Edit. The fourth test checks that embedded goal and
verify headings remain quoted evidence, authoritative plan fields stay intact
and original contract bytes do not change.

Existing #448: five tests passed; read-only selection/stopping: 22 passed.
Corpus: seven passed; growth guardrails: ten passed; protection audit: two
passed. Python frozen-fixture checks: nine passed. No new test is ignored.

Full `cargo test` completed with exit 0 outside the sandbox. Its 74 result
groups (including child-process result groups) sum to 2,799 passed, zero
failed and 38 pre-existing ignored tests. This includes the updated real-runner
compact-repair check, weak-verification refusal, root boundaries, no-change and
UI-only Recovery rejection, and incomplete-candidate promotion refusal.
Formatting and final Clippy both passed on the final Rust changes.

Release build completed with exit 0. The pre-commit local binary reported:

```text
commandagent 0.1.0 3952f99f+dirty 2026-09-09T01:35:25+09:00
```

The dirty suffix accurately identifies the verified pre-commit implementation.
This local check is not the freshly pinned, integrated #452 campaign binary.

## Before/after evidence and resolved findings

Before production edits, the focused `cargo test --lib issue449` exited 101:
one test passed and two failed. The compact test reported `missing-export /
Legacy: lost` followed by the original Japanese goal; the read-only handoff
test reported `missing-export: lost isValidTaskStatus`. These were behavioral
failures after correcting initial test API naming/setup errors. The legitimate
Write/Edit/root/no-op test passed before the fix. After the fixes, the same
two regressions pass for both cases, and the added authority test passes.

The initial Clippy run found a collapsible conditional in the new test setup;
it was corrected without a lint exemption. The initial full sandbox run also
exposed an old test requiring absence of the overall goal from compact prompts.
That assertion now requires the exact original goal; the existing successful
runner repair and sequence assertions are retained. The read-only handoff test
likewise checks the parsed authoritative goal instead of prohibiting diagnostic
step text anywhere in the saved evidence.

The sandbox full-suite attempt reported failures in local provider/server
tests and stalled in several server-dependent tests beyond 60 seconds. It was
interrupted (exit 130), then the complete suite was rerun outside the sandbox
and passed. No failed/partial run is being counted as the final verification.
Raw test/build logs stayed in temporary storage and are not committed.

## Integrity and interpretation

The Python checks validate every frozen #448 fixture hash. A separate read-only
comparison also confirmed all 14 original source/config/lock hashes and all
three source-evidence hashes against #448 provenance. Central R0 event identity
and saved handoff hashes are recorded in `r0-trace.json`.

Compiler outputs in the fast tests are the measured #448 results replayed
through the bounded verifier/parser. No fresh Next.js compiler matrix, live
model run or browser campaign was performed for this prompt/handoff change.
The actual initial-phase diagnostic loss and missing provider observations are
distinguished from the tested context-preservation boundaries in investigation.md.
Fixture improvement does not establish model success rate or original business
acceptance; real effect requires a separate new campaign/B-1.

The parent's #448 original-contract promotion follow-up is not in this pinned
base and remains subject to its own review. This report verifies the #449
development change; it does not declare that dependency or merge gate complete.
