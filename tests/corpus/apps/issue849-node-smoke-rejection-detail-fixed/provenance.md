# Issue #849 control fixture

Control for `issue849-node-smoke-rejection-detail`, whose provenance describes
the source (L-107 tool-q, develop `8261861c`, original sha256
`8e8d0575ad13977faaa36d4cce78c6fb701e905b8d6d1cbd0ca1d711f3888ce3`).

`smoke-check.js` is the same script rewritten as the repair guidance asks: the
regex literal becomes `includes`, the template literal becomes a concatenation,
and the top-level binding and `assert.ok(...)` stay. The recognizer accepts it,
so acceptance passes and no weak evidence is reported. This pins that following
the guidance is enough.
