# E-5c evidence and event shape audit

Status: initial audit complete (2026-07-29); migration not yet applied.

This is the additive-migration design record for E-5c. Existing files under
`workspace/management/runs/` were read only. The only run-ledger write in the
audit commit is this new file.

## Scope and method

The inventory follows every production JSON writer and the persisted examples
for the seven registered evidence families: `E`, `F`, `I`, `C`, `N`, `circle`,
and `workflow`. “Fields” below means the serialized top-level fields before
E-5c. Nested fields used by a transverse consumer are called out explicitly.

The three transverse consumers are:

- collector: `workspace/management/scripts/calibration_corpus.py`
- sheet: `workspace/management/scripts/acceptance_sheet.py`
- classify: `workspace/management/scripts/classify_runs.py`

Consumer marks are `R` (reads the listed fields), `G` (generic file/event
discovery only), and `-` (does not consume that shape).

## Evidence inventory

| family | file kind / production writer | current top-level fields | collector fields | sheet fields | classify fields | epoch before E-5c |
|---|---|---|---|---|---|---|
| E | `pipeline-run.json` / `minimal_loop/pipeline_probe.rs` | `capability_id,status,ok,outcome,command,duration_ms,exit_code,stdout,stderr,artifacts,isolation,failure_kinds,capture_warnings,python_error_extraction?` | - | `capability_id,command,exit_code|status,outcome|stderr` | text fallback only | none |
| E | `inspection-schema.json` / `profiles/data/inspection_schema.rs` | `capability_id,status,ok,inspection_path,input_path,failure_kinds` | - | `capability_id` (G) | text fallback only | none |
| E | `results-schema.json` / `profiles/data/checks.rs` | `capability_id,status,ok,results_path,error` | - | `capability_id` (G) | text fallback only | none |
| E | `reconciliation.json` / `profiles/data/checks.rs` | `capability_id,status,ok,input_rows,used_rows,excluded,excluded_rows,equation,failure_kinds` | - | `capability_id` (G) | text fallback only | none |
| E | `claims-binding.json` / `profiles/data/checks.rs` | `capability_id,status,ok,report_paths,claims,failure_kinds` | R: `claims[].raw,matched_result_value,value,ok,matched,nearest_miss` | R: `capability_id,claims[].raw,matched_result_value,rounded_result_value,ok,matched` | text fallback only | none |
| E | `rerun-consistency.json` / `profiles/data/checks.rs` | `capability_id,status,ok,entry,pipeline_run_ok,baseline_results,rerun_results,failure_kinds` | - | `capability_id` (G) | text fallback only | none |
| E | `data-assurance.json` / `profiles/data/runtime.rs` | `status,assurance,checks,reasons` | - | G | text fallback only | none |
| F | `fix-*-before[-attempt-*].json`, `fix-*-after.json`, `fix-*-regression-*.json` / `fix_runtime/evidence.rs` | `schema_version,intent,contract_version,contract_ref,requirement_id,binding_id,stage,expected,lineage,epoch,run_id,executed,outcome,reason,failure_classification?` | - | R: filename plus `stage,executed|outcome,expected` | text fallback only | `epoch` is a run-local monotonic evidence ordinal |
| F | `fix-*-adjudication.json` / `fix_runtime/evidence.rs` | `schema_version,intent,contract_version,contract_ref,run_id,adjudication,evidence` | - | G | text fallback only | nested F observations carry epoch |
| I | `investigation-run.json` / `investigation_runtime.rs` | `schema_version,intent,contract_version,contract_ref,requirement_id,reproducer,reproducer_lineage?,stage,expected,epoch,executed,outcome,stdout,stderr,failure_classification?` | - | R: `reproducer.command?` legacy fallback to `reproducer`, then `outcome|status` | text fallback only | `epoch` is present |
| I | `investigation-binding.json` / `investigation_runtime.rs` | `schema_version,intent,contract_version,contract_ref,requirement_id,claims` | R: `claims[].raw|quote,matched_result_value|value,ok|matched,nearest_miss` | R: `claims[].matched,quote|value|raw` | text fallback only | none |
| C | `cli-case-binding.json` / `profiles/python_cli/argv_probe.rs` | `entry,cases` | - | G | text fallback only | none |
| C | `cli-probe.json` / `profiles/python_cli/argv_probe.rs` | `capability_id,status,ok,binding_intact,c1_ok,c4_ok,binding,observations,output_claims,failure_kinds` | R: `output_claims[].claim,matched,nearest_miss,observation.stdout.text,source` | `capability_id` (G) | text fallback only | none |
| C | `help-binding.json` / `profiles/python_cli/help_binding.rs` | `capability_id,status,ok,help_observation,help_options,bindings,implementation_to_help_scope,failure_kinds` | R: `bindings[].direction,option,ok,nearest_miss,observation.stderr.text|stdout.text` | `capability_id` (G) | text fallback only | none |
| C | `cli-assurance.json` / `profiles/python_cli/runtime.rs` | `status,assurance,evidence.checks,reasons` | - | G | text fallback only | none |
| N | `ingest-candidate-freeze.json` / `profiles/ingest/accounting.rs` | `capability_id,selector,record_format,snapshots,candidates` | - | `capability_id` (G) | text fallback only | none |
| N | `ingest-probe.json` / `profiles/ingest/runtime.rs` | `capability_id,status,ok,candidate_freeze_path,snapshot_ids,required_artifacts,execution,failure_kinds` | - | `capability_id` (G) | text fallback only | none |
| N | `source-binding.json` / `profiles/ingest/source_binding.rs` | `capability_id,status,ok,records_path,bindings,failure_kinds` | - | `capability_id` (G) | text fallback only | none |
| N | `candidate-accounting.json` / `profiles/ingest/accounting.rs` | `capability_id,status,ok,selector,detected,accepted,excluded_by_reason,equation,candidate_ids,candidate_id_resolutions,failure_kinds` | - | `capability_id` (G) | text fallback only | none |
| N | `format-schema.json` / `profiles/ingest/runtime.rs` | `capability_id,status,ok,declared_fields,record_count,failure_kinds` | - | `capability_id` (G) | text fallback only | none |
| N | `rerun-consistency.json` / `profiles/ingest/runtime.rs` | `capability_id,status,ok,compared_paths,failure_kinds` | - | `capability_id` (G) | text fallback only | none |
| N | `ingest-assurance.json` / `profiles/ingest/runtime.rs` | `status,assurance,evidence.checks,reasons` | - | G | text fallback only | none |
| circle | `workflow-circle.json` / `workflow/evidence.rs` | `schema_version,workflow,origin,reproducer_suggestion?,edges,nodes,verdict,reason` | - | R: `origin,edges[].edge|checks,nodes,verdict,reason` | text fallback only | `edges[].checks["E-C"]` is an epoch-integrity verdict, not a timestamp |
| workflow | `workflow-events.jsonl` / `workflow/orchestrator.rs` | event-dependent; see event table below | - | R: events and event-specific fields | R: terminal event fields; text fallback otherwise | `workflow_started` and `workflow_adjudicated` have no epoch |

## Event inventory

The profile evidence checks primarily persist JSON files. Their shared
acceptance projection is emitted through `profile_behavior_probe` and
`ultra_final_acceptance`; the family-specific event surface is deliberately
small.

| family | event | persisted fields relevant to evidence |
|---|---|---|
| E/C/N | `profile_behavior_probe` | `profile,status,reasons,evidence_path` |
| E/C/N | `ultra_final_acceptance` | generic acceptance fields plus `profile_behavior_probe_status`, `profile_behavior_probe_reasons`, `profile_behavior_probe_evidence_path` |
| F | `fix_evidence_recorded` | `intent,contract_version,contract_ref,requirement_id,binding_id,stage,expected_polarity,lineage,epoch,run_id,executed,outcome,reason,evidence_path,failure_classification?` |
| F | `ultra_final_acceptance` | `intent,assurance,reason,requirement_statuses,evidence_path` |
| I | `investigation_adjudicated` | `intent,assurance,reason,requirement_statuses,evidence_paths` |
| I | `ultra_plan_complete` | `intent,assurance,reason` |
| workflow | `workflow_started` | `entry,origin`; no epoch |
| workflow | `workflow_edge_fired` | `edge,checks` |
| workflow | `workflow_node_started` | `node,intent` |
| workflow | `workflow_node_run_created` | `node,run_id,run_dir,model,provider` |
| workflow | `workflow_reproducer_bound` | `basis,command,lineage` |
| workflow | `intent_resolved` | `intent,node` at the outer stream; child stream also records `workflow_node,profile,model,provider` |
| workflow | `workflow_adjudicated` | `verdict,reason?`; no epoch |

`classify_runs.py` gives terminal fields priority. It reads only
`run_stop`, `tui_command_stop`, and `ultra_final_acceptance` fields
`failure_kind`, `stop_class`, `reason`, `stop_reason`, and
`final_acceptance_status`. If no terminal text exists, it falls back to the
entire tracked text corpus. An envelope must not broaden terminal
classification.

## Findings and migration disposition

1. No producer currently emits a common envelope.
2. `claims` already exists at the E and I top level, but the element schemas
   differ. C and N keep claim/binding material under `output_claims`,
   `bindings`, or `bindings`. Reusing the old top-level key would therefore
   change meaning; E-5c must add a nested envelope.
3. `nearest_miss` is an object in E/C2/N, a string in C3, and is named
   `nearest` in I. The envelope must normalize copies while retaining these
   original fields unchanged.
4. F and I observations have an epoch, while the E/C/N evidence families do
   not. The workflow circle’s E-C check is not a wall-clock epoch. A common
   envelope needs an independently defined epoch field.
5. The collector supports E2/I2/C2/C3 legacy shapes but no N nearest-miss
   shape. The sheet has detailed E/I/F rendering and only generic discovery
   for C/N. Classification intentionally consumes terminal protocol rather
   than semantic evidence. These are the expected transverse-followup gaps
   addressed by commit 3, not an incompatible discovery.
6. Historical fixtures and run evidence omit the new shape. Every consumer
   therefore requires an explicit legacy fallback; historical files must not
   be rewritten.

No finding changes the requested additive migration or requires adjudication,
so implementation may continue.

## Planned additive envelope

Every newly written evidence JSON object will gain one top-level
`evidence_envelope` object:

```json
{
  "evidence_envelope": {
    "envelope_version": 1,
    "family": "E",
    "kind": "claims_binding",
    "epoch": 1780000000,
    "claims": [],
    "nearest_miss": [],
    "source_refs": ["output/report.md"]
  }
}
```

All seven inner fields are present. `claims` and `nearest_miss` are normalized
arrays and are empty when the evidence kind has no such material. The original
top-level fields remain the authoritative family-specific representation and
are neither renamed nor removed. `source_refs` contains workspace-relative
paths only. `epoch` is Unix seconds at evidence issuance; it does not replace
the existing F/I run-local ordinal.

## Commit-3 completion record

Pending. This section will be finalized with the consumer matrix, family guard
result, and historical-fallback proof.
