# Platform-owned AppSpec schema v0 fixture

This pinned fixture supplies platform-owned schema v0.1 in measurement workspaces. Computed entries declare `entity`; same-entity computed references are evaluated in topological order, while self/mutual cycles and cross-entity references fail closed. Global references are QUEUED.

Replacement is a sealed ceremony: add the new schema beside the old one, verify both pins and validator paths, remove the old schema, then update the canonical pin and `manifest.sha256sums` together. Rerun pin, negative, Rust/Python parity, and adversarial checks. Missing schema remains `community_schema_missing`; mismatched pins remain rejected.
