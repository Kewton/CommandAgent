# Issue 166 design

## Scope and predecessor

Keep the behavior change in the existing GUI pack-wizard leaf, its
provider-free browser smoke, the focused GUI source guard, and the wizard user
guide. No lifecycle API, pack schema, event, corpus, or `.anvil/` state change
is required.

Issue 165's verified commit `38b7bc17` is the direct child of this branch's
current base and is not yet integrated here. Its saved-byte re-verification
fix overlaps the same wizard and smoke flow. Fast-forward to that required
predecessor before implementing Issue 166 so the new-version path preserves
the exact bytes shown after re-verification.

## Problem

The wizard correctly makes a pinned or retired version immutable, but that
same immutable state disables every path back through target selection,
starting-point selection, and editing. The terminal pin panel tells the user
to create a new version without providing an action that does so. Continuing
therefore requires a page reload that discards the current wizard state.

## Design

Add one explicit **新しい version を作る** action for both pinned and retired
states. The transition remains entirely local until the operator uses the
existing **保存して検証** action:

1. Increment the current semantic version's patch component.
2. Preserve the pack ID and displayed members, replacing the embedded
   `pack.version` identity in assist/eval YAML with the new version.
3. Change the editor lifecycle to `draft`, clear the old verification report,
   issues, and retirement acknowledgement, and return to the editor step.
4. Leave the Trial token and target cell unchanged.

This creates an editable copy, not an overwrite or an unretire operation. The
old pinned/retired version remains immutable on the server, and the existing
stage endpoint still validates and persists the complete new member map.

## Verification

Extend the wizard-only smoke to prove that a pinned version can start and
stage its next patch version without reloading the page. Then pin and retire
that new version and prove the same action creates another editable draft from
the retired state. Retain Issue 165's displayed-byte/pinned-byte comparison.
Run the focused GUI source/doc guards and smoke, GUI syntax/type/lint/build
checks, then repository formatting, clippy, and full Rust tests required for a
production-code handoff.
