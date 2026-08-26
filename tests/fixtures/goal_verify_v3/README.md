# goal_verify v3 frozen real workspaces

Registry: `eval/goal_verify/v0/phase6-real-workspaces-v3.json` (decision v3-D4).
One directory per primary cell, one subdirectory per stage.

- create cells: `initial/` is what the current product starts from; `reference/`
  is the frozen completed artifact against which candidate oracles execute.
- fix cells: `before/` contains the defect (product starting point); `after/` is
  the frozen fixed artifact (reference for candidate oracles).

Every `must_contain` line of the registry is asserted by
`tests/eval/test_goal_verify_v3_workspaces.py`. Python cells run with the
standard library and pytest only. The two Next.js cells need `node_modules`,
which is provisioned offline from a vendored tarball produced by
`scripts/goal-verify-v3-provision.sh` (stored outside git; sha256 recorded in
the registry at freeze). `node_modules/` and `.next/` are ignored here.

Ports are reserved per cell: 4173 (`create-ui-copy-style-port-path`),
4174 (`create-build-only-functional`).
