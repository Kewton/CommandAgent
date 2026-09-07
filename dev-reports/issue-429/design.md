# Issue #429 reopened design

Read the full dispatch Issue JSON (including the 2026-09-07 reopen comment),
reference review, AGENTS.md, developer guardrails, existing recognizer/preflight,
and real S1/S3/E3 writer sources before edits. Inspected predecessor #420's
implementation and passing report, then fast-forwarded from `5dec1494` to
`e7ecb137249382fa50c9c889a313c3e4b09a9f6c`, including its #435 parent.

The previous implementation rejects entire files containing templates or slash
operators, and treats every function parameter as unknown. S1 directly writes
constant destinations beside templates/regex; S3 and E3 pass module constants
through a local writer helper. Extend only the static recognizer leaf modules.

- Tokenize templates and regex as opaque values, with escaped delimiters,
  character classes, nested interpolation braces/templates, and division tokens.
  Literal text never supplies imports, bindings, or writer calls. Interpolation
  identifiers conservatively invalidate bindings, so hidden executable mutation
  cannot manufacture authority. Malformed/ambiguous syntax remains fail closed.
- Recognize one level of named module-local function parameter forwarding to a
  verified Node writer. Require a simple unmodified parameter, unambiguous helper
  identity, and a complete caller argument resolving to a module constant using
  the existing literal/cwd/join evaluator. Do not recursively propagate helper
  calls or accept dynamic argument prefixes, aliases, shadowing, or mutation.
- Preserve exact normalized JSON paths and every existing cwd, configuration,
  required/protected path, source, runtime namespace, traversal, symlink, snapshot,
  observation-isolation, acceptance, and promotion check. No directory fallback.
- Copy the complete S1/S3/E3 `src` trees byte-for-byte into corpus fixtures with
  SHA-256 provenance. Exercise actual source parsing and policy registration
  before/after JSON existence. Replay isolated writes with those same source
  trees, retaining Failed business outcomes and source-mutation rejection.
  Preserve the executable synthetic first-GET fixture and existing regressions.

Run focused scanner and preflight tests, corpus and guardrail checks, then
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test`. Record final results and limits honestly. No Python harness change
is planned. No live campaign, generated-app repair, historical-evidence rewrite,
runtime namespace migration, push, PR, or external Issue mutation is in scope.

## Findings incorporated during implementation

The real S1 source additionally exposed the existing condition/parameter
confusion: `if (!existsSync(STAFF_FILE)) {` invalidated STAFF_FILE. Treat
if/while/switch headers as conditions, while retaining for/catch parameter and
assignment invalidation. Template property names such as ALLOWED_ROLES.join must
not invalidate an unrelated lexical join import. The orchestrator review also
required rejecting dynamic evaluation that can hide mutation in a string;
eval/Function/constructor markers now fail closed, with six negative fixtures.
Ambiguous postfix/TS slash contexts and JSX text remain outside recognition.
