# Issue #442 corrective implementation — 2026-09-08

Base: `ab5c3d1461ede2a294a281dc56a5dc82ca97494e` after the required clean
fetch/fast-forward. This correction keeps business/game core implementation
with the planner when the full #442 preset includes TypeScript/import/export.

`nextjs/template_selection.rs` contains selector-only logic. Explicit core and
contract-wiring phases fall back to the planner. Before selecting setup/build,
the phase-local task passes the conservative implementation veto, so custom
plans cannot use a preset id to discard mixed implementation work. All 19
legacy implementation keywords retain their substring matching, including
inflections such as player controls, stateful updates and scoreboard. General
business implementation intent is added to declarative knowledge. Positive
template keywords require ASCII identifier boundaries; Japanese phrases keep
their existing matching. Explicit plural script/package/port tokens preserve
port-only optimization. Global goal/rules are excluded by the existing phase
field projection.

The manifest's mirrored selector declarations are synchronized with knowledge;
the existing byte/value equality test is retained.

The preset prompts, generation rules and template bodies remain byte-identical
to the base. The selector dispatcher shrinks `nextjs.rs`; no runner production
chokepoint, artifact merge, acceptance rule, budget, shell template, event or
schema changes. Existing test source and historical fixtures remain intact;
the UltraPlan flow test module only wires a new leaf test file.

New corpus directory: `tests/corpus/apps/nextjs-domain-preset-routing/`.
`r0.json` freezes the actual Japanese R0 goal, exact old/new preset tasks,
observed template and eight declared paths. Its provenance references the
immutable diagnosis, source comparison and extracted-selector replay with
SHA-256 hashes. Synthetic controls are labelled separately, including later
review controls and an exact snapshot of the legacy exclusion vocabulary.

Seven focused tests call the actual registered profile preset, full production
phase renderer and registered template selector, with real template builders:

- R0 old/new task controls in both layouts, with the current core task asserted
  byte-identical to the reviewed #442 task and all historical required paths
  retained in the full prompt.
- Ten business/game goals × four phases × two layouts = 80 combinations.
  Includes Japanese R0, English/general business, no explicit port, the prior
  inventory/shift/expense goals, Space/Breakout/Quiz and global type/import/export
  context. Setup/build remain deterministic; core/wiring return None.
- 23 custom controls × two layouts preserve lexical distinctions, explicit
  English/Japanese port-only plans, case/punctuation, incidental `core` ids,
  custom business intent and mixed core/port tasks.
- Four plural and four mixed setup/build negatives × two layouts reproduce the
  reviewer concerns and now fall back. All 19 legacy keywords and synthetic
  suffixes, plus scoreboard/player controlling, remain conservative exclusions.

The original six acceptance scenarios remain represented by the retained
response-shape tests, business/game guidance and corpus, frozen HTTP/disk
oracles, unchanged future campaign metrics, profile/event restrictions and
fixture-first/leaf/full-verification requirements. Prior UAT at `75c9a784` is
historical evidence, not transferred approval for this corrective commit.
Formal CI/UAT and binary/campaign pinning remain the root's work.

Limits: None means planner fallback, not successful UI generation. The existing
line-based phase field projection is not a general multiline/custom prompt
parser. The legacy veto may conservatively miss an optimization. Full-context
port extraction can still encounter a global default before a requested port;
numeric port assertions use the isolated explicit port-only input. Required
artifact ownership and the separately observed 3011 guidance conflict are
unchanged. R0 failed for separate reasons; nine business runs and B-1 comparison
did not run. No campaign recovery or effect is claimed.
