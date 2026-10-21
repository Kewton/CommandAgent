# Issue #478 design

Base: `fdc98a9b7d865b4c2115138765d3ff7b32612dae` (HEAD and fetched
origin/develop match). No required predecessors. Only this worktree is writable
for this task. Parent owns review, exact-commit UAT/CI and publication.

## Problem and deterministic control

Replay saved contract-wiring attempts 2 and 3 through the actual planner with a
scripted ChatClient, without a model call. Preserve the saved model proposals as
corpus inputs. The smoke instruction is 587 characters, SHA256
`75c69f8375ad4c8492158c3f65745a6532a82f26850cab51901015e8faa492e0`.
The existing host adds package.json and broad Profile guidance to the last
Implement, then admission pins that combined step. The next proposal separates
smoke from configuration, but preset conversion replaces the configuration
instruction/kind with a package-only Verify. Both the combined-text comparison
and lost executable Profile duties must be addressed. Record the expected
pre-fix failure before changing production logic.

## Intended change

Capture host augmentation at its actual mutation boundary, retaining model
instruction, outputs, result and normalized commands independently from host
additions. Keep the existing combined `original_scope` event field and add
explicit model/host scopes for diagnostics and retry feedback. Retain strict
owner-specific preservation for model duties; host additions can move only to a
responsible executable owner with their full instruction/outputs/result.

Do not reinterpret a broad Profile implementation duty as satisfied by a few
package checks. Preserve such owners through preset conversion and runtime
precheck boundaries. Keep dedicated setup conversion for its existing narrow
scope. Require original verification after its corresponding producers and
preserve original boundary dependencies even when a Verify has no output paths.

Implement in leaf modules with minimal driver wiring. Do not change persisted
Recovery contracts/hashes, recovery/planner budgets, event names/schema version,
verification classifiers, acceptance gates, guardrail baselines or live state.

## Verification

The saved split must pass with smoke text only on the smoke owner and complete
host guidance on the application/configuration owner. Controls must refuse
deleted assertions/checks, changed expected results, missing outputs, unrelated
owners, premature verification and missing Profile duties. Include fallback
checks and existing #466/#465 obligation/confirmation regressions.

Run focused Rust tests, corpus checks, fmt, clippy, full cargo test, release build
and local version/hash. Record worker-local results separately from pending
parent exact-head UAT/CI and integrated release verification.
