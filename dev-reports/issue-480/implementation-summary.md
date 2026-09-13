# Issue #480 implementation

The saved primary/input/state verifier now carries its executed Node exception
through the bounded build reason to the correct attribute, checked source path
and actual final repair prompt. Multiple missing hooks select the first executed
throw, including controls that swap primary/input predicate order or give the primary
throw the same misleading error label as the later state throw.

## Production change

`src/node_failure.rs` captures an error header plus its first eval stack frame
before executor stream truncation, scrubs secrets/home paths, and emits a complete
record of at most 400 bytes. The record binds to the exact command SHA256. Both
Bash and the normalized verifier executor use it. Bash's existing private-line
redaction and outside-workspace annotation run before this capture; missing or
altered causal evidence cannot be recovered by scanning echoed source.

`FullCommandOutput::failure_reason` preserves that record at the front of the
existing 500-character build-reason budget. `excerpt` remains a display excerpt.
The observation/report boundaries accept only the complete executor summary or
its known lifecycle reason wrappers. Existing raw-output storage is unchanged;
no new log, event field, schema version or unbounded output channel was added.

`src/node_failure/predicate.rs` recognizes complete source-read/includes/throw
programs, binding the error message and UTF-16 eval location to one executed
predicate and the read's path. Comments are skipped as tokens while their source
offsets are retained. Imports, aliases, extra reads, catches, printed diagnostics,
opaque control flow and unknown predicates do not grant attribute authority.
A separate complete single `node -p` read/includes/exit form requires an observed
nonzero execution and empty stdout/stderr; read exceptions stay generic.

`contract_attribute_repair::detect` consumes this causal binding. The former
command/profile string scan and lexical state priority were removed. Command text
plus a fabricated `command failed` reason is now generic. Existing structured
hook-status guidance remains intact through `final_acceptance_contract`; the
actual Runner regression now requires the missing restart attribute and source
path in the request sent to its execution client.

## Controls and compatibility

The #474 saved handoff now asserts all three correct errors/attributes/paths and
both actual final repair prompt layouts. Its existing unrelated-API refusal,
source-restoration acceptance, immutable registry and StaticSyntax controls remain.
Five older guidance/target tests now use observed product execution instead of
unsupported synthetic failure text, preserving their guidance/priority assertions.

The new corpus drives 13 positive variants and 12 generic controls. Positive
variants include ordered/reordered missing hooks, another checked path, misleading
comment paths/errors, Japanese and emoji source text, 499/500/501 and
23999/24000/24001/32000-character comment payloads. The tests follow raw Node
stderr through real final acceptance, bounded reason, attribute/path detection,
both final prompt layouts and the step repair prompt. Direct executor controls
cover secret/home redaction and private-output removal. Every incomplete record
prefix, another command hash and multiple diagnostics refuse attribution.

Namespace assertions containing `data-anvil-state` remain generic attribute
failures and keep #479 StaticSyntax classification. Actual business assertions
remain Test. Printed, swallowed, removed, unknown and other-read/module controls
cannot clear the existing final gate. No classifier, verifier, contract hash,
repair authority, candidate-discharge rule, runtime budget or guard baseline was
changed. No knowledge manifest, live `.anvil` namespace or historical report was
modified. Runner and loop chokepoint production files remain unchanged.

## Limits and handoff

This is a deliberately closed diagnostic grammar, not a JavaScript interpreter.
Unsupported syntax, missing/truncated/redacted locations, oversized records,
ambiguous diagnostics and errors in imported modules retain generic guidance.
Generic guidance never converts an execution failure into acceptance success.

The task parent is `502a3639893f8f61db8747533848a90f35cdc84b`, incorporating both
requested #479 follow-ups without losing the #480 working changes. Parent owns
exact-commit UAT/CI, integration/release, review and live evaluation. Existing
evaluation rows remain in the denominator with their original diagnostic
limitations; none were rewritten, excluded or claimed as evidence of this fix.
