# Issue #488 implementation

At baseline `85dd5fdd12b831deb49e7b3fe9e996c01f1fab67`, #479 already returned
D4 O/V1/SWALLOW to proposal repair. The unmet boundaries were direct registration
and accepting a supported requirement-preserving rewrite. The pre-change dynamic
reproduction is recorded in design.md; this is not the old C5 behavior replayed
as if it were current develop.

- Added a small registration guard at command, StepPlan/producer and empty
  handoff registration. It validates the entire normalized/policy-checked batch
  before contract persistence, producer changes or success events. The existing
  final classifier is run without workspace files and its weak reason passes
  through final acceptance's requirement normalization/pruning.
- Scoped this guard to trusted Config Next.js/create, a same-run owned generated
  unconfigured contract, new final-success inline literal candidates and a
  gate-relevant weak reason. Existing broader formation rejection is retained.
  Both profile checks use the existing `profile::is_nextjs_profile` boundary;
  the final-acceptance test module uses the repository's cfg(test) convention.
  Generality guardrail expectations, scanner and baselines are unchanged.
- Added finite whole-program literal-marker formation: regex literal throw/assert
  or swallowed includes predicates may become direct includes assertions only
  with identical file, ordered conditions, diagnostic messages and success stdout.
  Existing model/host scope projection retains other commands, outputs, responsible
  owners, expected results and boundaries. Raw proposals cannot hide a shell
  wrapper behind sanitizer normalization.
- Reused the existing three planner proposals and fallback finish checks. The
  new guard also checks final candidate returns. Added optional replacement
  metadata to the existing event; no old event name/field/schema was removed.
- Added exact-command/hash corpus, 24 reduced runtime cases, full final-executor
  controls, captured mock requests/returns, registration all-or-nothing tests,
  authority controls, synthetic independent business/compile failure boundaries
  and frozen-byte identity rejection. Actual last-valid returns and clearing
  are separate from direct finish tests.

No final classifier promotion, requirement downgrade, added retry/Recovery
budget, new fixed-verifier routing, parent WIP transplant, guardrail baseline
change, live runtime namespace change or external action is part of this change.
The baseline has no parent-WIP source_verifier_admission module; existing Next.js
profile/build/behavior and identity checks remain separate. V2 passing a text
check does not establish an application's business behavior.

The evidence JSON files are curated test observations and captured request bodies,
not historical or live run logs. Node v24.1.0 ran locally. `classifier_parser_argv`
is the argument vector supplied to the direct Node control. Only status/stdout/
stderr are compared with shell execution; shell-process argv is not observed.
No live model API or browser experiment was performed. The new D4 measurements
do not establish a successful application build or business workflow.
