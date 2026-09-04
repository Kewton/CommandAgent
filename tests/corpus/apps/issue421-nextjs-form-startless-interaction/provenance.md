# Fixture provenance

This synthetic fixture preserves the interaction shape reported by Issue #421
for GUI Trial session `01a06b3b-0a60-7040-98ab-70d97023722a`: a successful
Next.js form interaction with an input-driven state change on a visible surface
and no start control. It is session-equivalent regression evidence, not a copy
of the original runtime artifact.

The companion missing-input fixture removes the state-change observation so
the existing `interaction_detail_missing` boundary remains covered.
