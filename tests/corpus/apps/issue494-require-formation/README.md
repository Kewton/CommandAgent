# Issue #494 CommonJS verifier formation

`cases.json` binds the exact retained `node -p require()` command, saved
source-text proposal A, diagnostic dynamic-import proposal B, same-loader
positive, grammar/scope negatives, and hashes published by Issue #494. The
frozen `-02` run remains read-only outside the repository.

The accepted command retains CommonJS `require()` and `node -p` output while
adding an explicit sorted runtime export-set assertion. It is structural
evidence only. `module.cjs` and the root package pin the ordinary CommonJS
runtime; `esm/entry.mjs` pins the top-level-await loader difference. Tests
require Node v24.1.0 and do not infer an export expectation from source text.

`frozen-contract.json` is the byte-identical generated contract from both
saved A/B controls (SHA256
`eb2b04647a28ab10508c21e9c1cd026bea293a1c7f95dda261f4bd5529d8c9ab`).
Configured and Recovery tests prove its command list and bytes remain unchanged.
