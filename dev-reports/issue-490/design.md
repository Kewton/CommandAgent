# Issue #490 design

Baseline: `b13ef093e462084956bb792e10616d6848e00f40`, including #466,
#478 and #488. The working tree was clean. Read the full reviewed Issue
narrative (R1–R5), frozen campaign decision/report, harness and relevant code.

The defect is in `admission::preserve`: every same-kind/path Verify is
collected as an output owner. A later package reader therefore prevents an
otherwise intact original reader from satisfying the all-owner boundary.

Add a leaf module for supported reader obligations. Initially support the
closed profile package-manifest inspection instruction and the existing exact
Next.js package-check grammar. Require both: neither Verify kind, a path,
instruction wording alone, nor arbitrary JavaScript establishes read-only
behavior. Creation instructions, side effects and unknown external scripts
continue through the existing ownership rule. No runtime metadata is changed.

Match each acquired reader to one distinct proposed reader containing its
instruction, expected result, all required inputs and complete check group.
Keep reader order and both sides of the original executable-owner boundaries,
including split owners. Additional readers do not enlarge output ownership.
Model and host capture, formation, lint, registration, fallback and the existing
three-attempt limit remain authoritative.

Port frozen P01/P02-D input and N01–N07 differences into portable corpus.
First reproduce P02-D failure through normal generation, persistence, before-phase
and registration. Add atomic-duty, acquired before/update/after, non-applicability,
host fallback and runtime pass/fail/missing controls. Preserve registration paths,
commands, producer scope and the artifact-only README rule. Record original build
loss separately as an unresolved preset-conversion diagnostic.

Run focused regressions, related #466/#478/#488 and corpus checks, fmt, clippy and
the complete test suite using the dispatched dedicated target/cache/tmp/runtime.
This is a planner change, not a release or live application evaluation. Commit
only task-owned code, portable fixtures and the three worker contract documents.

The existing flow file was already at its total growth limit with a zero test
budget. Move its unchanged phase-entry sequence (resolve, persist, before-phase,
register) to `phase/phase_entry.rs` and call it from flow. Test checkpoint/observation
hooks live in that leaf; production order, events and error handoffs are preserved.
This reduces the chokepoint rather than raising its baseline.
