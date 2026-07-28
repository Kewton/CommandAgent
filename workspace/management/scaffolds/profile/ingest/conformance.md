# Ingest conformance

Executable suite:
`cargo test --test ingest_profile_conformance`

- fabricated record absent from source: N2 failed
- shifted field value: N2 failed
- unreported exclusion / silent drop: N3 failed
- schema-extra output: N4 failed
- N1 not executed: static, never partial/full
- frozen candidate set shrunk: N3 failed
- N1–N5 pass: earned and displayed full after admission

Production activation:
`ingest_final_acceptance_production_path_executes_n1_through_n5` runs the
actual final-acceptance entry and requires freeze, N1, N2, N3, N4, N5, and
assurance evidence files.
