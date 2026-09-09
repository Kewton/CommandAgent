# Issue #457 design

Written before implementation on 2026-09-09. Read the full GitHub Issue, AGENTS.md,
the issue-worker skill and development guardrails. Inspected #456's committed
implementation and passed verification at `32f997f4e98523df3659fadb408994afbe14de53`;
fast-forwarded this dedicated branch from `222feef7` without conflicts.

Read-only evidence root: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`.
Read the four specified 0909 investigation/review documents, #451/#452 cause analyses,
the archived source-equivalence manifest and compiler reproduction. The #456 corpus
contains the exact archived source/config files; reuse it as the immutable base and
add #457 overlays. Copy the archive's matching package lock for reproducible checks.

The original goal requests project creation, due-dated tasks, team assignment and
status filtering. The archived UI reads `members` from GET `/api/projects`, selects
member IDs and sends `assignee: Member | null`; types, task creation and store already
use Member objects. PATCH instead reads `assigneeId`. No directory supply exists.
For the repaired fixture, provide a documented, static sample team through the
existing projects GET (no invented user directory integration). Keep IDs in select
values, resolve to complete Member objects on the wire and in storage, use null for
unassignment and omission for no update, and return the stored task as `{item}`.
The existing JSON store is retained for this archived-source fixture only. This is
a fixture decision supported by the goal and majority of saved contracts, not a new
R0 JSON persistence requirement or evidence of live model repair success.

Observed product gaps: Next build stops at one diagnostic, extraction is capped at
five, compact compile context has no cross-file contract summary, and response-shape
inspection recognizes direct fetch/JSON bindings only. The archive uses a local
checked fetch helper and a guarded collection read; update request keys are never
compared. Existing generation rules already demand shared types, consistent JSON,
Response.ok and reload. Retain those rules rather than add application names.

Implement leaf modules to retain supplementary whole-project diagnostics from the
installed TypeScript toolchain after a Next.js type failure, map diagnostics to
related shared definitions in repair guidance, and conservatively inspect proven
local fetch wrappers and literal update payloads. Keep unknown/dynamic forms runtime
obligations and distinguish optional feature checks from consumed collections.
Preserve original build failure, verification commands, protected paths, event
schema, acceptance and promotion gates; no tool download or type-check bypass.

Add corpus cases for original (33 diagnostics), role-only (32), coherent types with
missing directory, coherent types with wrong update key, and fully aligned contracts.
Run identical strict type checks/builds and execute the real page/API/store flow for
list, selection, assignment, reload and unassignment. Include negative static controls
for indirect/dynamic code and renamed-domain positive controls. Use deterministic
fixtures to verify support; live model improvement remains #452.

Required verification: focused Rust and corpus tests; fixture type/build/interaction
matrix and focused harness tests if added; Ruff for Python; cargo fmt, clippy all
targets with warnings denied, cargo test, release binary build/version and diff check.
Write implementation-summary.md and exact-status verification.md, then commit only
#457 paths. Push/PR/merge/Issue state changes return to the parent.

## Refinements from reproduction and parent review

- A guarded `Array.isArray(data.members)` read is optional even when it feeds
  useState: a local default or another source can supply the directory. Keep this
  pattern advisory only. The saved fixture's absent directory fails the browser
  feature oracle, not generic static acceptance. Ten type-valid ambiguity controls
  include local defaults, helper parameter/destructured/arrow shadowing, direct and
  computed request access, property guards, and optional request defaults.
- Hard request mismatch requires disjoint literal payload/read keys and every
  observed 2xx return behind a recognized key-presence guard. Aliased destructuring
  uses its local binding for guards and wire key for comparison. Additional body
  use, unknown returns, helper transforms and ambiguous bindings stop inference.
- Separate internal `StoreError {code,message,details?,status?}` from HTTP
  `{error: string, details?}`. Keep `ApiResponse.error` as a string; routes map store
  messages onto that field. The original UI helper remains valid. Corrupt-file
  probes must exercise every route's string error and 5xx response, visible UI
  error rendering, and no overwrite of damaged or related files.
- Preserve the existing store's Promise semantics using
  `async withMutex<T>(fn: () => T): Promise<Awaited<T>>`. This models await's
  flattening without weakening types or asserting a runtime double-Promise bug.
- The projects GET must opt out of Next's static route caching for reload to read
  the existing mutable store. `force-dynamic` is confined to repaired overlays.
- Existing per-error definition heuristics do not map generic/union types in this
  archive to all shared files. Add an import-to-declaration-file inspection map
  from diagnosed sources, with canonical workspace/path-policy checks; imported
  file contents are not copied by this map. These relationships are repair leads,
  not asserted independent root causes or extra edit authority.
- Browser verification follows the current develop-root AGENTS reliability rules
  and the current-session Browser skill. Its client/service entries exist. Use the
  explicitly authorized standalone Playwright fallback with temporary profiles,
  own fixture processes and OS-assigned loopback ports; record settings, page,
  readable state, harmless input and screenshots. No plugin repair is claimed.
- The new request-key check belongs to final contract verification and repair
  guidance. Applying it to every intermediate invariant reproduced a #456 failure:
  immediately after read-only inspection, the existing invariant-repair path added
  next.config.js for the still-unrepaired application. Keep the pre-existing
  intermediate invariants unchanged and evaluate this additional functional
  contract at final verification. The exact #456 source-identity/reachability test
  must still pass; no snapshot assertion or final gate is relaxed.
