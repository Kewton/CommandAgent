# Issue #488 D4 inline checks

Baseline: `85dd5fdd12b831deb49e7b3fe9e996c01f1fab67`. Exact command strings and
SHA256 in `commands.json` were extracted from the complete Issue #488 body and
checked against the immutable C5 `inputs/commands.json` below:

`workspace/tmp/0915/recovery-improvement-execution-01/c5-d4-linked-controls-20260916-01/`

C5 commit `f0aceed8c44748855a543ae7a122e3ffefb43cbc` and parent develop
`e99f1ebe` plus WIP are separate contexts, not code transplanted into this change.
No historical evidence, old instrument, parent WIP, credentials or live state is
included. `contract.json` is the Issue's minimal initial generated contract.
The test establishes ownership with `begin_run`/`record_generated_contract`.

## Fixtures and expected stages

`src/app/page.tsx` and `fixtures/normal.txt` have the four requested marker lines.
`primary.txt`, `input.txt`, `state.txt` and `snapshot.txt` each omit just the named
condition. The sixth fixture is created by removing the file in a temporary test
workspace. These are new reduced text fixtures, not a buildable Next.js app and
not a rerun of the historical C-2 app or all 24 C5 cases.

`matrix.json` specifies 4 commands × 6 fixtures. The focused test consumes its
execution/classification/formation/registration expectations. Final isolated
acceptance additionally requires command success and required paths/evidence.
The generic corpus test exercises O's real static weak-evidence refusal.

| command | real Node normal / missing marker / missing file | final classification | formation / new registration |
| --- | --- | --- | --- |
| O | 0 / nonzero / nonzero | weak | retry / reject |
| V1 | 0 / nonzero / nonzero | weak | retry / reject |
| V2 | 0 / nonzero / nonzero | Test | ready / accept |
| SWALLOW | 0 / 0 / nonzero | weak | retry / reject |

V1 already has assertions. O and V1 detect omissions at runtime, but the current
conservative classifier does not recognize their regex failure path. The reason
`node_smoke_without_assertion` is preserved for compatibility, not interpreted as
proof of absent assertions. Classification is independent of fixture contents.
The real process test compares shell execution with **the classifier's parser**
and a second Node process using that exact parsed argv, checking stdout/stderr
and exit status for every case. Shell-process argv is not observed; the saved
`classifier_parser_argv` is supplied to the direct process only. The old
disqualified argv recorder is not reused.

## Scope and controls

A literal-marker rewrite preserves the exact file path, four ordered conditions,
diagnostic messages, stdout, pass result and model/host owner/output boundaries.
The supported whole-program grammar excludes arbitrary JS, nonliteral regexes,
extra statements and shell wrappers. `refusals.json` is consumed by mock-provider
regressions for condition/target/output/owner/result/order loss and masking.

The four mock series cover accepted repair, three weak replies, weakened repair,
and fallback. The last-valid positive case actually reaches the driver's
quality → lint/schema degradation → saved-plan return; a separate case observes
formation Retry clearing that saved plan. Direct finish checks are additional
controls, not evidence that a driver branch ran. All provider replies are local
successful mocks; provider transport retries and Recovery budgets are unchanged.

All three registration entries test both mixed-candidate orders and all-good
batches, including verifier creation obligations. Configured/unowned/registered
contracts and Config profile/intent/requirement controls distinguish no new rule
from the older broad #479 rejection. Existing normalization rejects unsplittable
inline compounds; simple `test -f ... && npm run build` still splits. Baseline
command policy accepts a nonliteral Node spelling; the new literal-only rule does
not change that policy or grant it evidence. No nonliteral command is executed.

`business-controls.json` is synthetic data tested by an independent real Node
oracle and handed to the existing behavior gate. It is not an observation of an
app saving or reloading. Compiler failure is likewise a synthetic observation
at the existing report boundary. The final executor tests use a generic profile
to isolate command/evidence behavior and separately assert the Next.js profile
still rejects this reduced non-app fixture. This baseline has no separate
`source_verifier_admission` or C5 fixed-verifier routing module; those parent WIP
modules are not imported. Existing Next.js profile, compile, behavior and frozen
contract-byte identity gates remain in force.

## Difference from existing corpora

#479 covers pure imports, export-set refinements and generated structural checks;
#484 covers saved package-script literals and their model/host owners. This corpus
adds exact D4 regex/assert/swallowed marker checks, finite literal-file rewrites,
registration atomicity at every entry and the real driver return controls.
Run `cargo test issue488 --lib` and `cargo test --test corpus_regression`.
