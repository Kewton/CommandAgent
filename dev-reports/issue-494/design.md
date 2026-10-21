# Issue #494 design

Baseline `b1d560bd4004f4b56e08f586f742a070ee64ffbe` matches
`origin/develop`. The committed #479 import-formation, #484 package-script
formation, #490 reader ownership, and #492 build-duty changes are present. The
Issue's frozen `-02` control is available read-only at
`/Volumes/SSD_NX/tmp/commandagent-l2-repair-shape-control-20260919-02`; its
published request/result/event hashes match the stored files.

## Selected design (before implementation)

Extend the existing import-check leaf with a separate CommonJS grammar. The
original recognizer accepts only the shell-tokenized three-word command
`node -p <source>`, where `<source>` is exactly one `require()` call containing
one single- or double-quoted `./` local literal accepted by the existing path
policy. It rejects every other executable, flag, argv count, dynamic or
nonlocal target, trailing token or statement, and shell composition. Recognition
does not promote the original: it remains Weak
`node_smoke_without_assertion`.

The corresponding strengthened grammar also stays closed and same-loader. It
uses `node -p`, assigns the result of the identical literal `require()` to
`actual`, strictly compares `Object.keys(actual).sort()` with an explicitly
supplied sorted JSON string array, and ends with `actual`. The final expression
preserves the original print/output behavior as well as load and module
evaluation failures. The proposal, never the host or workspace, supplies the
expected export set. This accepted shape is structural evidence, not a business
Test. Arbitrary JavaScript, catch/success overrides, direct source inspection,
and `require()` to dynamic-`import()` substitution remain outside the
correspondence.

Teach verifier projection to select a grammar from the original loader and
admit candidates only from the same grammar with the identical target and
expected result. Continue projecting the accepted command back to the original
only for the existing full preservation proof. That proof remains responsible
for complete instructions, required outputs, corresponding ownership, and
owner order, including already-legitimate owner movement. The returned and
registered plan retains the exact strengthened command. Persistent replacement
records continue to prevent later retry or finish paths from dropping it or
changing its expectation.

Add test-only admission tracing, without product event/schema changes, at the
successful preservation and `require_formed` boundaries. The focused positive
test will correlate one attempt's original command, target, expectation,
replacement, and formed command while separately asserting lint, quality,
finish, and normal registration.

## Fixtures and validation

Add a source-only, hash-bound `tests/corpus/apps/issue494-require-formation`
fixture containing the exact retained original, the saved source-text proposal
A, prior dynamic-import proposal B, a minimal same-loader positive, grammar and
scope refusal cases, the frozen request/result/event hashes, and the configured
and Recovery contract command/hash controls. Focused tests will consume this
fixture rather than reconstructing the cases in Rust.

Runtime tests pin Node `v24.1.0` and explicit package/module modes. They cover
ordinary CommonJS load, missing target, evaluation throw, wrong export set, and
an explicit ESM/top-level-await graph where `require()` fails but dynamic
`import()` succeeds. No admitted same-loader replacement may turn an original
failure into success.

Run the classifier, projection/admission persistence, runtime loader, corpus,
and relevant #479/#484 regressions first. Then run formatting, all-target
Clippy, and the full Rust suite. Replay the frozen A/B inputs plus a new
same-loader C case into a new `/Volumes/SSD_NX/tmp` run and write only its new
comparison report under `workspace/tmp`; never modify the frozen control or
historical evidence. No runner/minimal-loop chokepoint, live `.anvil` state,
verification strength, event name, or event schema changes are required.
