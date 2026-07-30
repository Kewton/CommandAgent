# P-0 assist/eval pack mechanism audit

Status: investigation-only design input (2026-07-30)

Baseline: `489c98869d005110888e170f42e67cefd3d1e98e`

## Scope and counting rule

This audit inventories the existing machinery that can inform an external
`assist.yaml` or `eval.yaml` pack. It does not introduce a pack loader or
change production behavior.

An **injector route** is one production composition boundary that adds
machine-selected material, a literal example, or a closed vocabulary to a
model-facing phase/step/repair prompt. Repeated stable/legacy renderings of
the same payload count once. A manifest prompt and its Rust loader count as
one route, not one route per sentence. Ordinary prompt framing (goal, profile,
style, step id), tool instructions, and error reporting to the human are not
assist injection and are excluded.

An **evaluation binding** is a check or intent requirement that is bound to a
profile phase or an evidence stage. The E/F/I/C/N inventory has 20 bindings
and 19 distinct IDs because `pipeline_probe` is shared by data and ingest.
There are 18 contract-lettered gates; `data_inspection_schema` and the data
execution prerequisite are additional machine bindings.

Under those rules the current tree contains:

- **17 injector routes** (10 fixed/literal/vocabulary routes and 7 measured
  material routes);
- **20 E/F/I/C/N bindings / 19 distinct IDs**;
- **8 named extraction-rule candidates** grounded in current Rust symbols;
- **5 declared normalizer IDs**. Two unconditional text operations
  (`decode_entities`, `normalize_space`) are implementation details of
  `identity`, not additional declared normalizers.

The source was found with `rg` over `src/planner`, `src/workflow`,
`intents/*.yaml`, all profile manifests, and the fixed profile/intent
contracts, followed by caller review. Line anchors are baseline anchors;
symbol and wire IDs are the durable references.

## 1. Injection machinery

### 1.1 Active injector routes

`Pack exposure` is a design recommendation, not current behavior.

| # | Candidate injector/source ID | Current production location | Current call point (profile × phase) | Parameters/material | Determinism basis | Pack exposure |
|---:|---|---|---|---|---|---|
| 1 | `profile_manifest_phase_prompt` | `src/planner/runner/driver.rs:3058,3164`; loaders in `profiles/{data,python_cli,ingest}/manifest.rs` | data × `data-inspection..data-validation`; python-cli × `cli-scaffold..cli-validation`; ingest × `ingest-implement..ingest-structural-gate` | fixed manifest phase id, goal substitution | embedded TOML, fixed phase order, only `{goal}` substitution | literals only; the plan floor stays Rust/manifest-owned |
| 2 | `profile_manifest_guidance` | `src/planner/profile.rs:1167`; `profiles/{data,python_cli,ingest}/manifest.rs::guidance` | profile-wide plan and phase prompts | ordered manifest guidance messages | strict manifest parser, unknown keys rejected, embedded bytes | yes, as reviewed literal additions above the floor |
| 3 | `profile_generation_rules` | `src/planner/runner/driver.rs:3117-3121,3244-3247`; `DomainProfile::generation_rules` | all generated phases; concrete data/python-cli/ingest/nextjs/generic variants | profile + intent | typed `ProfileRuntime` resolve and Rust/embedded text | no replacement; pack may add literals, never replace the rule |
| 4 | `profile_runtime_contract` | same driver sites; `DomainProfile::runtime_contract` | all generated phases | profile + intent + goal where implemented | typed runtime resolve; fixed renderer | no replacement; contract floor |
| 5 | `required_delivery_vocabulary` | `runner/driver.rs:2846-2932,3058-3255` | every step/phase | required artifacts, capabilities, evidence, obligations, available prior artifacts | sorted/registry-derived vectors and one resolved runtime | vocabulary projection may be exposed; producer stays Rust |
| 6 | `data_literal_examples` | `profiles/data/manifest.toml:17,21-33,114,134-141`; `profiles/data/manifest.rs:60-92` | data × inspection/generation and data acceptance repair | inspection 5-key shape, results/reconciliation shape, deterministic rules | embedded manifest; values explicitly marked examples; manifest checks resolve through catalog | yes: literal additions at existing data gates |
| 7 | `cli_contract_guidance` | `profiles/python_cli/manifest.toml:17-25,86-100`; `profiles/python_cli/manifest.rs:47-55` | python-cli × scaffold/implementation/validation and C repair | determinism, parser, frozen case binding, stdout claims | embedded manifest; C1-C4 share one typed input adapter | yes: literal additions at existing CLI gates |
| 8 | `ingest_canonical_literals` | `profiles/ingest/guidance.rs:1-45`; `profiles/ingest/manifest.toml:17,82-93`; `profiles/ingest/manifest.rs:98-150` | ingest × implement/structural gate and N repair | selector kinds, selector/inspection/records literal shapes, CSS supported forms, normalization vocabulary | Rust constants are asserted byte-present in both implement prompt and repair guidance | yes: literal/vocabulary additions above N floor |
| 9 | `investigation_claim_format` | `investigation_plan_synthesis/guidance.rs:10-24` | investigate × `diagnose` | exact error quote, `path:line`, existing-code block shape | fixed Rust literal; values must be replaced by observed R/files | yes: literal at `diagnosis_bound` |
| 10 | `nextjs_contract_knowledge` | `profiles/nextjs/knowledge.toml`; `profiles/nextjs/knowledge.rs`; `profiles/nextjs.rs:222-280` | nextjs × all create phases and interaction repair | route-bound hook vocabulary, state/action examples, interaction repair messages | strict embedded knowledge schema plus snapshot/corpus tests | limited to literals/vocabulary; verifier remains Rust |
| 11 | `ingest_snapshot_structure_injected` | `step_material.rs:9-28`; `profiles/ingest/snapshot_structure.rs:9-113` | ingest step `declare-ingest-inspection` (inside implement plan, before selector declaration) | root fixed to `data/snapshots`; caps: files 8, entries 256, depth 4, bytes/file 64 KiB, first 12 lines, 2 candidate windows, 200 chars/line | sorted regular files; symlinks ignored; bounded deterministic excerpts; event records limits and omissions | yes; first-class material source |
| 12 | `ingest_candidate_ids_injected` | `step_material.rs:30-52`; `profiles/ingest/candidate_guidance.rs:8-71` | ingest step `implement-ingest-delivery`, after selector declaration and before pipeline implementation/run | frozen selector + at most 1,024 canonical IDs and 64 KiB rendered text | `accounting::freeze`; sorted snapshot traversal; exact IDs; event records freeze evidence | yes; first-class vocabulary source |
| 13 | `R_output` | declared at `intents/investigate.yaml:7-11`; rendered in `investigation_plan_synthesis/observed_failure.rs:6-53`; composed in `guidance.rs:20-28` | investigate × `diagnose` | `evidence/investigation-run.json`, command, stdout/stderr, last non-empty excerpt, traceback | executed I1 evidence; each rendered stream uses the fixed 500-character snippet cap | yes; first-class measured source |
| 14 | `investigation_workspace_files` | `investigation_plan_synthesis/guidance.rs:20-77` | investigate × `diagnose` | existing files only; max 64 files, 1,024 scanned entries, depth 8; hidden/node_modules/target/vendor excluded | breadth-first bounded scan, normalized paths, lexical sort | yes; first-class vocabulary source |
| 15 | `R_failure_output` | declared at `intents/fix.yaml:4`; `fix_diagnostics.rs:97-181`; `fix_diagnostics/prompt_guidance.rs:3-34`; `fix_runtime/data_isolate.rs:15-59` | fix × `isolate-cause` and `repair` | executed F1 location, error kind/message/excerpt, selected target; present/absent canonical artifacts | runtime F1 evidence, traceback/parser extraction, sorted artifact presence, bounded body snippets | yes as measured material; target selection stays Rust |
| 16 | `verified_diagnosis` | declared at `intents/fix.yaml:5`; carried by `workflow/schema.rs::Carry::Diagnosis`; injected by `workflow/orchestrator.rs:614-628` | recovery workflow investigate→fix, then fix × `implement-fix`/repair targeting | I2-matched `output/diagnosis.md` and binding | workflow v0.1 carry validation; diagnosis is accepted only with I2 binding; same origin workspace | yes as trusted measured material; carry/adjudication stays Rust |
| 17 | `measured_repair_context` | `profiles/data/repair_policy/{inspection_guidance,claims_binding_guidance}.rs`; `fix_reproducer.rs:20-44`; `fix_contract_predicate.rs:12-74`; `contract_attribute_repair.rs:100-151`; `final_acceptance.rs:1250-1590` | data/nextjs/fix/final-acceptance repair phases | failed check output, claims nearest miss, reproducer suggestion, contract attribute location, state/interaction probe observations | only current verification/evidence objects; deduplicated/sorted where set-like; bounded snippets; fixture coverage | split before exposure; packable literals/measured excerpts only, never repair target or verdict math |

The last row is one route family because all members enter the same bounded
repair-prompt boundary. It must be split into typed sources before any member
is exposed; wrapping the existing free-form repair prompt as one pack source
would not create a closed vocabulary.

### 1.2 Existing sources not yet injected

These are existing Rust/evidence producers that P-1 can expose without
inventing a source implementation. They are **not** included in the 17 active
injector count.

| Source ID | Existing producer | Existing fields suitable for assist | Intended first use |
|---|---|---|---|
| `cli_probe` | `profiles/python_cli/argv_probe.rs::Report` → `evidence/cli-probe.json` | frozen normal/invalid cases, command argv, exit, bounded stdout/stderr, C1/C4 result | cli-assist actual-output injection at `cli-implementation` or a bounded repair turn |
| `data_inspection_schema` | `profiles/data/inspection_schema.rs` → `evidence/inspection-schema.json`, with deterministic CSV/TSV selection/table inspection | selected input path, headers, row count, observed types/distinct values/sample rows through the canonical inspection artifact | data-assist actual-structure injection after `data-inspection`, before cleaning/aggregation |
| `browser_interaction` | registered capability plus `minimal_loop/interaction_probe.rs` evidence | dispatched inputs, observed state transitions, hook status, surface/probe outcome | possible nextjs measured assist; not the requested testimony evaluator |

The requested P-1 `nextjs-eval` testimony gate has **no current registered
check ID**. It therefore cannot appear in an eval pack under the closed-ID
rule. P-1 must first add and test the Rust check/capability, then an eval pack
may bind that real ID.

### 1.3 Existing IDs available as injection points

Only existing manifest/intent/step IDs are eligible points in the draft:

| Domain | Existing point IDs |
|---|---|
| data create | `data-inspection`, `data-cleaning`, `data-aggregation`, `data-reporting`, `data-validation` |
| CLI create | `cli-scaffold`, `cli-implementation`, `cli-validation` |
| ingest create | `ingest-implement`, `ingest-run`, `ingest-structural-gate`, `declare-ingest-inspection`, `implement-ingest-delivery` |
| nextjs create | `project-setup`, `core-implementation`, `contract-wiring`, `build-verification` |
| investigate | `reproduce-candidate`, `diagnose`, `bind-verify` |
| fix | `reproduce-before`, `isolate-cause`, `implement-fix`, `repair`, `verify-after`, `verify-regressions` |

`repair` and `verify-regressions` are current runtime plan IDs while
`implement-fix` and `verify-after` are current `IntentSchema` IDs. The pack
loader must resolve the selected profile/intent plan first and reject a point
that is not present in that resolved plan; aliases are not silently invented.

## 2. Evaluation configuration

### 2.1 E/F/I/C/N bindings

| Family | Contract role | Registered ID | Current binding (`at`) | Parameters | Implementation |
|---|---|---|---|---|---|
| E prerequisite | pipeline execution | `pipeline_probe` | data final acceptance, before E1-E4 evaluation | `entry=pipeline/main.py`, `timeout_seconds=30` | `capability_catalog/data.rs`; bounded pipeline probe |
| E phase gate | inspection schema | `data_inspection_schema` | `data-inspection` only | none | `profiles/data/inspection_schema.rs` |
| E4 | fixed results schema | `data_results_schema` | data final acceptance | none | `profiles/data/results_schema.rs`, `checks.rs` |
| E1 | reconciliation | `data_reconciliation` | data final acceptance | none | `profiles/data/checks.rs` |
| E2 | report claims binding | `data_claims_binding` | data final acceptance | none | `profiles/data/claims_binding*.rs` |
| E3 | rerun consistency | `data_rerun_consistency` | data final acceptance | `entry=pipeline/main.py`, `timeout_seconds=30` | `profiles/data/checks.rs` |
| F1 | before fails | `before_fails` | `stage=before`, before workspace mutation | bound R, failure, lineage, epoch | `adjudication/contract.rs`, `adjudication/fix.rs`, `fix_runtime.rs` |
| F2 | after passes | `after_passes` | `stage=after` | same R/binding/lineage as F1, newer epoch | same |
| F3 | no regression | `no_regression` | `stage=after`, after F2 | frozen profile regression IDs and lineages | same plus `ProfileRuntime` regression adapters |
| I1 | reproducer fails | `reproducer_fails` | `stage=diagnosis`, before diagnosis | bound R, failure, lineage, epoch | `adjudication/contract.rs`, `investigation_runtime.rs` |
| I2 | diagnosis bound | `diagnosis_bound` | `stage=diagnosis`, after report creation | I1 output + `output/diagnosis.md` | `investigation_binding.rs`, `adjudication/investigate.rs` |
| C1 | argv probe | `cli_probe` | CLI final acceptance | `entry`, ordered `usage_paths`, timeout 5 | `profiles/python_cli/argv_probe.rs` |
| C2 | help binding | `help_binding` | CLI final acceptance, same frozen input as C1 | same | `profiles/python_cli/help_binding.rs` |
| C3 | output claims | `cli_output_claims` | CLI final acceptance, same frozen input as C1 | same | output-example branch in `argv_probe.rs` |
| C4 | rerun consistency | `cli_rerun_consistency` | CLI final acceptance, same frozen input as C1 | same | repeated observation in `argv_probe.rs` |
| N1 | pipeline execution | `pipeline_probe` | ingest final acceptance | `entry=pipeline/main.py`, timeout 30 | shared data probe adapter; ingest-specific evidence projection |
| N2 | source binding | `ingest_source_binding` | ingest final acceptance, after freeze | none | `profiles/ingest/source_binding.rs` |
| N3 | candidate accounting | `ingest_candidate_accounting` | ingest final acceptance, after freeze | none | `profiles/ingest/accounting.rs` |
| N4 | format schema | `ingest_format_schema` | ingest final acceptance, after freeze | none | `profiles/ingest/runtime.rs::check_format_schema` |
| N5 | rerun consistency | `ingest_rerun_consistency` | ingest final acceptance | `entry=pipeline/main.py`, timeout 30 | ingest runtime using typed rerun adapter |

The profile manifests bind 6 data checks, 4 CLI checks, and 5 ingest checks.
The intent contract registry binds 3 fix and 2 investigation requirements.
`pipeline_probe` is the sole cross-profile duplicate.

### 2.2 Extraction rules

These eight IDs are candidate pack vocabulary because each is an existing,
deterministic Rust symbol or module boundary. A pack may select a compatible
rule; it may not supply executable extraction logic.

| Extraction ID | Used by | Current rule |
|---|---|---|
| `claims_binding.extract_numeric_claims` | `data_claims_binding` | visible Markdown/HTML text; signed/grouped decimal and percent claims; max 10,000 |
| `claims_binding.DateLabelSpans` | `data_claims_binding` | ISO year-month/day spans are labels, not independent quantity claims |
| `argv_probe.extract_usage_case` | `cli_probe` | first safe Python invocation in fenced README/USAGE, in ordered path priority |
| `argv_probe.extract_output_examples` | `cli_output_claims` | output in command fence or deterministic labeled block immediately after it |
| `help_binding.extract_options` | `help_binding` | sorted distinct dash-prefixed runtime help tokens |
| `investigation_binding.bind_diagnosis` | `diagnosis_bound` | exact error quotes, existing `path:line`, fenced existing code snippets |
| `accounting.enumerate` | `ingest_candidate_accounting` | selector-specific whole candidate blocks with stable path+ordinal IDs |
| `source_binding.source_values` | `ingest_source_binding` | fragments from one frozen candidate plus declared, evidence-recorded normalization |

The CLI optional-argument/placeholder resolver and ingest unique-suffix
candidate-ID resolver are subordinate deterministic steps of the listed
extractors, not separately selectable extraction policies.

### 2.3 Normalizers

`profiles/ingest/source_binding.rs::NormalizationRule` is the only current
declared normalizer vocabulary:

| Normalizer ID | Semantics and evidence condition |
|---|---|
| `identity` | HTML entities decoded and whitespace deterministically collapsed; output value otherwise unchanged |
| `japanese_date_to_iso` | Japanese era or Japanese Gregorian date to `YYYY-MM-DD`; validates the calendar date |
| `document_year_context` | partial month/day completed with one unique title/external-heading year; requires both source fragments and positions and must accompany `japanese_date_to_iso` |
| `number_canonical` | full-width digits, full-width punctuation, and grouping separators canonicalized without changing numeric value |
| `time24h` | deterministic Japanese AM/PM/hour form to 24-hour `HH:MM`, with range checks |

`decode_entities`, `normalize_space`, `ascii_digits`, rounding-to-printed
precision, verify-command normalization, and CLI placeholder binding remain
Rust implementation details. They have no independent registered ID and must
not be named by a pack in v0.

### 2.4 Manifest binding model

- data, CLI, and ingest manifests use `[[checks.<slot>]]` entries. Each ID and
  parameter set is resolved through `capability_catalog`; unknown IDs,
  parameters, types, paths, and enum values are rejected.
- a missing `phases` scope means final acceptance; explicit phases narrow only
  the phase gate. Data deliberately scopes `data_inspection_schema` to
  `data-inspection` and omits phase scope for final E checks.
- CLI validation requires exactly C1-C4 and the runtime additionally proves all
  four adapters share the same entry, usage paths, and timeout.
- ingest validation requires exactly N1-N5.
- F/I use the typed `IntentContract` registry rather than profile manifest
  check tables. Stage, expected outcome, execution rule, lineage, and impact
  are Rust values and are not pack-editable.

## 3. Packable / non-packable adjudication proposal

### Packable as closed configuration

- selecting one of the audited source IDs at a compatible existing point;
- decreasing an existing bound or selecting a fixed presentation field from a
  measured source, when the source-specific parameter schema permits it;
- adding a reviewed literal example to an existing gate;
- projecting an already machine-issued vocabulary verbatim;
- composing registered check IDs at existing stages/phases;
- selecting one of the eight extraction IDs for a check whose compatibility
  table permits it;
- selecting any subset/order of the five declared normalizers **only where the
  profile contract and field declaration already permit it**;
- declaring an output JSON schema that is additive to, or stricter than, the
  profile contract floor.

### Must remain Rust

- probe execution, isolation, timeout enforcement, output caps, and evidence
  writers;
- extractor, comparator, nearest-miss, normalizer, selector, candidate-ID,
  schema-validator, and rerun implementation;
- F/I lineage/epoch/stage semantics and all assurance/adjudication projection;
- profile contract floors and admission caps;
- material trust decisions, path confinement, secret scrub, symlink handling,
  and source truncation;
- repair-target selection and source ownership;
- registration of a new source, point, gate, extraction rule, normalizer, or
  check ID.

This is deliberately not “validation as YAML”. YAML chooses reviewed,
Rust-registered components. Rust still defines what each component means and
whether the resulting evidence earns assurance.

## 4. P-1 migration map

| P-1 pack | Existing Rust source/gate | Existing injection replaced byte-for-byte | New work required before pack binding |
|---|---|---|---|
| cli-assist actual-output injection | source `cli_probe`; point `cli-implementation` | none: C1 currently writes evidence but does not inject it | typed, bounded rendering of existing C1 observation; register compatibility; golden byte fixture |
| data-assist actual-structure injection | source `data_inspection_schema`; point `data-cleaning` (or later data phase after inspection) | no standalone injection; it externalizes the same “observed values, not invented values” policy currently carried by data literal guidance | typed rendering from the canonical inspection artifact/evidence; point-order proof; measured fixture |
| nextjs-eval testimony gate | no current check ID | none | implement/register the Rust gate first, add conformance and fixture, then reference the real ID from `eval.yaml`; pack parsing must reject any provisional name |

The two assist packs can use existing producer IDs, but their render adapters
are new. The nextjs eval pack cannot be valid until its evaluator exists in
Rust. This ordering is the closed-vocabulary rule in action, not a gap to paper
over with a provisional YAML string.

## 5. Design conclusion

The repository already has the three ingredients needed for packs:

1. typed, bounded producers;
2. stable phase/check/intent IDs;
3. strict manifest/capability validation.

What is missing is a reviewed composition layer and its contract-floor guard.
The pack should externalize **selection and parameters**, not validation
semantics. The recommended v0 therefore exposes only the closed IDs cataloged
above, pins the exact pack bytes into measurement metadata, and refuses to
load an ID that Rust has not registered.
