# Issue 371 design

## Goal

Make the GUI extension boundary legible as one four-layer model, while
preserving the existing bounded pack lifecycle and the exact identities that
Gate 1 and acceptance already project.

## Current surface

- `gui/app/assets/page.tsx` mixes packs, contracts, and suites as peer tabs,
  which can make reviewed reference documents look like extension kinds.
- `api/packs` already projects pack source, pins, observed hashes, warnings,
  and Trial eligibility.
- `api/trial-options` already projects registered external profiles as draft,
  with their manifest hash, base profile, and `static` assurance ceiling.
- `api/runtime-status` does not expose whether `--extension-root` is configured.
- The pack wizard already enforces stage, verify, exact-hash pin, Trial, and
  terminal retirement transitions; it must remain the Layer 3 write path.

## Design

1. Put a four-card dependency map at the top of **拡張**:
   compiled capability vocabulary (Layer 1) -> draft profile (Layer 2) ->
   pack supply (Layer 3) -> reviewed admission (Layer 4). Each card uses the
   same fields: layer, source, status, hash, assurance, and registration or
   promotion path, plus explicit allowed and forbidden behavior.
2. Extend `runtime-status.prerequisites` additively with `extension_root`.
   Report `unconfigured`, `ready`, or `action_required` without exposing the
   private absolute root path. The page shows the reason and recovery command.
3. Use the existing `trial-options` projection to list only draft profiles in
   Layer 2. Show source, draft status, exact manifest hash, `static` assurance
   ceiling, base dependency, usability, and a safe GitHub registration-Issue
   link. The GUI does not create manifests, admit profiles, or edit capability
   vocabulary.
4. Keep the existing pack wizard, catalog, and Trial handoff as Layer 3. Add a
   consistent metadata grid to each pack and make conformance failures an
   explicit unavailable reason. No supply mutation API or exact-hash behavior
   changes.
5. Move Contract and Suite documents below a **参照資料** disclosure area.
   Label them as read-only contract/measurement evidence and explicitly state
   that they are not extension types or registration paths.
6. Update the existing two-base-path smoke to assert the layer/root/profile
   information at desktop width and that the extension page fits at mobile
   width. Keep the existing pack handoff probe.

## Safety and compatibility

- The change is additive to `runtime-status`; existing fields and routes stay
  backward compatible.
- Layer 1 and Layer 4 have no GUI mutation controls. Their links open a
  repository Issue workflow and state that review, implementation, tests, and
  measured evidence are required.
- External profiles remain draft and capped at `static`; local packs remain
  unadmitted even after verification and pinning.
- Gate 1, delegated CLI arguments, acceptance projection, event schemas, and
  the live `.anvil/` namespace are unchanged.

## Verification

- Focused Rust tests for runtime extension-root state and catalog warnings.
- The GUI static contract guard, typecheck, internal-link lint, and production
  build.
- Provider-free read-only browser smoke for `/` and
  `/proxy/commandagent/`, including desktop and mobile extension views.
- Repository formatting, clippy, and full Rust tests because a shared GUI
  runtime response contract is extended.
