Recover this failed run by producing and executing a focused ultra plan.

Original goal:
Repair step `implement-pipeline`. Verification failed: data_claims_binding:claims_binding_violation:output/report.md:54:60; claims_binding_violation:output/report.md:73:57; claims_binding_violation:output/report.md:92:3; claims_binding_violation:output/report.md:170:1; claims_binding_violation:output/report.md:193:1; claims_binding_violation:output/report.md:214:1; claims_binding_violation:output/report.md:295:2026; claims_binding_violation:output/report.md:299:-01; claims_binding_violation:outp

Profile: data

Failure scope:
- phase: data-cleaning
- step: implement
- kind: model_stagnation:read_only_loop

Failure evidence:
- read_only_stagnation: write_required reached after read_only_streak=7
- write_required exhausted without Write/Edit to pipeline/main.py: attempts=2/2
- write_required selected_targets=pipeline/main.py,output/inspection.json,output/results.json,output/report.md; selection_reason=required_path

Missing paths:
- none

Missing capabilities:
- none

Verification commands:
- none

Changed paths:
- none

Repair targets:
- pipeline/main.py
- output/inspection.json
- output/results.json
- output/report.md

Required recovery action:
- Inspect the current workspace state first.
- Preserve already useful artifacts.
- Create or repair the missing implementation artifacts.
- Use deterministic verification.
- Do not treat scaffold-only or build-only output as complete.
