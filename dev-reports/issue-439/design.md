# Issue #439 design

The approved A-1 decision and original Issue (including its empty comments list)
were read before implementation. This worktree starts at `36f73eeb`; the #435,
#420, and #429 fixes are already present. There are no required predecessors.
The 0907 campaign and all referenced reports remain read-only.

Implement command-specific Node evidence in a new evidence leaf module. Resolve
the command's script with its original case and classify its own executable
checks, including conditional nonzero exits. Inline printing, swallowed failures,
and shell failure masking remain weak. Keep test-runner registration independent
of artifacts that will be created later, and classify `node --test` alongside npm
test runners at final acceptance. Actual exit status remains authoritative in
the existing command verifier.

Limit generated Next.js registry exclusions to exact product hook checks covered
by final profile/hook verification; preserve arbitrary checks, configured
contracts, and #435 artifact-only filtering. Reuse command normalization for
exact matching rather than exempting commands merely containing hook names.

Add additive `weak_evidence_sources` telemetry linking each weak reason to its
contract command, obligation, or unbound route. Preserve existing event fields
and gate decisions. Put tests and campaign-derived, sanitized R0/S1 fixtures in
Issue-specific leaf modules and `tests/corpus/apps/issue439-node-evidence`.

Verify classification negatives, future test registration, R0/S1 structural
acceptance, remaining final checks, telemetry, and real smoke command failure.
Run focused tests first, then the checks in `scripts/ci.sh` (including corpus,
guardrails, and conformance). Commit only implementation, fixtures, and reports.
The orchestrator owns integration with #440, pushes, PRs, and the revalidation
campaign; local fixture replay does not assert full business acceptance.

## Equivalence refinement after inspecting final gates

The orchestrator's UAT clarification requires exact target/path/attribute
coverage. `client_component_contract_failure` only requires the directive when
client APIs occur, browser qualification does not identify a source path, and
hook snapshots are conditional on a previous qualifying snapshot. Therefore
these gates do not justify unconditional hook registry exclusions. Preserve the
path-specific commands and their final execution, and classify only exact
product-generated hook predicates as `StaticSyntax`. They cannot supply bound
business verification, and a failing hook still fails the completion contract.
Existing exact package-script exclusions remain; configured registries remain
unchanged. This applies A-1's authoritative equivalence condition conservatively.

| Command | Final coverage | Registry / evidence treatment |
| --- | --- | --- |
| Exact product build-script assertion | Next.js scripts.build invariant | Excluded (existing behavior) |
| Exact dev/start assertion for the requested port | Next.js requested-port invariant | Excluded (existing behavior) |
| Assertion for a different port or altered predicate | Not equivalent | Retained |
| Canonical generated state/action predicate, exact path and attribute | Its retained final command execution; browser alone cannot bind a source path | Retained / StaticSyntax |
| Arbitrary Node mentioning a hook or changing exit 1 to exit 0 | Not a canonical predicate | Retained / command-local classification |
| grep use-client at any path | Baseline invariant is conditional, and covers only app entrypoints | Retained |
| npm test / node --test before artifact creation | Future test run | Retained; Test only after artifact creation |

## Review hardening

Four review counterexamples were reproduced as real Node exit-0 scripts and
incorrect `Test` classifications before the correction. Limit trusted assert
bindings to lexical top level, reject computed assert access and indirect process
access, and refuse literal-boolean control flow when proving a conditional exit.
All four now remain weak. These are conservative recognizer boundaries, not a
new JavaScript interpreter.
