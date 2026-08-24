# Issue 122 implementation summary

## Outcome

Reorganized the CommandAgent documentation by reader without changing product
behavior, runtime state, event schemas, or acceptance semantics.

## Changes

- Added CLI and GUI onboarding at
  `docs/user/getting-started-{cli,gui}.md`, including provider/config/doctor,
  first-loop, exact-pack A/B, sample Trial, Gate 1, result reading, and the
  required glossary.
- Split the former monolithic GUI guide into Trial, history, extensions, setup,
  and operations owners. `docs/user/gui.md` is now a stable compatibility index
  that retains every former H2/H3 anchor and links to the live destination.
- Added `docs/dev/extension-catalog.md` for typed source/check registration,
  `ProfileDescriptor`, guard/golden/corpus updates, `PackLocator`, and
  `SupplyRoot` ownership.
- Added `docs/user/gui-help-map.md` with one owner for each selected explanation,
  term-help, empty-state, Gate primer, and action string. The GUI smoke validates
  source/map cardinality and live onboarding/action copy on both supported base
  paths.
- Routed both README Quickstarts directly to CLI, GUI, and extension layers;
  updated the documentation, guide, profile, pack, changelog, and mechanism
  indexes in EN/JA where paired content is present.
- Extended `tests/doc_drift.rs` to require the reader-document set, validate
  every new local Markdown file/fragment link, retain legacy GUI anchors, bind
  the help map to app source/smoke, and enforce three-layer bilingual
  Quickstarts. Updated the GUI lease-recovery guard to its new document owner.

## Compatibility and safety

- Public CLI flags, configuration keys, slash commands, and EN/JA guide file
  and H2/H3 parity remain checked by the existing drift contracts.
- GUI Trial remains a confirmed product-CLI delegation; no provider/runner or
  new write boundary was added.
- Existing `workspace/management/runs/`, `docs/migration/`, and `.anvil/`
  records/namespaces were not changed.
