# Issue #492 implementation

The preset conversion retention leaf now preserves explicit Next.js duties when
any declared command is outside the existing exact profile predicates for the
current goal, or when nonempty checks expect failure. It retains instruction,
kind, expected result, original paths and ordered checks. It does not recognize
new commands or broaden semantic equivalence. Existing profile implementation
retention still runs first, including its additive configuration checks.

Empty setup and exact passing package-only setup keep their existing conversion.
Unknown checks remain available to the shared normalizer, policy, Admission and
runtime verifier. Build is not classified as a reader. No retry count, fallback,
contract registration, acceptance gate, environment policy or event schema changed.
Production behavior changes are confined to `implementation_duties.rs`.

The original build has no expected_paths, and this fix preserves that declaration.
It therefore adds no package.json output owner or required path. Its Verify
precheck remains inapplicable, so the normal executor runs the complete build
verification after implementation. The explicit package reader retains its own
package.json input and all port/build-script checks. Normal duplicate-path
sanitation and pure package conversion still restore the reader input, preserving
#490's ownership behavior. Existing profile implementation path augmentation is
unchanged and has a dedicated regression.

Test-only observers in the parser, Admission augmentation and conversion leaves
record actual plans without changing their contents or decisions. The existing
#490 phase-entry replay checks save/reload, before_phase, normal registration and
live/disk contract equality. P1 asserts every step field at each boundary; the
build's mixed command array comes explicitly from the historical sanitized
snapshot, rather than assuming the new plan's sanitizer adds the package guard.
The original loss snapshot remains byte-identical and tied to the baseline SHA.

P1 checks both contract additions (verifier command, paths and producer) and a
subsequent no-write registration. N1–N4 independently acquire the same obligations
through the normal first-retry boundary, then vary one later proposal. They check
targeted refusal, unchanged acquired scopes, the three-proposal limit and atomic
non-registration. Existing #478/#488/#490 tests retain fallback, freezing,
final-acceptance and reader coverage.

The portable real fixture fixes Next.js/TypeScript/dependencies and source. Its
opt-in test enters the actual preset's core phase via ordinary prepare and the
ordinary phase executor, on an explicitly provisioned workspace. A transparent
npm observer records source/config hashes and the exact executable, command,
phase/step, ordering, count, exit and compiler output. A sidecar carries observer
paths and SSD TMPDIR without changing product environment filtering. Good source,
broken TypeScript, missing dependencies and two profile defects are independent
controls. Fixed operations preserve source through the first build observation;
no successful repair or whole-application completion is claimed.

Only task-owned source, tests, portable fixtures and the three worker reports are
committed. Raw outputs stay in the dispatched SSD runtime directory. Historical
reports, the frozen loss JSON, live `.anvil`, HOME and external Issue/PR state are
unchanged. Final commands, outcomes and binary identity are in `verification.md`.
