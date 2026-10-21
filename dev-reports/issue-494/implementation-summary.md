# Issue #494 implementation summary

## Outcome

Preclosure verifier formation now preserves the supported CommonJS loader
semantics for the exact weak original
`node -p "require('./src/lib/types.ts')"`. The classifier extracts only that
local target and continues to classify the original as weak evidence. Formation
accepts a closed `require()`-based export-set assertion only when target,
expectation, complete scope, owner, and owner order are preserved.

Dynamic-import formation remains loader-specific. A CommonJS original cannot be
replaced by the prior dynamic-import diagnostic control without an equivalence
proof. The saved source-text proposal remains rejected, while the accepted
same-loader command retains the original printed value and propagates load and
module-evaluation failures.

## Changes

- Added exact CommonJS original and strengthened-command grammars in the import
  classification leaf. The grammar rejects alternate executables or flags,
  extra argv, dynamic/nonlocal targets, trailing JavaScript, shell composition,
  catches, and success/exit overrides.
- Made verifier projection choose the replacement grammar from the original
  loader and continue using the existing preservation proof for scope,
  expectation, ownership, and ordering.
- Added loader-specific repair guidance. CommonJS guidance names only the
  same-loader export-set shape; dynamic-import guidance remains limited to
  dynamic-import originals.
- Added test-only admission tracing for the successful formation boundaries.
  It correlates the attempt, original command, target, expected result,
  replacement, and formed command without changing product events or schemas.
- Added focused classifier, projection, admission, persistence, and Node runtime
  tests. Node and package/module settings are pinned, including an ESM graph
  with top-level `await` that demonstrates the loader difference.
- Added the hash-bound
  `tests/corpus/apps/issue494-require-formation` fixture with the saved input,
  positive and negative cases, and frozen configured/Recovery contract bytes.

No production changes were made to the planner runner, minimal-loop runner,
live `.anvil` state, acceptance strength, product event names, or event schemas.

## Frozen replay

The authoritative `-02` control was left unchanged. A fresh copy-on-write
`-03` replay confirmed that saved proposal A is still rejected for source-text
inspection, diagnostic proposal B is still rejected as a cross-loader
replacement, and same-loader proposal C clears the original `types.ts`
correspondence before the unchanged independent `store.ts` owner-scope
rejection. The disposable replay report is at
`workspace/tmp/issue-494-l2-replay-20260919-03.md` and is intentionally not
part of the commit.
