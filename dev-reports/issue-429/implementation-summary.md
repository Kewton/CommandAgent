# Issue #429 reopened implementation

Recovery now recognizes the real S1/S3/E3 JSON destinations before the files
exist and when existing files are legitimately updated. The complete unchanged
source trees are in the corpus, with SHA-256 provenance. S1/S3 register only
`data/shifts.json` and `data/staff.json`; E3 registers only
`data/departments.json` and `data/expenses.json`.

## Recognition

The lexer lives in a new leaf module. It treats template/regex text as opaque,
handles escaped delimiters, character classes, nested interpolation braces and
templates, and accepts ordinary division in known expression contexts. Words
referenced by executable interpolation invalidate trusted bindings; property
names such as ALLOWED_ROLES.join are not lexical references to the imported
join function. Opaque regex words are also conservatively invalidated, so a
slash ambiguity cannot conceal a trusted identity assignment. Ambiguous slash
contexts (including a newline before slash and automatic-semicolon insertion),
JSX/type-assertion ambiguity, and escaped executable identifiers are
rejected. eval/Function/constructor markers in executable tokens or quoted
selectors, including interpolation and ordinary aliases, reject the source.

A new forwarding leaf recognizes a named module-level helper with simple typed
or untyped parameters (including E3's single generic type parameter). Every use
of its path parameter must be the complete first argument of a verified Node
writer directly in that helper body. The helper identity must remain
unambiguous. Callers supply an immutable module constant resolved by the existing
literal/cwd/join evaluator; forwarding is exactly one level. Dynamic arguments,
aliases, shadowing, reassignment, and nested forwarding grant nothing.

S1 also exposed an existing false rejection: the parameter heuristic treated
`if (!existsSync(STAFF_FILE)) { ... }` as a parameter declaration and invalidated
STAFF_FILE. Conditions in if/while/switch no longer create parameter bindings.
Assignments within conditions, for-loop bindings, catch parameters, and actual
function parameters remain protected.

No observation, hash, acceptance, promotion, or event production code changed.
The existing allowed_generated_paths array receives exact normalized files.
All path normalization, cwd, configuration, required/protected path, source,
hidden/runtime namespace, traversal, and symlink gates remain in place. No
whole-directory authority or growth-baseline change was introduced.

## Tests and limits

Seven reopened focused tests cover unchanged real-source registration and
hashes, harmless literals/division, hidden executable mutations, six dynamic
evaluation fixtures, helper forwarding, control conditions, and isolated
preflight effects. The latter covers 24 combinations (three source trees,
missing/existing outputs, pass/business-fail/source-change/unknown-output).
Business failure reaches Recovery as Failed; success and Unavailable do not
start a repair. All original-session hashes and data remain unchanged.
Existing synthetic first-GET, source/config mutation, symlink, required/protected
path, exact-file hashing, cwd, and weak-authority regressions are retained.

This remains a deliberately bounded static recognizer, not a general JS/TS
interpreter. Unknown or ambiguous constructs may conservatively lose grants;
interpolation outputs, arbitrary dynamic callees, cross-module path propagation,
helper aliases, multi-level forwarding, and dynamic output paths are unsupported.
The real-source preflight observer replays synthetic effects; it does not run
those Next.js apps. The original plain-JS synthetic GET fixture still executes
its own lazy writers. No new build/browser/campaign or domain-success claim is
made for the historically failing generated apps.

Predecessor #420's passing commit `e7ecb137249382fa50c9c889a313c3e4b09a9f6c`
(including #435) was inspected and incorporated by fast-forward before edits.
No Python harness changed. Historical evidence, original generated sources,
credentials, and the live .anvil namespace remain unchanged. No push, PR,
CommandMate operation, or Issue lifecycle mutation was performed.
