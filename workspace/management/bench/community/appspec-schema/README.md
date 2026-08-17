# Platform-owned AppSpec schema v0 fixture

This pinned fixture supplies the platform-owned schema in measurement workspaces. When the real platform schema arrives, replace the schema and pin together, update `manifest.sha256sums`, and rerun the contract pin check. The negative fixtures prove missing schema remains `community_schema_missing` and a mismatched pin is rejected.
