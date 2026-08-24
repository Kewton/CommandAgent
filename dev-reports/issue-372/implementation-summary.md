# Issue 372 implementation summary

## Outcome

Layer 2 now has an authenticated draft-profile registration wizard backed by
one fail-closed leaf supply boundary. Operators can preview compact manifest
v2 or an additive overlay, confirm the normalized relative path, profile ID,
exact hash, draft status, and static assurance ceiling, then create the file
without overwriting existing content. A successful save is explicitly
separate from runtime availability and reports `restart_required`.

## Implementation

- Added `profile_manifest::supply::ProfileSupplyRoot` as the only new writer
  below `<extension-root>/profiles`. It accepts only the two managed path
  shapes, rejects absolute/traversing and symlinked paths, enforces the 256 KiB
  limit, and reuses the existing manifest and overlay decoders.
- Added preview and register operations with exact-hash confirmation,
  built-in/external identity collision checks, owner-private temporary files,
  file and directory synchronization, atomic no-replace installation,
  idempotent identical bytes, and explicit conflicting-content errors.
- Added bounded, scrubbed `profile_register` journal records without changing
  the existing pack `JournalEntry` schema or recording submitted TOML,
  credentials, or private absolute paths.
- Added authenticated catalog, preview, and register GUI endpoints with Origin
  enforcement on mutations, route-local body limits, stable error codes, and
  actionable Japanese messages for authentication, Origin, validation,
  conflict, stale confirmation, and I/O failures.
- Added the Layer 2 wizard and supply catalog. The catalog projects only
  normalized extension-root-relative paths and keeps saved/unavailable state
  distinct until restart.
- Added focused Rust unit, GUI server integration, corpus, static protection,
  error-mapping, and two-base-path browser coverage. After restart, integration
  coverage confirms the draft appears in Trial and retains its exact hash and
  static ceiling in Gate 1.
- Updated English/Japanese guides, operations recovery, the manifest and pack
  contracts, GUI help ownership, and README entry points.

## Compatibility and safety

`src/planner/runner.rs`, `src/minimal_loop/loop_run.rs`, existing event schemas,
and the live `.anvil/` namespace are unchanged. The additive journal record
does not widen the existing pack record type, and registration never mutates
the process-lifetime profile registry.
