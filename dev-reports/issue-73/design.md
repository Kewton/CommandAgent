# Issue #73 Design

## Scope

- Keep `ConfirmationIdentity`, its serialized bytes and card hash, the Trial
  API routes/payload fields, explicit checkbox, and exact-hash dispatch check
  unchanged.
- Rewrite only the Gate 1 presentation markdown in Japanese so it explains the
  requested work, each required contract check, comparable-run result, write
  boundary, model pins, and confirmation ID in reader-facing language. Python
  CLI C1-C4 descriptions stay semantically aligned with
  `docs/cli-profile-contract.md`; other known create-profile checks receive
  descriptions as well.
- Render the returned `card_markdown` in the Trial page through a small safe
  leaf component with `data-testid="gate-one-card-markdown"`. Keep time/cost,
  the explicit write boundary, and confirmation controls beside it, but remove
  duplicated route/check and internal measurement labels.
- Make the Terminal heading a Gate 3/Gate 4 result summary. Show assurance only
  as a human explanation below it, and replace D-3d/scrub/persist labels with
  plain follow-up wording without changing directive APIs or behavior.

## Compatibility

The Issue branch was integrated with the current `develop` baseline after
Issues #63, #64, #66-#72, #76, #77, and #80 were merged. Conflict resolution
keeps their lease, option, phase, feedback, artifact, session-index,
localization/runtime-status, polling, and shared coded-error behavior while
adding only the Issue #73 presentation and smoke assertions. The markdown
renderer remains a leaf component.

Issue #76 fixes GUI-owned language to Japanese without i18n. Every
reader-facing string added or replaced by this Issue follows that integrated
decision. Opaque profile/provider values, hashes, filesystem paths,
contract/API references, and internal event/status identifiers remain
unchanged.

## Tests and verification

- Extend Gate 1 presentation tests for described C1-C4 wording and preservation
  of the confirmation ID/instruction.
- Extend the GUI source guard and two-base-path Playwright smoke to require the
  markdown test ID, Japanese check descriptions and visible copy, and a
  Terminal heading that cannot equal assurance.
- Run focused Rust/GUI checks first, then formatting, Clippy, and the full Rust
  suite because shared TUI presentation and browser smoke contracts change.
