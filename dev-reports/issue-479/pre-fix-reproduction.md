# Pre-fix reproduction

Production base: `0dfc6e570b843eb081934a5dc9e4a67fe5930f4b` (#478).
Only test wiring and the initial three focused tests were added before the
production fix. `cargo test issue479 --lib` exited 101 with one passing control
and two expected failures:

- `issue479_pure_import_requires_preclosure_formation`: real Admission returned
  Ready for the two unchanged pure imports; the expected Retry assertion failed.
- `issue479_inventory_classifies_original_and_host_commands`: the saved API
  existence loop was Weak where the intended structural classification was
  asserted. The full real-classifier inventory was saved separately before this
  assertion: Build + four Weak historical commands, plus three Weak package
  checks. App source changes did not change any inline classification.
- `issue479_original_import_remains_weak_at_real_final_acceptance`: passed. The
  real CompletionContract verification remained blocked by the two inline
  imports before/after app changes.

The first draft compared the entire acceptance reason before/after an unrelated
API addition; that addition also changed an independent implementation-evidence
reason. The control was corrected to assert the command-local weak reason and
continued rejection, then rerun before production edits. This harness correction
does not claim the unrelated evidence reason was a product bug.

See [pre-fix-inventory.json](pre-fix-inventory.json) for all eight exact strings
and measured classifications. These are individual classifier measurements;
the historical four repair-obligation observations are not four independent
command measurements.
