# Session 01a06793 recovery hardening verification

Date: 2026-09-04

## Scope

- Bind automatic Recovery to the registered run-level completion contract.
- Require stable pre/post observer identity and preserve control on mismatch.
- Observe Next.js behavioral capabilities without treating an unavailable probe as an app failure.
- Reject false persistence based only on a cleared draft field.
- Reject optimistic UI success when the backing HTTP mutation returns a non-success status.
- Detect missing Next.js App Router mutation methods and unchecked mutation responses.
- Project control/treatment/promotion resolution consistently to the GUI.

## Incident reproduction

The frozen session artifacts were copied to scratch before execution. The control app's build and
live POST/PATCH/DELETE behavior passed. The treatment's client-shaped PUT and DELETE requests both
returned HTTP 405, and persisted task tokens remained after both recovery cycles. The new static
profile check rejects the treatment because the client calls DELETE while the registered route does
not export DELETE. No historical session evidence was modified.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test`: passed, including all library, integration, guardrail, and doc tests; configured
  ignored/live tests remained ignored.
- `cargo test --test generality_guardrails`: passed (10/10) without raising a baseline.
- `cargo test --features gui --test gui_server recovery_run_rejects_stale_drift_pending_directive_and_treatment_rejection`: passed.
- `cargo test --test corpus_regression session_01a06793_api_method_mismatch_fails_profile_acceptance`: passed as part of the full suite.
- `npm run lint`: passed.
- `npm run typecheck`: passed.
- `npm run smoke:session-index`: passed outside the filesystem/network sandbox for both `/` and
  `/proxy/commandagent/`; both reports returned `ok: true`, explained the rejected treatment, and
  disabled the stale Recovery Plan action.
- Next.js guidance compatibility checks for Space, Breakout, and Quiz plus generated corpus
  regression passed.

## Honest-failure result

The change does not promote the rejected treatment or revise frozen evidence. A treatment can only
replace control after the same registered observer authority passes post-Recovery. Probe
infrastructure failures remain unavailable/unverified, while HTTP 405 and route-method mismatches
remain product failures with their method and status preserved.
