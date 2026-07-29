# E-5a ID vocabulary audit

Date: 2026-07-29  
Baseline: `386fa3cf8be9a23bfa0b3a5b0a63b3f0c59edb6b`

## Scope and method

`src/**/*.rs` contains 1,747 `format!` invocations at this baseline. This audit
selects every production invocation whose format literal constructs a
machine-consumed identifier in one of these protocol families:

- `*_violation`
- `*_underivable`
- `edge_not_earned`
- `data_assurance_*`

The result is 51 construction sites. Prose/UI rendering, filesystem paths,
URLs, timestamps, human diagnostics without a protocol ID prefix, and
test-only format strings are outside this audit. A repeated format at a
different source location is a separate site because each location can drift
independently.

`classes.toml` correspondence uses the substring semantics implemented by
`workspace/management/scripts/classify_runs.py`. `yes` means that the emitted
family is covered by a registry pattern. `specific only` means that
`classes.toml` contains one or more measured concrete members but not the
whole emitted family. `no` means no current `match_reason` or
`match_stop_class` covers the shape.

## Inventory

| # | Location | ID kind | Current emitted shape | `classes.toml` correspondence |
|---:|---|---|---|---|
| 1 | `src/workflow/orchestrator.rs:95` | stop_class (reason payload) | `edge_not_earned:{edge}:{failed_check}` | yes — `edge_not_earned` |
| 2 | `src/workflow/runner.rs:43` | stop_class (reason payload) | `edge_not_earned:{edge}:{reason}` | yes — `edge_not_earned` |
| 3 | `src/workflow/runner.rs:97` | other (terminal reason) | `origin_goal_underivable:{error}` | yes — `origin_goal_underivable` |
| 4 | `src/workflow/runner.rs:100` | other (terminal reason) | `origin_goal_underivable:{error}` | yes — `origin_goal_underivable` |
| 5 | `src/workflow/runner.rs:129` | other (terminal reason) | `origin_goal_underivable:invalid action goal: {error}` | yes — `origin_goal_underivable` |
| 6 | `src/workflow/runner.rs:190` | other (terminal reason) | `origin_verify_underivable:{error}` | yes — `origin_verify_underivable` |
| 7 | `src/workflow/runner.rs:194` | other (terminal reason) | `origin_verify_underivable:{error}` | yes — `origin_verify_underivable` |
| 8 | `src/planner/assurance.rs:45` | assurance reason | `data_assurance_{level}` | no |
| 9 | `src/planner/profiles/data/inspection_schema.rs:69` | violation | `inspection_schema_violation:missing_keys:{keys}` | yes — `inspection_schema_violation` |
| 10 | `src/planner/profiles/data/inspection_schema.rs:92` | violation | `inspection_schema_violation:inspection_path:{error}` | yes — `inspection_schema_violation` |
| 11 | `src/planner/profiles/data/inspection_schema.rs:95` | violation | `inspection_schema_violation:inspection_metadata:{error}` | yes — `inspection_schema_violation` |
| 12 | `src/planner/profiles/data/inspection_schema.rs:100` | violation | `inspection_schema_violation:inspection_unreadable:{error}` | yes — `inspection_schema_violation` |
| 13 | `src/planner/profiles/data/inspection_schema.rs:102` | violation | `inspection_schema_violation:invalid_json:{error}` | yes — `inspection_schema_violation` |
| 14 | `src/planner/profiles/data/inspection_schema.rs:128` | violation | `inspection_schema_violation:column_names_missing_headers:{headers}` | yes — `inspection_schema_violation` |
| 15 | `src/planner/profiles/data/inspection_schema.rs:142` | violation | `inspection_schema_violation:input_row_count_mismatch:expected={expected}:reported={reported}` | yes — `inspection_schema_violation` |
| 16 | `src/planner/profiles/data/inspection_schema.rs:163` | violation | `inspection_schema_violation:type_summaries_missing_columns:{columns}` | yes — `inspection_schema_violation` |
| 17 | `src/planner/profiles/data/inspection_schema.rs:193` | violation | `inspection_schema_violation:distinct_values_missing_categorical_columns:{columns}` | yes — `inspection_schema_violation` |
| 18 | `src/planner/profiles/data/inspection_schema/input_table.rs:16` | violation | `inspection_schema_violation:input_metadata:{error}` | yes — `inspection_schema_violation` |
| 19 | `src/planner/profiles/data/inspection_schema/input_table.rs:21` | violation | `inspection_schema_violation:input_unreadable:{error}` | yes — `inspection_schema_violation` |
| 20 | `src/planner/profiles/data/inspection_schema/input_table.rs:48` | violation | `inspection_schema_violation:input_unreadable:{error}` | yes — `inspection_schema_violation` |
| 21 | `src/planner/profiles/data/inspection_schema/input_table.rs:63` | violation | `inspection_schema_violation:input_unreadable:{error}` | yes — `inspection_schema_violation` |
| 22 | `src/planner/profiles/data/inspection_schema/input_table.rs:94` | violation | `inspection_schema_violation:input_header:{error}` | yes — `inspection_schema_violation` |
| 23 | `src/planner/profiles/data/inspection_schema/input_selection.rs:26` | violation | `inspection_schema_violation:input_scan:{error}` | yes — `inspection_schema_violation` |
| 24 | `src/planner/profiles/data/inspection_schema/input_selection.rs:39` | violation | `inspection_schema_violation:input_scan:{error}` | yes — `inspection_schema_violation` |
| 25 | `src/planner/profiles/data/inspection_schema/input_selection.rs:46` | violation | `inspection_schema_violation:multiple_inputs:{inputs}:guidance={guidance}` | yes — `inspection_schema_violation` |
| 26 | `src/planner/profiles/data/checks.rs:97` | violation | `reconciliation_violation:invalid_results_schema:{error}` | no |
| 27 | `src/planner/profiles/data/checks.rs:119` | violation | `claims_binding_violation:invalid_results_schema:{error}` | no |
| 28 | `src/planner/profiles/data/checks.rs:151` | violation | `rerun_consistency_violation:baseline_results:{error}` | no |
| 29 | `src/planner/profiles/data/checks.rs:160` | violation | `rerun_consistency_violation:pipeline_run:{failure_kinds}` | no |
| 30 | `src/planner/profiles/data/checks.rs:166` | violation | `rerun_consistency_violation:pipeline_run_error:{error}` | no |
| 31 | `src/planner/profiles/data/checks.rs:174` | violation | `rerun_consistency_violation:rerun_results:{error}` | no |
| 32 | `src/planner/profiles/data/checks.rs:199` | violation | `reconciliation_violation:excluded_reason_empty:index={index}` | no |
| 33 | `src/planner/profiles/data/checks.rs:220` | violation | `reconciliation_violation:input_rows={input_rows} used_rows={used_rows} excluded_rows={excluded_rows}` | no |
| 34 | `src/planner/profiles/data/checks.rs:237` | violation | `claims_binding_violation:report_not_file:{report_path}` | no |
| 35 | `src/planner/profiles/data/checks.rs:243` | violation | `claims_binding_violation:report_path:{report_path}:{error}` | no |
| 36 | `src/planner/profiles/data/checks.rs:252` | violation | `claims_binding_violation:report_metadata:{report_path}:{error}` | no |
| 37 | `src/planner/profiles/data/checks.rs:259` | violation | `claims_binding_violation:report_size_limit:{report_path}` | no |
| 38 | `src/planner/profiles/data/checks.rs:267` | violation | `claims_binding_violation:report_unreadable:{report_path}:{error}` | no |
| 39 | `src/planner/profiles/data/checks.rs:274` | violation | `claims_binding_violation:claim_count_limit:{report_path}` | no |
| 40 | `src/planner/profiles/data/checks.rs:288` | violation | `claims_binding_violation:{report_path}:{byte_offset}:{raw}` | no |
| 41 | `src/planner/profiles/ingest/source_binding.rs:163` | violation | `source_binding_violation:record={record_index}:field={field}:value={value}` | specific only — one measured date example |
| 42 | `src/planner/profiles/ingest/accounting.rs:130` | violation | `candidate_set_violation:reenumeration:{error}` | specific only — compound CSS and one unknown candidate example |
| 43 | `src/planner/profiles/ingest/accounting.rs:151` | violation | `accounting_violation:duplicate_record_index:{record_index}` | no |
| 44 | `src/planner/profiles/ingest/accounting.rs:168` | violation | `accounting_violation:empty_exclusion_reason:{candidate_id}` | no |
| 45 | `src/planner/profiles/ingest/accounting.rs:180` | violation | `accounting_violation:unaccounted_candidate:{candidate_id}` | no |
| 46 | `src/planner/profiles/ingest/accounting.rs:192` | violation | `accounting_violation:equation:detected={detected}:accepted={accepted}:excluded={excluded}` | no |
| 47 | `src/planner/profiles/ingest/accounting.rs:377` | violation | `candidate_set_violation:{resolution_kind}:{candidate_id}` | specific only — compound CSS and one unknown candidate example |
| 48 | `src/planner/profiles/ingest/accounting.rs:381` | violation | `candidate_set_violation:duplicate_candidate:{candidate_id}` | specific only — compound CSS and one unknown candidate example |
| 49 | `src/planner/profiles/ingest/accounting.rs:400` | violation | `accounting_violation:record_indices:expected={expected:?}:observed={observed:?}` | no |
| 50 | `src/planner/profiles/ingest/runtime.rs:349` | violation | `format_schema_violation:record={record_index}:fields` | no |
| 51 | `src/planner/profiles/ingest/runtime.rs:363` | violation | `format_schema_violation:record={record_index}:field={field}:type` | no |

## Family summary

| Rust-emitted family | Sites | Kind | Registry status |
|---|---:|---|---|
| `edge_not_earned` | 2 | stop_class/reason | covered |
| `origin_goal_underivable` | 3 | terminal reason | covered |
| `origin_verify_underivable` | 2 | terminal reason | covered |
| `data_assurance_*` | 1 | assurance reason | absent |
| `inspection_schema_violation` | 17 | violation | covered |
| `reconciliation_violation` | 3 | violation | absent |
| `claims_binding_violation` | 8 | violation | absent |
| `rerun_consistency_violation` | 4 | violation | absent |
| `source_binding_violation` | 1 | violation | concrete measured member only |
| `candidate_set_violation` | 3 | violation | two concrete measured members only |
| `accounting_violation` | 5 | violation | absent |
| `format_schema_violation` | 2 | violation | absent |
| **Total** | **51** | | |

## Initial registry observation

The inventory intentionally does not change `classes.toml`. It records that
the registry is a campaign-derived adjudication catalog, not currently an
exhaustive vocabulary registry. The proposed bidirectional guard therefore
must distinguish:

1. class IDs declared by Rust and registered by `[[class]].id`;
2. emitted stop-class patterns that must have live Rust producers; and
3. lower-level violation/evidence IDs, many of which are not adjudication
   classes today.

Treating all three sets as interchangeable would turn the first guard run into
a policy migration. E-5a does not silently perform that migration.
