# Assist/eval pack institution contract

Status: fixed v0.1 (2026-08-19)

Date: 2026-08-19

This document defines the fixed institutional v0.1 contract for external
`assist.yaml`, `eval.yaml`, and `materials/*.md` files. The YAML document schema
identifiers remain `/v0`; v0.1 additively revises pack membership, supply, and
hashing without changing existing YAML shapes. Implementations and later schema
revisions are subordinate to this contract.

The inventory in
`workspace/management/runs/p0-pack-audit.md` is normative input to this contract.
In particular, a pack can compose only an injector source, injection point,
literal gate, check, extractor, or normalizer that already exists in that
inventory and in the corresponding Rust registry. A YAML name never creates
an implementation.

The key words MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY describe
the proposed conformance requirements.

## 1. Institution boundary

A pack is reviewed configuration with code-equivalent trust. It selects and
parameterizes Rust-registered behavior; it does not define validation
semantics.

- `assist.yaml` selects bounded material injection, literal examples, and
  machine-issued vocabulary.
- `eval.yaml` selects registered checks, their existing execution boundary,
  registered extraction rules and normalizers, an output schema, and an
  optional score projection governed by `docs/f1-score-institution.md`.
- The effective profile contract is always the Rust/profile-manifest floor
  merged with the pack. A pack can add checks or make registered parameters
  stricter. It cannot remove, relocate, disable, replace, or weaken a floor
  check.
- Probe, extractor, comparator, nearest-miss, normalizer, schema-validator,
  rerun, adjudication, and assurance implementations remain Rust.

The v0.1 pack composition directory is:

```text
packs/<pack-id>/<version>/
├── assist.yaml            # optional
├── eval.yaml              # optional
└── materials/             # optional
    └── <name>.md          # zero or more direct UTF-8 text members
```

At least one of `assist.yaml` or `eval.yaml` is REQUIRED. `materials/*.md` is a
bounded input to registered renderers and cannot constitute a pack by itself.
The files above are the only composition members. `pack.sha256` and `RETIRED`
are permitted management artifacts outside the hash and MUST NOT affect
execution; any other member is rejected.

## 2. Identity, hashing, and strict decoding

### 2.1 Shared identity

Every present file carries the same identity:

```yaml
pack:
  id: municipal-ingest
  version: 1.0.0
  profile: ingest
  intent: create
```

The fields have these complete v0 constraints:

| Field | Type | Constraint |
|---|---|---|
| `id` | string | `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`, at most 64 bytes |
| `version` | string | SemVer core `MAJOR.MINOR.PATCH`; non-negative decimal integers without leading zeroes |
| `profile` | enum | `data`, `python-cli`, `ingest`, `nextjs` |
| `intent` | enum | `create`, `fix`, `investigate` |

The resolved runtime MUST reject a profile/intent pair it does not support.
The two files, when both exist, MUST have byte-identical values for all four
identity fields.

### 2.2 Content hash

The runtime computes one composition hash; it is not written inside a YAML or
material file:

```text
SHA-256(
  "commandagent-pack-v0\0" ||
  u64be(len("assist.yaml")) || "assist.yaml" ||
  u64be(len(assist bytes))  || assist bytes ||
  u64be(len("eval.yaml"))   || "eval.yaml" ||
  u64be(len(eval bytes))    || eval bytes ||
  for each path in bytewise_sort(direct material paths):
    u64be(len(path bytes))     || path bytes ||
    u64be(len(material bytes)) || material bytes
)
```

For an absent file, its byte length is zero and it contributes no file bytes.
The `assist.yaml` and `eval.yaml` names are still included in that fixed order.
Material paths have the exact form `materials/<name>.md` and are ordered by
their normalized relative-path UTF-8 bytes, ascending. A pack with no material
members therefore retains its pre-v0.1 hash: the domain separator remains
`commandagent-pack-v0\0` and no empty materials marker is appended. The external
form is `sha256:` followed by 64 lowercase hexadecimal characters. The hash
pins exact bytes without truncation, newline conversion, Unicode normalization,
or YAML/Markdown reserialization.

### 2.3 Decoder rules

Both schemas use a closed world:

- one UTF-8 YAML document per file;
- mappings and sequences only; YAML tags, aliases, anchors, merge keys,
  duplicate keys, non-string mapping keys, multiple documents, and implicit
  environment substitution are rejected;
- every unknown key is rejected recursively;
- file size is at most 256 KiB, nesting depth at most 16, any sequence at most
  256 entries, and any scalar at most 64 KiB;
- paths are normalized repository-relative POSIX paths. Absolute paths,
  `..`, empty segments, NUL, and backslashes are rejected;
- there is no `include`, script, expression, template, regular-expression, or
  network source in v0.

Material membership is also closed:

- every material is one UTF-8 text file at the direct path
  `materials/<name>.md`, where `<name>.md` matches
  `^[A-Za-z0-9._-]+\.md$`;
- a material is at most 65,536 bytes and the sum of material content bytes is
  at most 262,144 bytes;
- nested paths, directories in `materials/`, absolute paths, `..`, NUL,
  backslashes, and symbolic links at the pack directory, `materials/`
  directory, or member are rejected;
- `pack.sha256`, `RETIRED`, and `journal.jsonl` are not material inputs and are
  never made visible to a renderer.

These rules apply before the contract-floor comparison.

## 3. `assist.yaml` schema

### 3.1 Complete document shape

```yaml
schema_version: commandagent.pack.assist/v0
pack:
  id: municipal-ingest
  version: 1.0.0
  profile: ingest
  intent: create
inject:
  - point: declare-ingest-inspection
    source: ingest_snapshot_structure_injected
    required: true
    params:
      max_files: 8
      max_entries: 256
      max_depth: 4
      max_bytes_per_file: 65536
      leading_lines: 12
      candidate_windows: 2
      max_chars_per_line: 200
literals:
  - gate: ingest_source_binding
    example:
      format: json
      value: '{"source_value":"令和8年8月3日","normalized":"2026-08-03"}'
vocabulary:
  - point: implement-ingest-delivery
    source: ingest_candidate_ids_injected
    mode: verbatim
    required: true
    params:
      max_ids: 1024
      max_rendered_bytes: 65536
```

Top-level keys are exactly `schema_version`, `pack`, `inject`, `literals`,
and `vocabulary`.

| Key | Type | Cardinality |
|---|---|---|
| `schema_version` | exact string | REQUIRED, exactly `commandagent.pack.assist/v0` |
| `pack` | shared identity mapping | REQUIRED |
| `inject` | sequence of `Injection` | optional, default empty, unique `(point, source)` |
| `literals` | sequence of `Literal` | optional, default empty, unique `(gate, example.value)` |
| `vocabulary` | sequence of `Vocabulary` | optional, default empty, unique `(point, source)` |

At least one of the three sequences MUST be non-empty.

### 3.2 Closed injection points

`Injection.point` and `Vocabulary.point` are exactly one of:

```text
data-inspection
data-cleaning
data-aggregation
data-reporting
data-validation
cli-scaffold
cli-implementation
cli-validation
ingest-implement
ingest-run
ingest-structural-gate
declare-ingest-inspection
implement-ingest-delivery
project-setup
core-implementation
contract-wiring
build-verification
reproduce-candidate
diagnose
bind-verify
reproduce-before
isolate-cause
implement-fix
repair
verify-after
verify-regressions
```

The point MUST occur in the resolved profile/intent plan. An ID from another
resolved plan is rejected. No alias or “nearest” point is inferred.

### 3.3 Closed sources and parameters

`Injection` has exactly:

| Key | Type | Rule |
|---|---|---|
| `point` | point enum | REQUIRED |
| `source` | source enum below | REQUIRED |
| `required` | boolean | optional, default `true`; `false` is allowed only for a source explicitly marked optional by Rust |
| `params` | source-discriminated mapping | optional; unknown keys rejected |

The v0.1 source enum and complete parameter schemas are:

| Source | Parameters | Current maximum/default |
|---|---|---|
| `ingest_snapshot_structure_injected` | `max_files`, `max_entries`, `max_depth`, `max_bytes_per_file`, `leading_lines`, `candidate_windows`, `max_chars_per_line` | unsigned integers, respectively at most/default `8`, `256`, `4`, `65536`, `12`, `2`, `200` |
| `ingest_candidate_ids_injected` | `max_ids`, `max_rendered_bytes` | unsigned integers, at most/default `1024`, `65536` |
| `R_output` | `fields`, `max_chars_per_stream` | ordered unique subset of `command`, `stdout`, `stderr`, `last_non_empty`, `traceback`; cap at most/default `500` |
| `investigation_workspace_files` | `max_files`, `max_entries`, `max_depth` | unsigned integers, at most/default `64`, `1024`, `8` |
| `R_failure_output` | `fields`, `max_chars_per_excerpt` | ordered unique subset of `location`, `error_kind`, `message`, `excerpt`, `selected_target`, `artifact_presence`; cap at most/default `500` |
| `verified_diagnosis` | `render` | exact enum `full`; default `full` |
| `cli_probe` | `case`, `fields`, `max_bytes_per_stream` | `case`: `normal` or `invalid`; fields: ordered unique subset of `argv`, `exit_code`, `stdout`, `stderr`; cap at most/default `24000` |
| `data_inspection_schema` | `fields` | ordered unique subset of `input_path`, `column_names`, `input_row_count`, `type_summaries`, `distinct_values`, `sample_rows`; the source artifact retains its fixed 2 MiB Rust limit |
| `browser_interaction` | `fields` | ordered unique subset of `dispatched_inputs`, `observed_state`, `hook_status`, `surface`, `outcome`; existing producer field bounds remain fixed and are not pack-editable |
| `pack_material_document` | `file`, `max_bytes` | `file` is a REQUIRED basename matching `^[A-Za-z0-9._-]+\.md$` and resolves only as `materials/<file>`; `max_bytes` is a positive integer at most `65536`, default `16384` |

An omitted `fields` uses the producer's registered ordered default. A numeric
parameter can only reduce an existing bound; zero and a value above the Rust
maximum are rejected. The source renderer MUST preserve the producer's
sorting, redaction, truncation, and evidence references.

`pack_material_document` is the exact Rust/source ID implemented by E-17
(Issue 116). Its renderer treats the complete hashed material as untrusted
observation data: it MUST apply credential scrub, a fixed non-instruction
preamble, source/path-labelled delimiters, and an explicit truncation marker.
`max_bytes` narrows only the rendered projection; the full material remains in
the composition hash. Conformance rejects a missing member or credential-like
material before selection, and the renderer never resolves outside the loaded
pack composition.

`cli_probe` is timing-valid only at `cli-validation`, after C1 has produced
the referenced observation in that phase. It cannot be injected at
`cli-implementation`, because that observation does not yet exist.
`data_inspection_schema` is valid only at `data-cleaning`,
`data-aggregation`, `data-reporting`, or `data-validation`, after the
inspection gate. The remaining source/point compatibility is:

| Source | Compatible point(s) |
|---|---|
| `ingest_snapshot_structure_injected` | `declare-ingest-inspection` |
| `ingest_candidate_ids_injected` | `implement-ingest-delivery` |
| `R_output`, `investigation_workspace_files` | `diagnose` |
| `R_failure_output` | `isolate-cause`, `repair` |
| `verified_diagnosis` | `implement-fix`, `repair` |
| `browser_interaction` | `build-verification` after its producing observation only |
| `pack_material_document` | Next.js create `project-setup`, `core-implementation`, `contract-wiring`, `build-verification` |

Conformance proves that the source is available before prompt rendering. It
rejects a timing cycle rather than rendering an empty placeholder.

### 3.4 Literal examples

`Literal` has exactly `gate` and `example`. `example` has exactly:

| Key | Type | Rule |
|---|---|---|
| `format` | enum | `text` or `json` |
| `value` | string | REQUIRED, 1..16384 UTF-8 bytes; JSON must parse when `format=json` |

`gate` is exactly one of the 22 registered evaluation IDs:

```text
pipeline_probe
data_inspection_schema
data_results_schema
data_reconciliation
data_claims_binding
data_rerun_consistency
before_fails
after_passes
no_regression
reproducer_fails
diagnosis_bound
cli_probe
help_binding
cli_output_claims
cli_rerun_consistency
ingest_source_binding
ingest_candidate_accounting
ingest_format_schema
ingest_rerun_consistency
path_layout_conforms
design_tokens_only
lint_config_present
```

A literal is guidance, never evidence. The runtime MUST prefix it with the
existing statement that values are examples and must be replaced by measured
values. It MUST NOT be inserted into a gate that is absent from the effective
profile/intent contract.

### 3.5 Vocabulary projection

`Vocabulary` has exactly:

| Key | Type | Rule |
|---|---|---|
| `point` | point enum | REQUIRED |
| `source` | enum | `required_delivery_vocabulary`, `ingest_candidate_ids_injected`, or `investigation_workspace_files` |
| `mode` | enum | REQUIRED and exactly `verbatim` |
| `required` | boolean | optional, default `true` |
| `params` | mapping | the matching source parameters from section 3.3; `required_delivery_vocabulary` takes `{}` only |

The projection preserves ordering and spelling. A pack cannot edit, omit, or
invent individual machine-issued vocabulary entries. Compatible pairs are:

| Source | Compatible point(s) |
|---|---|
| `required_delivery_vocabulary` | any point in the resolved plan |
| `ingest_candidate_ids_injected` | `implement-ingest-delivery` |
| `investigation_workspace_files` | `diagnose` |

## 4. `eval.yaml` schema

### 4.1 Complete document shape

```yaml
schema_version: commandagent.pack.eval/v0
pack:
  id: municipal-ingest
  version: 1.0.0
  profile: ingest
  intent: create
checks:
  - id: ingest_source_binding
    at:
      kind: final_acceptance
    extraction:
      - source_binding.source_values
    normalizers:
      - identity
      - japanese_date_to_iso
      - document_year_context
    params: {}
schemas:
  - artifact: output/records.json
    format: json
    root: array
    fields:
      - name: name
        type: string
        required: true
      - name: date
        type: string
        required: true
      - name: location
        type: string
        required: true
      - name: source_file
        type: string
        required: true
    additional_fields: false
score:
  schema_version: commandagent.eval.score/v0
  usage: [report]
  weights:
    - atom: {id: ingest_source_binding, params: {anchor: ingest.output.frozen_source}}
      points: 1
```

Top-level keys are exactly `schema_version`, `pack`, `checks`, `schemas`, and
`score`.

| Key | Type | Cardinality |
|---|---|---|
| `schema_version` | exact string | REQUIRED, exactly `commandagent.pack.eval/v0` |
| `pack` | shared identity mapping | REQUIRED |
| `checks` | sequence of `Check` | optional, default empty, unique `id` |
| `schemas` | sequence of `ArtifactSchema` | optional, default empty, unique `artifact` |
| `score` | score declaration | optional; closed schema `commandagent.eval.score/v0` |

At least one of `checks` or `schemas` MUST be non-empty.

### 4.2 Checks and execution boundary

`Check` has exactly:

| Key | Type | Rule |
|---|---|---|
| `id` | enum | one of the 22 gate IDs in section 3.4 |
| `at` | `At` mapping | REQUIRED and compatible with the registered check |
| `extraction` | sequence of extractor IDs | optional; exact registered order when the check has extractors |
| `normalizers` | sequence of normalizer IDs | optional; unique, order significant |
| `params` | check-discriminated mapping | optional, default `{}`; unknown keys rejected |

`At` is one of these complete discriminated forms:

```yaml
at:
  kind: phase
  id: data-inspection
```

```yaml
at:
  kind: stage
  id: before
```

```yaml
at:
  kind: final_acceptance
```

For `phase`, `id` is REQUIRED and is one of the point IDs in section 3.2. For
`stage`, `id` is REQUIRED and is `before`, `after`, or `diagnosis`. For
`final_acceptance`, `id` is prohibited. The current compatibility is:

| Check(s) | Required `at` |
|---|---|
| `data_inspection_schema` | `{kind: phase, id: data-inspection}` |
| data `pipeline_probe`, `data_results_schema`, `data_reconciliation`, `data_claims_binding`, `data_rerun_consistency` | `{kind: final_acceptance}` |
| `before_fails` | `{kind: stage, id: before}` |
| `after_passes`, `no_regression` | `{kind: stage, id: after}` |
| `reproducer_fails`, `diagnosis_bound` | `{kind: stage, id: diagnosis}` |
| all four CLI checks | `{kind: final_acceptance}` |
| ingest `pipeline_probe` and N2-N5 | `{kind: final_acceptance}` |
| `path_layout_conforms`, `design_tokens_only`, `lint_config_present` for Next.js create | `{kind: final_acceptance}` |

The shared `pipeline_probe` is disambiguated by `pack.profile`; it cannot be
used for an unsupported profile.

Check parameter schemas are:

- data/ingest `pipeline_probe` and their rerun check: exactly `entry` and
  `timeout_seconds`; `entry` must remain `pipeline/main.py`; timeout is a
  positive integer no greater than 30;
- all four CLI checks: exactly `entry`, `usage_paths`, and `timeout_seconds`;
  all four effective values must remain identical, `entry` must remain
  `cli/main.py`, `usage_paths` can only preserve or extend the registered
  ordered list with confined documentation paths, and timeout is a positive
  integer no greater than 5;
- `path_layout_conforms`: `required` is 1..64 unique workspace-relative glob
  strings and `forbidden` is an optional 0..64 unique glob list; absolute,
  parent-containing, NUL, backslash, and invalid glob forms are rejected;
- `design_tokens_only`: `css_globs` is a required 1..64 glob list,
  `tokens_file` is one confined workspace-relative path, and `allow` is an
  optional 0..64 unique literal list; it rejects raw hex, `rgb(`/`rgba(`, and
  `hsl(`/`hsla(` color literals outside the token file unless the exact
  matched literal is allowed;
- `lint_config_present`: `path` is one confined workspace-relative file and
  `must_contain` is an optional 0..64 unique literal list;
- every other check takes `{}` in v0.

The three generic convention checks are additive-only for the initial Next.js
create binding. They execute as bounded Rust filesystem checks, launch no
shell command, emit `pack_check_result`, and cannot replace or weaken a
profile-floor check.

Changing an entry point, increasing a bound, or moving a check is a floor
violation rather than parameterization.

### 4.3 Extraction rules

The extractor enum is exactly:

```text
claims_binding.extract_numeric_claims
claims_binding.DateLabelSpans
argv_probe.extract_usage_case
argv_probe.extract_output_examples
help_binding.extract_options
investigation_binding.bind_diagnosis
accounting.enumerate
source_binding.source_values
```

The required compatibility is:

| Check | Exact extraction sequence |
|---|---|
| `data_claims_binding` | `claims_binding.extract_numeric_claims`, `claims_binding.DateLabelSpans` |
| `cli_probe` | `argv_probe.extract_usage_case` |
| `cli_output_claims` | `argv_probe.extract_output_examples` |
| `help_binding` | `help_binding.extract_options` |
| `diagnosis_bound` | `investigation_binding.bind_diagnosis` |
| `ingest_candidate_accounting` | `accounting.enumerate` |
| `ingest_source_binding` | `source_binding.source_values` |

Checks not listed take an empty sequence. In v0 a pack records the existing
choice; it does not replace or reorder it. This explicit field makes the
effective evaluator configuration hash-visible without moving extraction
logic out of Rust.

### 4.4 Normalizers

The normalizer enum is exactly:

```text
identity
japanese_date_to_iso
document_year_context
number_canonical
time24h
```

Only `ingest_source_binding` accepts a non-empty sequence. `identity` is the
base rule. The other rules are permitted only for fields whose declared
format requires them. `document_year_context` additionally requires
`japanese_date_to_iso` and the existing two-fragment evidence condition.

The effective normalizer sequence is constrained by the profile contract and
field declarations. A pack cannot remove a floor normalizer, authorize a
normalizer for a new field, or change any rule's semantics. A rule not
registered in Rust is rejected.

### 4.5 Declared artifact schemas

`ArtifactSchema` has exactly:

| Key | Type | Rule |
|---|---|---|
| `artifact` | confined path | REQUIRED |
| `format` | enum | exactly `json` in v0 |
| `root` | enum | `object` or `array` |
| `fields` | sequence of `Field` | 1..256, unique names |
| `additional_fields` | boolean | REQUIRED |

`Field` has exactly:

| Key | Type | Rule |
|---|---|---|
| `name` | string | `^[A-Za-z_][A-Za-z0-9_]{0,63}$` |
| `type` | enum | `string`, `number`, `integer`, `boolean`, `object`, `array`, `null` |
| `required` | boolean | REQUIRED |

This is a structural declaration consumed by an existing Rust schema check.
It does not contain comparisons, code, expressions, coercion, default values,
or extraction instructions. Relative to the contract floor, a pack may add a
required field, narrow `additional_fields` from `true` to `false`, or add an
artifact schema. It cannot remove a field, make a required field optional,
change an existing field type, permit additional fields forbidden by the
floor, or replace the contract-owned artifact path.

### 4.6 Score projection

The optional `score` key has the complete closed schema, registered parameter
families, fixed state coefficients, and non-adoption invariant specified in
`docs/f1-score-institution.md`. Score atoms MUST name checks present in the
same `eval.yaml`; unknown atoms, free-form judges, invented existence-only
atoms, coefficient/formula overrides, and `usage: adoption` are rejected.
Scoring reads existing typed evidence and emits additive evidence only. It
cannot change verdict, assurance, earned, admission, or release-gate state.

## 5. Contract-floor merge

The loader first resolves the Rust profile/intent contract and its manifest,
then validates the pack, then constructs one effective configuration. It
never treats YAML as the base configuration.

The floor contains:

- data: the execution prerequisite, inspection phase gate, and E1-E4;
- fix: F1-F3 and their stage, lineage, epoch, and outcome rules;
- investigate: I1-I2 and their stage, lineage, epoch, and binding rules;
- python-cli: C1-C4 with one shared runtime input;
- ingest: N1-N5, candidate freeze, source-binding semantics, and declared
  normalization evidence conditions;
- every profile's admission cap and assurance projection.

A pack-floor comparison MUST reject:

1. a missing floor check;
2. a floor check moved to another phase, stage, or boundary;
3. an execution timeout increased or isolation/boundedness disabled;
4. a required usage path, extractor, normalizer, schema field, evidence field,
   or source reference removed;
5. a parser/check parameter changed outside its registered narrowing rule;
6. a schema loosened;
7. a literal presented as evidence or measured material;
8. a source rendered before its producing observation;
9. a pack request to change verdict, assurance, admission, or adjudication.

Additive checks still require registered Rust IDs and compatibility. “Stricter”
means a registered narrowing with the same meaning, not an arbitrary failure
condition that creates false positives.

## 6. Measurement metadata and band identity

Every run using a pack MUST pin this effective composition in run metadata:

```yaml
pack:
  id: municipal-ingest
  version: 1.0.0
  hash: sha256:<64-lowercase-hex>
  source: admitted # admitted | repository | local
  assist_present: true
  eval_present: true
  assist_schema_version: commandagent.pack.assist/v0
  eval_schema_version: commandagent.pack.eval/v0
```

The runtime also records the resolved profile/intent floor version through
the existing manifest/contract metadata. A report MUST render the pack ID,
version, and hash. Scrub MUST inspect pack bytes and rendered injection
evidence before the run is publishable.

A band key is the existing model configuration **plus the exact pack
composition hash**. Runs with different pack hashes cannot be silently
aggregated. An unconfigured run is a distinct `no-pack` composition, not an
implicit empty hash.

Historical runs without pack metadata retain their current interpretation and
are never rewritten.

## 7. Trust boundary and review

A pack crosses a code-equivalent trust boundary because assist text enters a
model prompt and eval configuration participates in release/assurance gates.
Conformance establishes structural safety and floor preservation; it does not
by itself establish review, admission, signature provenance, or measured
effectiveness.

- Admitted pack changes require code-equivalent review, scrub, conformance,
  CI, and an exact admitted-registry tuple. The reviewed exact bytes and hash
  are committed together.
- Repository and local packs are explicitly unapproved. A pin makes their
  bytes reproducible but does not grant admission or a measured band.
- Measured sources remain untrusted data. Rust renderers delimit, escape,
  truncate, redact, and label them as observations; YAML cannot mark source
  bytes as instructions. `materials/*.md` is treated the same way regardless
  of supply source.
- Pack files cannot read environment variables, credentials, arbitrary files,
  network resources, or another pack.
- A pack cannot choose a repair target, ownership boundary, verdict, or
  assurance level.

This trust boundary is why packs are not general prompt templates or
validation programs.

### 7.1 Supply identity: `PackSource`

The Rust/API type name is exactly `PackSource`, with the following closed enum
and snake-case serialized values:

```rust
pub enum PackSource {
    Admitted,
    Repository,
    Local,
}
```

| Value | Definition | Selection and execution | Mutating operations | Required Japanese display |
|---|---|---|---|---|
| `admitted` | Exact `id`, version, hash, and point match an entry in the reviewed admitted registry | selectable when compatible; runs retain their measured-band identity only when that exact composition was measured | not through the extension-root supply API; change by reviewed repository commit | `承認済み` |
| `repository` | Pinned repository `packs/` member that is not an exact admitted tuple | explicit selection only after baseline conformance and profile/intent compatibility | verify or bundle freely; stage/pin changes go through source control and review; extension-root retire does not apply | `リポジトリ（未承認）` |
| `local` | Pinned unsigned pack under operator-supplied `--extension-root` | explicit selection only after baseline conformance and profile/intent compatibility; GUI supply/selection requires Trial authentication and mutating requests also require Origin | authenticated stage, verify, pin, bundle, and retire in the extension root; overwrite and delete are prohibited | `ローカル（未承認・帯域未計測）` |

Every list, Gate 1 card, acceptance sheet, GUI row, and machine summary MUST
carry `id@version`, full `sha256:` hash, and `PackSource`. Gate 1, acceptance,
and GUI use the Japanese display above. `--summary-json` uses the locale-neutral
snake-case enum; `local` normatively means both unapproved and band-unmeasured,
so no contradictory approval flag is introduced. The guarantee text for a
local pack is `pack 固有保証なし（既存 profile/intent の earned assurance のみ）`.

The resolution order is extension-root, then repository `packs/`. When a local
pack shadows the same repository `id@version`, the resolved source remains
`local` and Gate 1/GUI MUST add
`ローカル優先: 同名のリポジトリ pack より拡張ルートを優先`. Displaying a pack
does not select it, and a displayed extension not captured by the confirmation
identity MUST NOT affect execution.

### 7.2 Extension-root layout and lifecycle

The three roots are mutually disjoint: `--repository-root` is the repository
read boundary, `--execution-root` owns live `.anvil/` state, and
`--extension-root` owns extension supply. The extension root and its managed
children MUST be non-symlink directories writable only by their owner; a
group- or other-writable root is rejected.

```text
<extension-root>/
├── packs/<id>/<version>/
│   ├── assist.yaml
│   ├── eval.yaml
│   ├── materials/*.md
│   ├── pack.sha256
│   └── RETIRED
└── journal.jsonl
```

`stage` creates only an unpinned new `id@version` via temporary-directory plus
atomic rename. `verify` applies the same baseline conformance used at runtime.
`pin` re-reads and re-hashes the members, then creates `pack.sha256`; it MUST
NOT overwrite an existing pin. `retire` creates `RETIRED` without deleting or
rewriting pack bytes, the pin, or history. A retired pack remains listable and
bundle-readable for audit, but `locate_pinned` and new selection reject it.

### 7.3 Extension journal schema

`<extension-root>/journal.jsonl` is append-only UTF-8 JSON Lines. The API type
name is `JournalEntry`, appended only through
`planner::pack::supply::journal::append(root, &JournalEntry)`. Each operation
appends one closed object with all fields required:

```json
{"ts":"<RFC3339>","actor":"gui|cli","action":"stage|verify|pin|retire","pack":{"id":"<pack-id>","version":"<semver-core>","hash":"sha256:<64-lowercase-hex>"},"result":"ok|error","detail":"<bounded scrubbed text>"}
```

`ts` includes an RFC 3339 timezone. `actor`, `action`, and `result` use exactly
the alternatives above. `detail` is at most 4,096 UTF-8 bytes after credential
scrub and MUST NOT contain secret source text. The journal is outside every
pack hash. Existing lines MUST NOT be edited, truncated, reordered, or changed
from `error` to `ok`; retirement does not erase history.

## 8. Pack conformance

The baseline conformance required before a repository or local pin is
selectable comprises conditions 1 through 6:

1. strict schema and duplicate/unknown-key rejection;
2. identity agreement and reproducible exact-byte hash;
3. closed-ID registry resolution for every source, point, gate, check,
   extractor, and normalizer;
4. profile/intent/phase and source-before-point compatibility;
5. contract-floor comparison;
6. path confinement, bound checks, and scrub;
7. **admission only:** a real measured fixture for every
   `inject`/`vocabulary` source and every
   added check;
8. **admission only:** golden rendering for byte-compatible externalization,
   or an explicitly
   additive fixture for new behavior;
9. **registry/release gate:** evidence that floor checks still execute from the production acceptance
   path;
10. **registry/release gate:** negative fixtures for unknown ID, unknown key, timing cycle, floor check
    removal, parameter weakening, schema weakening, and hash mismatch.

Conditions 7 and 8 are REQUIRED for promotion to `admitted`; their absence is
expected for an experimental local pin and MUST be displayed as unapproved and
band-unmeasured, not silently treated as success. Conditions 9 and 10 protect
the Rust registry/release that interprets packs and are not waived by local
supply. The conformance report pins the same pack hash used by selection and
measurement. Synthetic fixtures may test rejection paths, but an admitted pack
also requires its declared real measured fixture.

## 9. Permanently out of scope

The following do not move into a v0 or future pack without a separate
institutional contract:

- implementation of probes, extractors, comparators, nearest-miss logic,
  normalizers, selector engines, candidate freezing, schema validators, or
  rerun logic;
- executable scripts, expressions, regular expressions, plugins, dynamic
  includes, or network fetches;
- adjudication, lineage, epoch, verdict, assurance projection, admission, or
  contract-floor computation;
- configuration below a profile contract floor;
- new source, point, gate, check, extractor, or normalizer IDs created only by
  spelling them in YAML.

YAML is composition. Validation remains Rust so that a semantic change passes
code review, focused tests, conformance, and CI.

## 10. P-1 first applications

| P-1 application | Valid v0 binding | Existing behavior externalized byte-for-byte | Required implementation/admission work |
|---|---|---|---|
| cli-assist actual output | `source=cli_probe`, `point=cli-validation`, after C1 observation | none; current C1 evidence is not prompt material | typed bounded renderer, within-phase timing hook, source/point compatibility registration, real C1 fixture, additive rendering golden; using it at `cli-implementation` is rejected |
| data-assist actual structure | `source=data_inspection_schema`, `point=data-cleaning` or a later data phase | none; current literal guidance remains the contract floor | typed renderer of canonical inspection evidence, ordering proof after `data-inspection`, measured fixture and scrub |
| nextjs-acme convention fixture | `source=pack_material_document` at `project-setup`/`contract-wiring`; `path_layout_conforms`, `design_tokens_only`, and `lint_config_present` at `final_acceptance` | none; the existing Next.js build/browser/hook/testimony floor remains Rust/manifest-owned | implemented by E-17 as the unadmitted repository fixture `nextjs-acme@1.0.0`; admission and a measured band remain separate work |

The existing ingest structure and frozen-ID injectors are the byte-compatible
reference migrations for loader implementation:

| Reference migration | Existing binding | Migration requirement |
|---|---|---|
| ingest snapshot material | `ingest_snapshot_structure_injected` → `declare-ingest-inspection` | exact prompt/event bytes and bounds remain unchanged |
| ingest frozen vocabulary | `ingest_candidate_ids_injected` → `implement-ingest-delivery` | exact canonical IDs, order, prompt/event bytes, and freeze timing remain unchanged |
| investigate observed failure | `R_output` + `investigation_workspace_files` → `diagnose` | exact bounded rendering remains unchanged |
| fix failure/carry | `R_failure_output` → `isolate-cause`/`repair`; `verified_diagnosis` → `implement-fix`/`repair` | exact evidence ownership, carry validation, and rendering remain unchanged |

P-1 should first prove one byte-compatible externalization, then add the two
assist behaviors, and only then bind the newly registered nextjs evaluator.
No provisional YAML ID is permitted.

## 11. Resolved review questions

The 2026-07-30 review resolved all four questions:

1. **Identity:** the institutional identity is the exact-byte hash in
   section 2.2. Canonical YAML hashing is not used.
2. **CLI ownership:** `cli-validation` is the valid ownership point for
   post-C1 injection. `cli-implementation` remains invalid because the C1
   observation does not yet exist.
3. **v0 schema:** the structural subset is sufficient for the first three
   packs. Any revision requires the same sequence as a new contract:
   inventory, draft, review adjudication, fixed seal, conformance, and
   migration fixture.
4. **Supply location (revised by v0.1 on 2026-08-19):** reviewed packs continue
   to live in the repository under `packs/`. Operator-supplied unsigned packs
   MAY live under `--extension-root/packs/` as pinned `local` packs, but remain
   unapproved, band-unmeasured, and explicitly selected. Resolution is local
   before repository. Signed or remote supply, publisher identity, trust roots,
   and revocation remain Phase G **QUEUED** work; local operator supply is not a
   signature substitute or admission path.

This resolution seals institutional contract v0.1 while retaining the `/v0`
YAML schemas. Schema evolution is explicit and versioned; a loader must not
infer a later schema from unknown keys.
