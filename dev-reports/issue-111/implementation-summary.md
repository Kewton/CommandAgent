# Issue #111 Implementation Summary

## Delivered behavior

- Integrated the committed Issue #107 through #110 dependency stack so the
  REPL uses the established profile descriptors, admitted pack catalog,
  exact-byte selection contract, runtime environment guard, and direct pack
  actions.
- Added plain-request `--pack <id@version>` parsing. The selector is removed
  from the request body, resolved only against compatible admitted catalog
  entries, and frozen into the Gate 1 identity.
- Added `/packs` to the slash registry. Both the direct `--packs` action and
  the REPL call the same renderer, preserving headers, hashes, source labels,
  ordering, filtering, and errors. In an active boundary session the REPL uses
  the confirmed profile and intent.
- Added `/pack <id@version>` for Gate 4. It selects the typed `pack_change`
  action, retains the failed request/route/model pins, and emits a fresh Gate 1
  proposal with a new confirmation hash. No execution occurs before the new
  `/confirm <hash>`.
- Confirmed dispatch revalidates the pack bytes, installs the frozen pack with
  the existing scoped runtime environment guard, restores the prior process
  environment afterward, and emits the additive schema-v1 `pack_injected`
  event. Confirmed directive continuations receive the same pack environment.
- Gate 4 advertises `pack_change` only when a different compatible admitted
  pack exists and gives the executable `/pack <id@version>` instruction.

## Tests and contract updates

- Added inline selector, catalog selection, shared `/packs` rendering, Gate 4
  transition/card, runtime environment, restoration, and event assertions.
- Added the `issue111-repl-pack-selection` corpus fixture for the confirmed
  route followed by `pack_injected` event ordering and fields.
- Updated the English/Japanese slash-command guides, first-loop guide, D-3c
  design, and integration status. Existing confirmation and event schemas were
  not weakened or migrated.

## Files of note

- `src/tui/slash.rs`: registry, inline parser, and `/packs` dispatch.
- `src/tui/repl.rs`: Gate 1 selection, Gate 4 switching, and confirmed runtime
  installation.
- `src/tui/boundary_shell/{mod.rs,pack_catalog.rs}`: exact admitted selection
  and pack-change state transition.
- `src/pack_actions.rs`: shared deterministic list renderer.
