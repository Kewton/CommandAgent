# Next.js Create Profile Contract

Status: **draft for review (2026-07-31)**

This is a retrospective contract draft for the admitted `nextjs × create`
profile. Sections 1–5 document the implementation as it exists; they do not
change its checks, evidence, assurance, prompts, or capability band. Sections
6–7 propose a new testimony gate and its measurement labeling. Neither
proposal is authorized for implementation until review adjudicates this
draft.

## 1. Scope and deliverables

The profile generates a runnable Next.js application with one route-bound
task implementation. The canonical create path is an App Router scaffold in
the workspace root, consisting of `package.json`, TypeScript and styling
configuration, `src/app/layout.tsx`, `src/app/page.tsx`,
`src/app/globals.css`, and `src/app/global.d.ts`. Exactly one supported
`tailwind.config.*` path is selected when the Tailwind stack is used.

`package.json` must retain coherent `next`, `react`, and `react-dom`
dependencies, `scripts.build = "next build"`, and deterministic `dev` and
optional `start` scripts on the requested port (3011 when no port is
declared). The task-specific implementation must be connected to an actual
Next.js route; an unimported component, scaffold alone, package metadata
alone, or a build-only shell is not a completed application.

The manifest is currently admitted and its create plan has four phases:
project setup, core implementation, contract wiring, and build verification.
The final phase is verification-only.

## 2. Existing meaning of `full`

`full` means that the production build boundary and route-bound implementation
are established and, for a browser-interactive contract, a real browser probe
has rendered the route, performed the required interaction, and recorded an
observable application-state change, with every bound path, capability,
evidence item, obligation, external completion contract, and release gate
passing.

The browser clauses are activated by the existing goal/capability inference.
A non-interactive static goal does not currently make browser interaction
applicable. This conditional is existing behavior, not a proposed relaxation.

## 3. Existing checks and required evidence

### 3.1 Profile and build checks

The profile manifest currently binds these registered checks:

| Phase | Existing check | Existing purpose |
|---|---|---|
| project setup | `package_json_port_script` | The development command uses the bound port. |
| project setup | `package_json_script_matches` | The package build script is `next build`. |
| project setup | `scaffold_files_present` | The canonical scaffold paths exist. |
| contract wiring | `hook_attribute_present` | The route page exposes the primary, restart, and state hooks required by the current manifest. |
| build verification | `next_build_verify` | The production Next.js build succeeds. |

Final profile verification also checks dependency coherence, the selected
entrypoint and App Router layout, TypeScript aliases and compiler settings,
CSS side-effect imports, relative-import closure, client-component
requirements, and styling-toolchain coherence. The acceptance contract binds
`build_command_or_dependency_missing_boundary`; a missing dependency boundary
is an honest failure condition, not a substitute for a successful delivery.

### 3.2 Route and implementation evidence

`nextjs_route_evidence` requires a non-empty recognized route entrypoint in
the route-bound source closure. Application-like goals also bind the
`implementation` obligation, which scaffold, setup, style, verification, and
acceptance-evidence files cannot satisfy by themselves.

Capabilities inferred from the goal add their existing source and runtime
evidence. Examples include:

- stateful interaction: implementation, visible surface, input handler,
  state update, interactive UI, and non-static screen evidence;
- start or restart flow: an implementation and visible/input surface plus
  restart or recoverable-state evidence;
- player/user control, challenge, progression/score, failure/collision, and
  persistence: the corresponding route-bound source evidence.

This source evidence is a prerequisite, not a replacement for an applicable
browser probe.

### 3.3 Browser readiness and interaction evidence

For an applicable interactive contract, the release gate consumes actual
`browser-readiness.json` and `browser-interaction.json` observations (including
their supported historical filenames). Readiness passes only when the route
was rendered, rather than from HTTP success alone.

Interaction passes only when the probe performed an input and observed state
change after a start transition, or observed an input-driven state change on
a visible surface that has no start control. The evidence records such fields
as action hooks, state dimensions changed, input events, transition state,
and the evidence path. Missing detail is unavailable/inconclusive; an
explicit missing surface, input event, transition, or state change is a
failure.

## 4. Existing assurance and honest failure

The runtime begins from the admitted Next.js profile's `full` base, but does
not award it directly. Final assurance is earned only after:

1. all expected paths exist;
2. profile invariant and final verification pass;
3. the generated or supplied completion contract is bound and passes;
4. runtime capability, evidence, and implementation obligations pass; and
5. every applicable browser readiness and interaction gate is connected and
   passes.

Disconnected, skipped, or `not_applicable` browser gates on an interactive
contract cannot produce `full`; the `acceptance_gates_disconnected` guard
makes that mismatch loud. Failed build, route, runtime, browser, interaction,
or profile-behavior evidence fails final acceptance. Unavailable or
unexercised evidence remains partial/static according to the existing release
projection and cannot be promoted by prose or by scaffold presence.

## 5. Existing machine assistance and permanent exclusions

The current capability price includes machine assistance. The profile owns a
deterministic four-phase preset, pre-provisions missing canonical scaffold
files without overwriting existing files, supplies registered literal hook
guidance (`data-anvil-action` and `data-anvil-state`), fixes build/port verify
commands, discovers the route-bound import closure, and supplies bounded
browser-probe and repair guidance. A measured `full` therefore means
“model plus this declared machine scaffold and verifier,” not unaided model
generation.

The following remain permanently outside current `full`:

- visual polish or aesthetic quality;
- UX judgment, accessibility quality beyond mechanically bound checks, and
  product usefulness;
- performance, load, scalability, and production operations;
- exhaustive browser/device compatibility;
- truthfulness of README or response-document feature testimony (the gap
  proposed in section 6).

## 6. Proposed testimony gate (new; not implemented)

### 6.1 T1 `testimony_binding`

T1 would extract functional claims from `README.md` or a goal-response
document delivered with the application and compare each claim with the
route-bound implementation and observed behavior. The binding anchor is
execution observation: the existing browser readiness, interaction, action
hook, state-dimension, and route evidence. Source inspection may locate the
implementation or explain a nearest miss, but source text alone cannot prove
a behavioral claim.

Each proposed T1 result would preserve, per claim:

- the exact claim and source file/location;
- the claimed action, state, route, or outcome;
- the browser/interaction evidence and route-bound source references used for
  comparison;
- `matched`, `claims_absent`, or a typed violation result; and
- a bounded `nearest_miss` when no observation satisfies the claim.

A claim that an action works, a score changes, data persists, a retry resets
state, or a named route is connected must be matched to the corresponding
observed action/state/route evidence. A claim must not pass because a similarly
named function or static string exists.

### 6.2 Required implementation order

No registered Rust check ID for this Next.js testimony comparator exists as of
this draft. `T1` is a contract label, not an ID that may be placed in
`eval.yaml`.

The required order is:

1. implement the Rust extractor/comparator and evidence shape;
2. register its typed check/violation vocabulary and add real measured
   fixtures plus conformance;
3. connect it to the production Next.js acceptance path and assurance
   projection; then
4. only after those gates are green, reference the final registered ID from a
   reviewed eval pack.

This order follows the pack institution rule: YAML composes registered
verification; it does not create a verifier by spelling a new ID.

## 7. Proposed value-label and band migration (new; not implemented)

Adding T1 can only retain or lower the reported Next.js `full` rate for the
same artifacts. That is not a capability regression. It is a stricter,
symmetric test that restores comparability with CLI C3 by asking both profiles
to bind product testimony to observed behavior.

The P-2 band design should therefore add an explicit **Full meaning** label to
every Next.js band row:

| Label | Meaning |
|---|---|
| `nextjs-create/v0: build+browser+state+route` | Historical/current contract documented in sections 1–5. |
| `nextjs-create/v1: v0+T1-testimony` | Future window after T1 Rust registration, production wiring, conformance, and review admission. |

Historical rows and evidence remain immutable and keep the v0 label. A v1
window starts only after the implementation commit and its effective pack
ID/hash, if any, are pinned. The band must not compare v0 and v1 percentages
without showing these labels, because their denominators have different
meanings of `full`.

Review must adjudicate T1 extraction scope, the treatment of
`claims_absent`, violation vocabulary, evidence filename/schema, assurance
projection, conformance cases, and the exact v1 window before implementation.
