# Issue #849 fixtures

Read-only source: `<user>/share/work/gen-eval-1022/campaign-G1-3a5/stage3a/tool-q/work/smoke-check.js`
(L-107, campaign G1 stage 3a, built from develop `8261861c`).
The original is 135 lines and has sha256
`8e8d0575ad13977faaa36d4cce78c6fb701e905b8d6d1cbd0ca1d711f3888ce3`.

`smoke-check.js` here is a short rewrite, not a copy. It keeps the four things
that mattered: a top-level `const assert = require("assert");`, a top-level
`assert.ok(...)`, a regex literal whose character class holds a `"` (the quote
opens a string that never closes, as at original line 78), and a template
literal (as at original line 12). The recognizer therefore refuses it with
`unterminated_quote_or_comment,template_literal` while a human sees an
asserting script. The verdict stays `node_smoke_without_assertion`; only the
reason is new. The original paths, page sources and secrets are not included.

The Rust test `rejection_tests.rs` reads this directory and its `-fixed`
sibling: it checks the first refusal, the final-acceptance repair line built from
it, and that the sibling rewritten into the shape the line names is accepted.
