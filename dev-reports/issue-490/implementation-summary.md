# Issue #490 implementation

Supported package readers now retain one complete obligation on one distinct
Verify execution instead of joining all same-path readers into an output-owner
set. Matching preserves instruction, expected result, required inputs, complete
checks, reader order and both sides of original executable owners (including
split outputs). No expected_paths, runtime prechecks or registered paths are
removed. Unclassified Verify steps sharing the input remain potential owners
and block the exemption.

Support requires the exact existing package-manifest inspection instruction and
nonempty commands accepted by the exact generated Next.js package-check grammar.
Other instructions, creation work, arbitrary inline code, unknown external
scripts and model read-only claims retain the existing ownership rule. There is
no side-effect inference or unconditional Verify exemption.

`admission::preserve` calls the new reader leaf, then applies the unchanged owner
rule to other obligations. Existing model/host acquisition, #488 projection,
formation, lint, finish/fallback, registration, final acceptance and the
three-attempt limit remain in place. Events and schemas are unchanged.

The phase-entry sequence was mechanically extracted from `phase/flow.rs` to
`phase/phase_entry.rs` so portable replay hooks fit without increasing guardrail
baselines. The product still resolves, saves, runs before_phase and registers
before execution, with the same error handoffs and event order. Test hooks only
restore the witnessed setup checkpoint, observe persisted/registered state and
stop after successful core registration. They do not modify proposals or bypass
Admission. The flow chokepoint shrank from 1,690 to 1,575 lines.

## Regression evidence

- P02-D failed before the production change with the documented complete-scope /
  all-owner-boundary error, exhausted three attempts and never registered core.
  The same portable raw input now registers with both package paths intact.
- P01 registers; N01–N05/N07 refuse their targeted loss and leave the core contract
  unchanged. N06 is host reaugmentation recovery, explicitly separate from
  acquired-host refusal evidence.
- R1: actual Admission capture and finish/fallback tests reject missing reader
  instruction, result, checks, input, instruction/check collage, distributed
  checks and ID/path-only correspondence.
- R2: a history acquiring both before/update/after obligations has a passing
  control. Either deletion, cross-update movement or one-execution merge fails.
  Normal-flow tests also register the positive and refuse deletions/movements.
- R3: creation, side-effect, external-script and self-declared-reader controls
  do not receive the exemption, including extra unknown same-path Verify steps.
- R4: every P02-D step has pinned metadata/precheck flags. Original, retained and
  added readers run real package checks for pass, wrong-port failure and missing
  input. Pass short-circuits; failures reach the existing execution/repair path
  and propagate its controlled error. The same runtime test passes with original
  ownership wiring and the fix; missing input remains a missing-path and existing
  dependency-boundary result.
- R5: the existing #478 acquired-host Admission and finish/fallback refusal test
  remains unchanged and is included in related/full verification.
- Registration retains the old contract and adds exactly README.md,
  verify-ui.cjs and node verify-ui.cjs. The sole new producer is script-only
  Implement/pass. Saved/reloaded plan, before_phase result, live contract and
  saved contract agree. `test -f README.md` remains only in the plan.

The committed corpus contains normalized frozen inputs, acquisition history,
variants, expected registration/runtime metadata and provenance hashes. Tests
need no historical run or local absolute path. Original preset build loss is
retained in a separate stage-correct diagnostic; it is not fixed by #490.
No live app/model/GUI success, #488 projection improvement, release or Issue
lifecycle change is claimed.
