# Extension catalog development

This maintainer guide covers compiled pack capabilities and profiles. Operator
supply and naming are documented in [`docs/user/gui-extensions.md`](../user/gui-extensions.md).

## Add a pack source or check

Pack YAML is declarative and may reference only registered capabilities. Never
register a free-form shell or logic-bearing template.

1. Add the typed capability implementation in the appropriate leaf module
   under `src/planner/capability_catalog/`, or use the existing pack/data/CLI
   typed enum owner.
2. Add its `CapabilitySpec` to `src/planner/capability_catalog.rs` with fixed
   `CapabilityKind`, parameter schema, and renderer/check resolver. Keep IDs and
   parameters closed; unknown values must fail.
3. If it is a guidance source, bind it through pack vocabulary/schema and a
   bounded renderer. If it is a check, bind it through the typed internal
   check executor. Do not put logic in YAML or Markdown.
4. Add the vocabulary/schema rejection tests first, then positive resolver and
   execution tests. Update `tests/golden/` when exact rendered output or a
   manifest hash is an intentional contract change.
5. Add/update the relevant corpus fixture under `tests/corpus/apps/` so the
   normal profile/intent pathway demonstrates the capability and honest
   failure. Update catalog coverage or conformance matrices rather than adding
   a bespoke bypass.
6. Run pack conformance, the focused module/test target, protection audits, and
   the shared suite. A new check may strengthen a floor; it may not weaken or
   silently replace existing verification.

Declarative Next.js knowledge belongs in
`src/planner/profiles/nextjs/knowledge.toml`; evidence knowledge belongs in
`src/minimal_loop/evidence_knowledge.toml`. Follow the scenario-matrix rule in
[`dev-guardrails.md`](dev-guardrails.md) before adopting either change.

## Register a `ProfileDescriptor`

`src/planner/profile_descriptor.rs` is the single compiled registry used by
CLI inference, GUI options, pack compatibility, contracts, and measurement
keys.

1. Implement the profile in a leaf module under `src/planner/profiles/` and
   keep runner chokepoints to wiring only.
2. Add exactly one `ProfileDescriptor` with canonical ID, non-conflicting
   aliases, Japanese display/description, admission function, runtime/domain
   owners, contract reference, optional band key, and optional `PackProfile`.
3. Register the matching pack-profile enum only when the profile truly owns a
   closed pack vocabulary. Absence is preferable to a false compatibility.
4. Add the shared conformance-matrix row, inference/alias tests, GUI option
   projection test, and focused domain proof. Admission and measured band must
   be supported by their own evidence.

External `manifest.toml` and additive `overlay.toml` descriptors remain draft,
hash-pinned, and capped at static assurance. They do not modify the compiled
registry or self-admit.

## Update guards without weakening them

- Register every new execution/write boundary in
  `tests/protection_coverage_audit.rs` before use, naming its required wrapper
  and exact allowlist.
- Update `tests/generality_guardrails.rs` only for intentional source movement
  that lowers a reviewed baseline. Never raise a budget to admit growth.
- Keep event names/schemas backward compatible unless a separately authorized
  migration changes them.
- Add doc-drift coverage for public CLI/config/help surfaces. Maintain EN/JA
  files and H2/H3 counts together.
- Do not rewrite historical run or migration evidence to make a new catalog
  entry appear established.

## Use `PackLocator`

`planner::pack::catalog::PackLocator` is the read/selection boundary. Construct
it with the repository root and optionally configure the explicit extension
root. It resolves exact `id@version`, applies extension-root precedence, checks
retirement and exact-byte pins, validates profile/intent compatibility, and
returns a `LocatedPack` with the winning source.

Use it for listing and run selection. Do not recreate directory precedence or
hash parsing in CLI, REPL, GUI, or profile code. A repository file's presence
alone is not admission, and a local match must retain its local source/warning.

## Use `SupplyRoot`

`planner::pack::supply::SupplyRoot` is the only write boundary below an
extension root. It validates root permissions/separation, stages bounded
members atomically, verifies schema/vocabulary/floor/path/scrub contracts,
creates a first pin only for matching bytes, retires without deletion, and
appends scrubbed journal records.

GUI/CLI adapters may validate HTTP/argument shape and authorization, then call
`SupplyRoot`; they must not write pack files, pins, retirement markers, or
`journal.jsonl` themselves. Preserve its typed `SupplyError` distinctions and
failure atomicity. Tests must cover path traversal, symlink, size/count, stale
pin, retirement, journal scrubbing, and concurrent/conflicting operations.

Read-only catalog projection uses `PackLocator`; lifecycle mutation uses
`SupplyRoot`. Keeping those owners separate prevents an inspection endpoint
from becoming an unreviewed write path.
