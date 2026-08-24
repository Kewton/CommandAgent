# Issue 372 design

## Goal

Allow an operator to validate and register one external draft profile from
Layer 2 without giving the GUI an unconstrained filesystem write primitive.
The saved bytes remain draft, exact-hash identified, and unavailable to the
running process until an explicit restart reloads the extension root.

## Existing boundaries

- `profile_manifest::source` already parses compact manifest v2, rejects
  unknown fields and capabilities, bounds files, and forces external status to
  `draft`.
- `profile_manifest::overlay` already validates the additive-only overlay
  contract and admitted embedded base identities.
- `extension_profiles` already projects registered profiles into Trial, Gate 1,
  acceptance, and the `static` admission ceiling.
- `gui_server::trial_access` already supplies bearer-token and exact-Origin
  checks, while the pack supply boundary establishes private-root, journal,
  and atomic-write conventions.

## Design

1. Add `profile_manifest::supply` as the only new write boundary below
   `<extension-root>/profiles`. It accepts only the exact relative shapes
   `profiles/<id>/manifest.toml` and `profiles/<admitted-base>/overlay.toml`,
   rejects absolute/parent paths and every symlinked managed component, and
   applies a 256 KiB document limit before parsing.
2. Preview bytes entirely in memory with the existing manifest/overlay
   decoders. Return the effective profile id, normalized relative destination,
   exact-byte hash, local source, `draft` status, `static` ceiling, optional
   base profile, and warnings. Reject built-in and already supplied identities
   fail-closed.
3. Require registration to repeat the document and the previewed exact hash.
   Re-validate and re-hash it, then write an owner-only create-new temporary
   file, sync it, and install it without replacement. Existing identical bytes
   return an idempotent success; different bytes return a conflict. Empty
   directories may be created, but no partial manifest becomes visible.
4. Append bounded, credential-scrubbed `profile_register` success or failure
   records to the existing extension journal using an additive profile record
   shape. Existing pack `JournalEntry` records and their closed schema remain
   byte-compatible.
5. Add authenticated GUI endpoints for catalog, preview, and register.
   Preview/register POSTs require both the existing bearer policy and an
   allowed Origin and have a route-local body limit. Map authentication,
   Origin, body, validation, conflict, and I/O failures to stable profile error
   codes with actionable Japanese messages.
6. Add a Layer 2 wizard only when `extension_root` is ready. The wizard edits a
   compact v2 manifest or additive overlay, previews the identity/path/hash and
   draft/static boundary, requires explicit confirmation, and reports saved
   bytes separately from `restart_required`. A live disk catalog shows source,
   status, hash, and current runtime availability.

## Compatibility and safety

- The runner and minimal-loop chokepoints are untouched. No event schema or
  `.anvil/` runtime path changes.
- Registration never calls the process-lifetime profile registry. The response
  therefore always states `restart_required`; only normal server/CLI startup
  exposes the saved exact hash to Trial, Gate 1, and acceptance.
- The request does not accept an arbitrary absolute destination. The displayed
  path is a normalized extension-root-relative path, so private root details
  and document content never enter the journal or public catalog.
- The extension write protection audit is extended to recognize this one new
  leaf boundary; GUI handlers remain free of direct filesystem writes.

## Verification

- Focused Rust unit tests for path/symlink, size/schema/capability/overlay,
  built-in/external collisions, idempotency, no-clobber atomicity, and journal
  scrubbing.
- GUI server integration tests for disabled root, auth, Origin, body limit,
  preview/register/error mapping, catalog projection, and restart semantics.
- GUI typecheck, lint, build, static protection guards, and both-base-path
  browser smoke.
- Repository format, clippy (default and `gui`), protection/doc guards, and the
  full Rust suite because shared extension-root contracts are touched.
