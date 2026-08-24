# Issue 158 design

## Problem

The Gate 1 markdown renderer places each list item and paragraph into ordinary
text elements. Long unbroken values such as the selected pack hash and the
confirmation hash therefore keep their intrinsic width. The containing panel
clips overflow, so the tail of a hash is hidden at both desktop and mobile
widths. The separate confirmation-ID block already applies an `overflow-wrap`
rule and is not the source of the defect.

The dispatched predecessor tips were inspected before this design. Issue 205
changes Python CLI assurance dispatch and its corpus fixture. Issue 154 changes
documentation and documentation drift coverage on top of its prerequisites.
Neither predecessor changes the Gate 1 component, GUI stylesheet, or GUI smoke
script, and neither is an ancestor of this worktree yet.

During verification, the owner additionally required the already-passed Issue
162 reconnect fix (`551fa209`) to be integrated before the original full smoke
could be authoritative. That commit is retained intact as a separate
predecessor commit on this branch.

## Change

- Make the Gate 1 markdown root a shrinkable box and allow unbroken text within
  it to wrap at any character. This covers hashes, paths, model identifiers, and
  future plain-text values without changing the deliberately small markdown
  grammar or the card's visible content.
- Extend the existing full GUI smoke at Gate 1 to measure both the markdown
  panel and the separately rendered confirmation ID at 1440px and 390px.
  Require `scrollWidth <= clientWidth` for every measured element, record the
  dimensions in `browser-smoke.json`, and keep the existing desktop/mobile
  screenshots.
- Do not rewrite the existing tutorial screenshot or historical smoke evidence;
  those are records of earlier runs. Fresh smoke output will demonstrate the
  corrected rendering.

## Verification

Run the GUI script syntax check, lint, typecheck, and production build first.
Then run the root-and-proxy GUI smoke in its focused Gate 1 mode as additive
layout coverage. Also run the unchanged original full smoke with a release
binary rebuilt after integrating Issue 162; the focused mode does not replace
that required gate. Finally run `git diff --check`. The Issue 158 change itself
does not alter Rust behavior or shared event/corpus contracts.
