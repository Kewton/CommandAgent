# Profile manifest v1

`ManifestV1` is the settled profile format produced by the rule-of-two work.
It captures the common declarative shape demonstrated by the Next.js and data
profiles while keeping execution, evidence interpretation, and assurance
decisions in shared Rust code. Both repository manifests were migrated in one
change; the loader accepts `schema_version = "v1"` only and has no v0
compatibility branch.

The embedded Next.js and data manifests are both `admitted`. Admission status
controls only assurance projection: a draft profile may run and retain all
evidence, but cannot declare assurance above `static`.

## Ownership model

Layer 1 is shared Rust mechanism and shared knowledge. It owns interpretation,
execution, safety, and cross-profile contracts. Layer 2 is the profile
manifest. It owns declarative profile choices and may reference Layer-1 shared
knowledge, but it cannot replace Layer-1 behavior with executable text.

| Section | Layer-2 declaration | Layer-1 owner or reference boundary |
| --- | --- | --- |
| `metadata` | Profile `id`, display name, `v1`, and admission status. | The loader rejects every schema version except v1 and validates the closed `draft`/`admitted` status enum. The shared admission gate caps draft assurance at `static` without suppressing evidence. |
| `plan` | Preset style, intent, ordered UltraPlan phase ids/prompts, required literal `{goal}`, and optional literal `{port}`. | UltraPlan construction, placeholder expansion, scheduling, and phase execution remain Rust mechanisms. |
| `step_templates` | Scaffold/build-verification match words, implementation-kill words, ownership markers, and inert template artifact bytes. | Matching precedence, kill decisions, template selection, placeholder expansion, and file writes remain Rust mechanisms. |
| `artifacts` | Unconditional required paths and named path groups with `either_of` or `exactly_one_of` cardinality plus a preferred generation path. | Layer 1 validates safe paths and group structure. Profile adapters decide where the declaration is enforced; `preferred` is a deterministic generation choice, not a relaxation of cardinality. |
| `vocabulary` | A typed reference to `evidence_knowledge` sections `vocabulary` and `goal_hints.translations`. | Values remain single-sourced in `src/minimal_loop/evidence_knowledge.toml`; scanning and translation remain Layer 1. |
| `guidance` | A map of profile-chosen variant names. Every variant contains typed trigger conditions and a map of named message bytes. | Failure observation and trigger matching, ordering, deduplication, and prompt assembly remain Rust mechanisms. |
| `checks` | Named arrays of catalog bindings: an `id`, optional `phases`, and typed `params`. | `src/planner/capability_catalog.rs` owns parameter schemas, validation, adapters, and execution. |
| `evidence_targets` | Either a typed reference to shared `repair_targets` or a profile-local evidence-kind-to-path mapping. | Shared mappings remain single-sourced; local paths are workspace-relative. Route closure and repair-target selection remain Rust mechanisms. |

The vocabulary section uses a reference instead of copied arrays. Next.js also
references its shared evidence-target mapping. The data profile has distinct
repair targets and therefore uses the mutually exclusive local mapping form.

## Schema and load contract

The root section list is fixed by
`tests/golden/profile_manifest_v1_sections.txt`:

1. `metadata`
2. `plan`
3. `step_templates`
4. `artifacts`
5. `vocabulary`
6. `guidance`
7. `checks`
8. `evidence_targets`

Unknown root or nested fields are rejected. Metadata enums are closed, phase
ids are unique, required strings are non-empty, and `plan.profile` equals
`metadata.id`. `{goal}` is mandatory; `{port}` may be omitted by a profile
without port semantics, but when present it is the exact literal token.
Arbitrary interpolation syntax is not supported.

### Named guidance variants

Guidance no longer has fixed Next.js-era fields. A profile declares any
meaningful variant name under `guidance.variants`, typed trigger conditions,
and named message bytes:

```toml
[guidance.variants.inspection]
triggers = [
  { condition = "check_failure", values = ["data_inspection_schema"] },
]

[guidance.variants.inspection.messages]
schema_repair = "Write the inspected shape ..."
```

The closed trigger conditions are `always`, `check_failure`, `evidence_key`,
`failure_kind_prefix`, `goal_signal`, and `hidden_path`. `always` carries no
values; all other conditions carry one or more non-empty values. Variant and
message names are profile-local, so data does not occupy fields named after a
canvas application.

### Artifact cardinality

Artifact groups express alternatives without executable manifest logic:

```toml
[[artifacts.groups]]
id = "human-report"
cardinality = "exactly_one_of"
paths = ["output/report.md", "output/report.html"]
preferred = "output/report.md"
```

`either_of` means at least one listed path and `exactly_one_of` means one and
only one. Every group contains at least two unique workspace-relative paths;
the preferred path is a member of the group. Required and grouped paths cannot
overlap within a manifest.

### Phase-bound checks

Check bindings use TOML arrays under a logical binding name:

```toml
[[checks.inspection]]
id = "data_inspection_schema"
phases = ["data-inspection"]
params = {}
```

`phases` is a formal v1 field. An explicit list binds the check only to the
named declared phases. Omission means final acceptance by default. For a
canonical or dynamically named final phase, the bound set is therefore the
checks with omitted `phases` plus checks explicitly naming that final phase;
checks explicitly assigned to another phase are not carried forward. Empty,
unknown, and duplicate phase ids are rejected.

`ManifestV1::from_toml()` calls `resolve()` before returning. Every check is
resolved through the capability catalog at load time. An unknown id, missing
or extra parameter, wrong type, unsafe path, unsupported value, or registered
but unimplemented adapter makes loading fail. There is no permissive fallback
or deferred runtime validation.

`nextjs_manifest()` and the data loader embed TOML with `include_str!`, parse
once through `OnceLock`, and fail immediately on an invalid repository
manifest. Golden and direct compatibility tests compare the migrated plan,
template, guidance, and contract message bytes with the pre-v1 sources; the
schema move does not rewrite those values.

## Kept in code

The following are deliberate Layer-1 design decisions, not missing manifest
features:

- The earned assurance hierarchy (`full`/`partial`/`static`/`failed`) depends
  on observed execution and evidence history. A manifest cannot author its
  branching, thresholds, fallback, admission criteria, or terminal projection.
- Catalog check implementations, probe dispatch, parameter schemas, evidence
  formats, and pass/fail interpretation remain compiled code. A manifest may
  bind a registered capability but cannot provide executable checks or weaken
  their result.
- Template selection, expansion, writes, repair pressure, and recovery state
  transitions remain code even when their immutable input bytes are declared
  in a manifest.

## Intentionally unsupported

Manifest v1 has no representation for:

- free-form shell commands or command fragments;
- conditionals, predicates, loops, or profile-authored branching;
- executable template selection, expansion, write, or repair logic;
- capability implementations, probe dispatch, or validation weakening; or
- profile-authored admission conditions or fallback policy.

The artifact strings under `step_templates` remain inert data retained from
the existing Next.js knowledge. A new verification need must be registered in
the capability catalog with a schema, adapter, golden update, and tests; it
must not be smuggled into a manifest as a shell string.

## Format-gap ledger

This remains the canonical B-2/B-4 format-gap list. B-4 reviewed every entry;
none remains open. The allowed settlement states are `schema-v1-resolved`,
`kept-in-code`, and `future-issue`. Rows are retained with their validating
tests so the schema boundary remains auditable.

| Gap id | B-2 requirement and source | Attempted v0 mapping | Missing semantic | Fixture/test | B-4 disposition |
| --- | --- | --- | --- | --- | --- |
| `FG-B2-001` | Data plan/guidance from contract §8 | Populate Next.js-shaped `step_templates` and T28-shaped guidance slots with data values. | Fixed guidance containers made data occupy unrelated canvas-named fields. | `data_manifest_v1_knowledge_matches_golden`; `embedded_manifest_keeps_existing_nextjs_knowledge_values` | `schema-v1-resolved`: named variants and typed triggers are profile-neutral; message bytes are preserved. |
| `FG-B2-002` | Contract §1 permits `output/report.{html,md}` | Require Markdown and treat HTML as an extra artifact. | Artifact lists could not express alternative or exact cardinality. | `v1_represents_both_artifact_group_cardinalities_and_rejects_v0`; v1 artifact validation | `schema-v1-resolved`: `either_of` and `exactly_one_of` groups are first-class. |
| `FG-B2-003` | Contract §4 earned assurance hierarchy | Bind checks declaratively, then classify observed evidence in the data runtime adapter. | Assurance depends on execution history and cannot safely be manifest branching. | `data_assurance_is_earned_from_the_observed_profile_probe_level`; B-3 admission negatives | `kept-in-code`: assurance and check execution remain Layer 1, as documented above. |
| `FG-B2-004` | DATA-10 inspection stagnation in `uat-test0714-m4-004` and `uat-test0714-m4-001` | Add optional `phases` to check bindings. | Checks needed an artifact-availability phase scope and a conservative final default. | `converted_inspection_and_final_steps_match_phase_scope_snapshot`; `omitted_scope_is_final_only_and_explicit_other_phases_stay_excluded` | `schema-v1-resolved`: phase binding is a validated formal v1 field. |
| `FG-B2-005` | B-2f data inspection guidance | Store data repair text in `canvas_input_wiring_checklist`. | A data-specific variant name could not be declared because variant and message containers were fixed to Next.js names. | `inspection_literal_example_is_observation_bound_and_reaches_repair_prompt`; named-variant compatibility tests | `schema-v1-resolved`: data declares `inspection`; Next.js retains `canvas_game`, with unchanged values. |

No row is classified `future-issue` at the v1 seal. New representational needs
must append a stable row before schema expansion.

The data profile remains governed by `docs/data-profile-contract.md` (fixed).
Contract mismatches are implementation defects unless this ledger records a
genuine representational gap.

## Admission criteria

Every new profile starts as `draft`. A draft profile can execute its checks and
record evidence, but both `ultra_final_acceptance` and terminal projection cap
its assurance at `static` with reason `profile_not_admitted`. The generic
profile is admitted by definition; manifest-backed profiles must be promoted
explicitly, and an unregistered named profile fails closed as draft.

Promotion to `admitted` requires all five of the following:

1. The profile contract is marked `fixed`.
2. Its conformance suite is green, including false-success resistance tests.
3. A corpus fixture records the executable contract.
4. Distribution measurement and a committed capability band exist.
5. Promotion and supporting evidence are recorded in the mechanism ledger.

If a false success is detected, the profile returns to `draft` immediately
while the cause is investigated. The guardrail rejecting known measured
fixture terms from `plan`, `step_templates`, and `checks` is a heuristic
tripwire, not a proof against semantic overfitting; `vocabulary` is excluded
because domain vocabulary is its declared purpose.
