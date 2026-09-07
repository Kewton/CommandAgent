# Issue #439 implementation

Node evidence now inspects the referenced script with its original path/case,
instead of borrowing assertion strings from unrelated workspace files. A bounded
recognizer accepts direct built-in assertions and conditional nonzero exits in
executed statement bodies. Display-only code, uncalled function checks, swallowed
failures, shell masking, and missing/unsupported script targets remain weak.
Execution still determines whether each registered check succeeds.

`npm test`, `npm run test`, pnpm/yarn tests, and `node --test` remain registered
before test artifacts exist. At final acceptance they are `Test` when test
artifacts exist, otherwise `node_test_without_test_artifact` remains weak.

Exact product-generated hook predicates are `StaticSyntax`, retain their source
path and attribute, and still run at final verification. They do not supply
business-success evidence. The equivalence audit in `design.md` explains why
browser observations and conditional use-client validation do not justify
dropping these path-specific commands. Existing exact package-script exemptions
and #435's `test -f` / `cat` filtering remain. Explicit configured contracts are
unchanged; no historical contract was edited to create a passing registry.

Review hardening reproduces four actual exit-0 counterexamples: a nested require
binding followed by an unrelated no-op assert, bracket mutation of process.exit,
bracket mutation of assert.equal, and an unreachable false-and-condition exit.
Trusted assert imports must be at top level; computed assert access, indirect
process access, and literal-boolean control flow remain unknown/weak.
Two derived cases also reproduce and reject nested shadowing of a top-level
assert binding and mutation through an assert alias. Runtime negatives use the
existing bounded-process runner with a five-second limit.

`ultra_final_acceptance` adds `weak_evidence_sources` entries containing `source`,
`reason`, and the exact `command` for command evidence. Existing event names,
fields, reasons, gates, and assurance semantics are preserved. Source entries
removed by evidence arbitration are also removed from the provenance list.

The Issue-specific corpus copies R0 source and its actual smoke script, stores
S1 treatment source and the recovery-runtime contract, and includes selected
historical event/browser fields plus negative Node fixtures. Twenty-one source
hashes match the retained campaign inputs. Tests exercise StepPlan registration
through generated contract refresh and Recovery authority/handoff, configured
registry immutability, future test artifacts, additive final-event output, and
actual smoke/hook command failure on copied sources. The existing final-event
key snapshot adds only the new field. No growth baseline was changed.

The existing max_parallel Python test depended on incidental repository text
containing `Independent`. Apply the byte-identical test-input isolation from
#440 commit `aaf5c87f`: mock only rg enrichment for that test and preserve every
batch, order, and width assertion. Scheduling production code is unchanged.

Scope limits: the JS recognizer is conservative, not a general interpreter;
opaque control flow may remain weak. Historical build/browser observations are
replay inputs. The fixtures remove the weak-evidence acceptance blocker but do
not demonstrate full S1 business correctness or a new live promotion campaign.
The orchestrator owns integration with #440 and subsequent CI/UAT, pushes, PRs,
and lifecycle actions.
