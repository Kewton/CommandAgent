# Command inventory

Exact strings are in the corpus JSON and measured report JSON. Classification
and execution are separate; every retained command still runs at its existing
step/final boundary.

| Command / origin | Target and expected result | Measured baseline → final class | Formation / fixed-contract repairability |
| --- | --- | --- | --- |
| `npm run build` / historical | Whole app; successful production build | Build → Build | Unchanged. Build/runtime inputs may be repaired; dependency/build gates remain. |
| types pure import / historical | Exact `./src/lib/types.ts`; import resolves | Weak → original remains Weak; proposed namespace refinement StaticSyntax | Only validated preclosure replacement. Saved Node/package expectation is `["default","module.exports"]`. Missing/evaluation failure and unexpected export both fail. Frozen original cannot gain evidence through app edits. |
| store pure import / historical | Exact `./src/lib/store.ts`; import resolves | Weak → original remains Weak; proposed namespace refinement StaticSyntax | Explicit sorted six-function export set. Same import-failure and unexpected-export controls. No claim about function behavior. |
| API existsSync/forEach / historical | Five exact projects/tasks/members route files; every file exists | Weak → StaticSyntax | Exact command retained. Missing any one route throws; app/file content can repair execution. No Test credit. |
| page includes / historical | Exact `src/app/page.tsx`; all three `data-anvil-action`, `data-anvil-state`, `use client` markers present | Weak → StaticSyntax | Exact command retained. Removing any predicate fails; unrelated API edits cannot satisfy the page target. No Test credit. |
| dev script / #478 host | package `scripts.dev` runs Next dev with explicit 60302 | Weak → StaticSyntax | Byte-identical command still executes on Profile owner. Existing final Profile covers only exact goal-port identity. |
| optional start script / #478 host | absent start accepted; present start must run Next start with 60302 | Weak → StaticSyntax | Same optionality, token conditions and exit 1 propagation; no newly required start script. |
| build script / #478 host | package `scripts.build` is exactly `next build` | Weak → StaticSyntax | Same string comparison and exit 1. Actual build remains a separate Build command. |
| `node smoke-check.js` / #478 model | Original smoke requirements under its exact retained Implement owner; later execution passes | Missing file: Weak; real assertion-backed artifact: Test | Missing artifact alone is not irreparable. Existing registered-owner creation and confirmation tests pass. |

The first eight classes were measured against both an empty workspace and app
source changes. Inline classification is independent of those changes. All nine
resulting step commands and the six-command final registration set were read
from the actual formed plan, including complete host guidance and original
owner order; see `formation-inventory.json`.

For fixed contracts, an immutable Weak *classification* stops app-only repair
only when the existing evidence-quality obligation actually applies. Otherwise
runtime execution remains authoritative (the #465 delegated-check control).
Under an evidence obligation, both immutable inline and frozen Weak scripts
prevent pending/discharge. Missing registered scripts and genuine Test/structure
runtime failures retain their existing repair/confirmation paths.
