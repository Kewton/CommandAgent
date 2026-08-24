# Extension catalog development

This maintainer guide covers compiled pack capabilities and profiles. Operator
supply and naming are documented in [`docs/user/gui-extensions.md`](../user/gui-extensions.md).

## Add a pack source or check

Pack YAML is declarative and may reference only registered capabilities. Never
register a free-form shell or logic-bearing template. The registered
`command_check` is the narrow exception for external draft profiles: it takes
direct `argv`, literal `cwd: workspace`, and a typed expectation table, while
rejecting shell strings and interpreter-eval escape hatches.

1. Add the typed capability implementation in the appropriate leaf module
   under `src/planner/capability_catalog/`, or use the existing pack/data/CLI
   typed enum owner.
2. Add its `CapabilitySpec` to `src/planner/capability_catalog.rs` with fixed
   `CapabilityKind`, parameter schema, and renderer/check resolver. Keep IDs and
   parameters closed; unknown values must fail.
3. If it is a guidance source, bind it through pack vocabulary/schema and a
   bounded renderer. If it is a check, bind it through the typed internal
   check executor. Draft-profile `command_check` declarations must additionally
   pass the compiled verify policy, workspace confinement, fixed timeout, and
   output bound in `declarative_command_checks`. Do not put logic in YAML or
   Markdown.
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

## Extend a task family

`src/tui/boundary_shell/family_catalog.rs` is the canonical family catalog. A
family is a closed typed identity, not a free-form classifier label. Each
admitted catalog entry binds that identity to one admitted profile, one
registered intent, and one formal band row. The `Unknown` variant is the
fail-closed result for unrecognized input, not an admitted family.

1. Establish the formal band row and its measurement provenance first. New
   evidence belongs in a new run record; never edit historical records under
   `workspace/management/runs/` to make a family appear established.
2. Add one `TaskFamilyId` variant, include it in `TaskFamilyId::ALL`, and give
   it one canonical `as_str()` spelling. If an old spelling must remain valid,
   resolve it to the same identity in `parse()` rather than creating a second
   family. Keep unknown tokens mapped to `Unknown`.
3. Add exactly one `TASK_FAMILY_CATALOG` entry for each admitted
   profile × intent × family combination. Use the profile descriptor's
   canonical ID, a typed `IntentId`, and the exact formal `band_source` and
   `band_row`; do not use an alias or a fabricated denominator.
4. Add the matching `BAND_VALUES` entry in
   `src/tui/boundary_shell/band_catalog.rs`. Synchronize
   `workspace/management/scripts/task_family_vocabulary.py` and its owning
   aggregate/test code so producer tokens still resolve bidirectionally to the
   Rust identities. A historical producer spelling may remain as an alias, as
   `stats` does for the canonical `generic` identity.
5. Add bounded request/material observations in
   `src/tui/boundary_shell/route.rs` when the new family can be selected
   deterministically. The ambiguity classifier must select only from catalog
   candidates and must continue to reject unregistered output. Update profile
   manifest validation, extension-profile projection, and GUI/band projection
   only where the new typed identity is actually consumed.
6. Extend the catalog vocabulary, formal-row, exactly-one-band, alias,
   deterministic-route, ambiguity-rejection, and profile-link tests. Add or
   update the relevant fixture under `tests/corpus/apps/` so both the positive
   route and honest unknown/ambiguous failure are represented.
7. Run the focused family, band, routing, and producer-vocabulary checks, then
   formatting, Clippy, and the full Rust suite because the catalog is shared by
   CLI, TUI, GUI, manifests, and measurements.

The family addition is complete only when its typed identity, formal band
provenance, route, and measured value agree. Adding a classifier word, a band
value, or a tool registration by itself does not extend the catalog.

## Add an intent

`src/planner/adjudication/contract.rs` owns the typed `IntentId` and closed
`IntentContract` registry. An intent is an execution and evidence contract, not
only a CLI token. Define its failure and assurance semantics before making it
selectable.

1. Freeze the intent contract and public contract reference: phase roles,
   entry/exit requirements, expected outcomes, execution rules, evidence
   lineage, assurance policy, and required profile hooks. Add the typed
   `IntentId`, canonical `as_str()` value, `IntentContract`,
   `intent_contract()` branch, and `registered_intents()` entry together;
   unknown names must still return `None`.
2. Add a strict declarative schema under `intents/<intent>.yaml` and a bounded
   loader in `src/planner/intent_schema.rs`. YAML declares structure only;
   Rust continues to own normalization, checkpoints, adjudication, and side
   effects. Put new runtime/adjudication behavior in leaf modules and keep
   `src/planner/runner.rs` and `src/minimal_loop/loop_run.rs` to minimal wiring.
3. Update every typed ingress and projection, including the CLI `IntentArg`
   and config conversion, goal inference, boundary-shell deterministic and
   ambiguity parsing, profile-manifest parsing, extension-profile projection,
   workflow conversion, and user-visible Gate/help text. Preserve existing
   event names and schemas unless a separate migration authorizes a change.
4. Admit concrete profile × intent × family combinations through
   `TASK_FAMILY_CATALOG`, then add their formal `BAND_VALUES`. A new enum value
   alone is not routable. Each admitted profile must implement the contract's
   required hooks and evidence/assurance behavior; unsupported combinations
   must fail closed rather than fall back to `create`.
5. Update pack schema, vocabulary, compatibility, and floor checks so the new
   intent is accepted only where its contract can be enforced. Do not weaken a
   pack, verification, evidence, or release floor to admit it, and do not add a
   tool merely because the intent exists.
6. Add focused contract/schema tests, unknown-token rejection, CLI and
   manifest parsing, deterministic/ambiguous routing, profile-hook coverage,
   pack compatibility/floor coverage, evidence lineage, assurance, and
   honest-failure tests. Add the relevant `tests/corpus/apps/` fixture and
   update public EN/JA CLI/config documentation and snapshots together.
7. Run the focused schema, contract, routing, profile, pack, and corpus checks,
   then `cargo fmt --all -- --check`,
   `cargo clippy --all-targets -- -D warnings`, and `cargo test`. Treat missing
   profile hooks, evidence, formal bands, or negative coverage as incomplete,
   not as documentation-only follow-up.

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
