# Issue 165 design

## Scope

Keep the fix inside the existing GUI pack-wizard leaf and its provider-free
browser smoke. No pack lifecycle, hash, event, or extension API schema changes
are required. The required Issue 243 predecessor was inspected; its committed
headless provider-usage changes do not overlap this GUI flow and are not merged
into this branch.

## Problem

After a successful stage, editing a member clears the browser's verification
report but does not change the already-staged server bytes. The reached-step
navigation still permits **保存済み bytes を再検証**. That action verifies the
server's older bytes and restores a pinnable report while leaving the edited,
unsaved bytes visible. Pin therefore fixes bytes different from the editor.

## Design

Preserve the meaning of the existing re-verification action: it verifies the
bytes already saved by `SupplyRoot`; it does not silently stage local edits.
After the verify request succeeds, fetch the pack detail through the existing
read API and replace the editor state with those persisted members before
exposing the successful report. Add nearby copy explaining that re-verification
reloads the editor from the saved exact bytes. The server remains authoritative
for verification and pinning, and an unsuccessful reload leaves no pinnable
report.

Convert the returned member map to the existing editor shape in a small local
helper. `assist.yaml` and `eval.yaml` map to their fixed editors; sorted direct
`materials/*.md` entries map to material name/content rows. No new endpoint or
write capability is introduced.

## Verification

Extend the wizard-only browser smoke with the reported route: stage valid
bytes, make a different valid edit, re-verify saved bytes, confirm the editor
was reconciled, pin, then fetch the pinned pack detail and compare every member
with the bytes displayed immediately before pin. Run the focused source guard,
JavaScript syntax check, GUI typecheck/lint/build, and the provider-free wizard
smoke, followed by the repository formatting, clippy, and full Rust test checks
required for production-code handoff.
