# Issue #492 design

Baseline: `d51cc6ed9e04f8d9747d015ff788845ea1b1e59b` (includes #478, #488,
and #490). The reviewed AC1–AC7 and dispatched worker brief define the scope.

Preset setup conversion must not replace explicit executable duties with a
configuration-only subset. Preserve Next.js steps with any check outside the
existing exact profile predicates for the current goal, including their
instruction, kind, expected result, paths and ordered commands. Nonempty checks
with a non-pass expected result are also preserved. Empty pure setup and exact
passing package-only setup still receive the existing profile checks (including
the declared package input that sanitation may remove). Unknown commands remain subject to existing sanitation, policy,
Admission and runtime verification; preservation does not authorize them or
declare them equivalent. Existing profile implementation duties retain their
previous additive setup checks; the new fallback is applied only when that
existing retention path does not apply. Other profiles retain their existing
canonicalization.

Do not add package.json to a build step's expected_paths: the original input/output
declaration remains unchanged, package commands already read their explicit input,
and a build is not a #490 read-only package observer. Keep profile configuration
verification in the original package reader / explicit setup duties. Verify the
resulting runtime precheck decision and execution, rather than assuming that an
unchanged plan proves execution. No retry, reader exemption, schema, live state,
or guardrail baseline changes are planned.

First replace the historical loss assertion with a preservation assertion and
record its failure on unchanged production source. Retain the frozen historical
JSON and its baseline provenance. Add portable whole-plan controls with an
acquired package reader, implementation, and trailing original build. Observe
parse, host augmentation, sanitation, conversion, Admission, save/reload,
before_phase and registration. Compare contracts separately (normalization,
filtering, required paths and producers), including additions and no-op replay.

P1 and real runtime proposals explicitly contain the original mixed array from
the frozen sanitized snapshot. The normal sanitizer does not add the guard to
a bare `npm run build` in these new full plans; that earlier observation is not
assumed. The actual parsed/augmented/sanitized/converted stages are captured.

Each N1–N4 uses fresh state and the ordinary acquisition prefix, then changes only
one later proposal: remove build, move build before implementation, change its
expected result, or remove another original check. Verify targeted rejection,
three-attempt bound, fallback checks and unchanged registration on failure.

Use fixed real Next.js source/configuration/lockfile and versions for good, broken
TypeScript, missing dependencies, and bad profile controls. Isolate applications,
state and generated outputs on the dispatched SSD paths; record the actual
post-implementation command, immediate source hash, order, count, exit and error.
Use fixed responses with no repair before the initial observation. Passing build
means only passing build, not completion of the business application.

Run focused controls, #478/#488/#490 regressions, corpus and guardrails, then fmt,
clippy and the complete Rust suite. Record exact commands, source/binary identity,
environment and SSD evidence in the two final worker reports; commit only task
code, fixtures and the three reports.
