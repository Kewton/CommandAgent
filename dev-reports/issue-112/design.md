# Issue #112 Design

## Goal

Add admitted pack selection to GUI Trial without creating a second pack
catalog or allowing ambient GUI-server environment variables to influence a
delegated run. The selected exact-byte identity must remain stable across the
Gate 1 proposal, confirmation, delegated CLI, acceptance sheet, and session
history.

## Predecessor integration

- Integrate the Issue #107–#111 pack stack, which owns profile descriptors,
  the admitted catalog, exact `PackSelection` pins, Gate 1/sheet presentation,
  CLI pack selection, and runtime injection.
- Integrate Issue #106's split GUI Trial modules and its `env_clear()` child
  boundary before adding pack-specific delegation.
- Integrate Issue #118's Trial hook/component/API split so the new UI behavior
  lands in the current module owners instead of regrowing `try/page.tsx`.
- Issue #105 is documentation-only background for profile overlays and does not
  need production wiring in this issue.

## Backend contract

- Add an unauthenticated, read-only `/api/pack-options` endpoint backed directly
  by the admitted planner catalog. Each option exposes the selector identity,
  compatible profile/intent, exact hash and injection point, plus the closed
  source value and Japanese source label.
- Extend GUI Trial requests with an optional exact `id@version` selector. After
  deterministic routing, resolve it through the shared boundary-shell catalog;
  clients never provide the hash, point, or source. Freeze the resulting
  `PackSelection` into `ConfirmationIdentity`, so changing the selector changes
  `card_hash` and an older confirmation receives the existing 412 stale error.
- Render Gate 1 and the acceptance sheet using the shared `PackLocator` and
  persisted identity. Extend session summaries by reading the bounded,
  immutable confirmation record and projecting the same pack identity.
- Keep `env_clear()`. For a pinned identity, validate its admitted location and
  observed bytes, pass `--pack` and `--pack-hash`, and set all four
  `COMMANDAGENT_PACK_*` values from that frozen identity/location only. For no
  pack, set none of them. Apply the same construction to initial and
  continuation children.

## Frontend contract

- Load pack options through the extracted Trial API module. Show compatible
  admitted entries in the compose form, including `cli-assist@1.0.0` and
  `cli-assist@1.1.0` for `python-cli × create`, with an explicit no-pack choice.
- Treat the selector as launch identity: changing it clears the proposal and
  confirmation just like other Gate 1 fields. Display the frozen pack and
  supply source in the Gate 1 confirmation area.
- Add a pack column to Trial history using the server-projected persisted pin.

## Verification

- Add focused GUI-server integration assertions for option enumeration, Gate 1
  rendering, hash staleness, persisted history/sheet identity, and child
  environment provenance.
- Update structural/read-only guards for the new endpoint and extracted UI
  owners without weakening their mutation or process-boundary checks.
- Run focused Rust tests and GUI smoke checks first, then formatting, Clippy,
  full Rust tests, GUI typecheck, lint, and production build.
