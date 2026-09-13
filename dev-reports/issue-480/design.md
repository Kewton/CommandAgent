# Issue #480 design

Base: `3c814d9167975cc8b054a8b84a626f6cf84f82e4` on
`feature/issue-480-cli-acceptance-node`, clean at dispatch. Fetch confirmed
`origin/develop` remains `fdc98a9b7d865b4c2115138765d3ff7b32612dae`.
The committed #478/#479 implementation and passing reports were inspected.

The saved Node hook command echoes all three predicates before its actual
exception. Both Bash executors currently summarize matching source lines first;
display truncation then loses the exception. Attribute repair separately scans
the entire command/reason and gives any state literal priority.

Implement a shared leaf for a bounded, redacted executed Node diagnostic. Capture
the exception and eval stack location before stream/display truncation, then
preserve it through the full-output/failure-reason boundary. Keep existing public
event fields and command/hash boundaries. No extra raw output persistence.

Attribute attribution must bind that executed location to a supported source-read
predicate and its checked path. Recognize a conservative complete program grammar;
comments/literals cannot supply executable predicates, and unsupported control
flow, printed/swallowed errors, other-module failures and ambiguity stay generic.
Sequential missing attributes identify only the first executed throw. Existing
structured profile hook diagnoses remain available. No evidence classification,
acceptance gate, repair authority, budget or guard baseline changes.

First change the #474 handoff assertion to desired error/attribute/path behavior
and reproduce its failure against unmodified production code. Then test actual
Node execution through final acceptance and both real repair prompt layouts,
including ordered missing hooks, long source/truncation, misleading comments and
paths, unknown/nonzero/printed/swallowed failures, namespace assertions, genuine
Test and unchanged source/API refusal controls. Update the driven corpus.

Worker verification: focused tests and #478/#479/#474/#465/#466 regressions,
corpus, event-key and protection audits, growth guards, fmt, clippy, full tests,
release build/version/hash, explicit-path commit, clean committed release rebuild.
Parent owns exact-commit UAT/CI, integration/release and live evaluation. Existing
evaluation rows remain in the denominator with diagnostic limitations annotated;
no historical reports or campaign records will be rewritten.

## Predecessor and authority feedback

Preserved the in-progress #480 changes and fast-forwarded report-only
`ad6a0fcf10904505cec589e0241f514ce8111c64`, then CI/runtime-only
`502a3639893f8f61db8747533848a90f35cdc84b`. The latter is the final implementation
parent. Both predecessors leave production/test/contract bytes unchanged.

A fabricated Node command plus `command failed` has no observed causal predicate;
its former attribute expectation was incorrect. Those guidance tests now execute
an actual supported read/predicate command. Structured profile hook-status
observations remain authoritative through final_acceptance_contract and actual
Runner repair prompts. No lexical fallback is retained. A complete single
node-print read/includes/exit program may diagnose only after a nonzero completed
execution with empty stdout/stderr; a thrown read error stays generic.

Bash captures summary input after its engine-private-line redaction and outside
path annotation, before stream truncation. Verifier execution captures its own
pre-stream-truncation output. The shared diagnostic scrubs secrets/home paths
before serialization. If preprocessing loses exception/location evidence, the
result stays generic. Eval locations use UTF-16 columns, including Japanese and
emoji comment controls. Arbitrary JavaScript, imports, aliases and swallowed or
printed exceptions do not establish attribute authority.
