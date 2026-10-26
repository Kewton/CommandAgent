# Fixture provenance

`browser-interaction.json` is a synthetic probe output. It is derived from the
S-118b analysis of develop b08dc5a9 (path P1) and is not a recorded run.

The shape: the probe ran in contract mode, found a restart hook in the
pre-start `action_hooks`, observed neither the primary click nor a ranked
candidate start within the window, and then credited the legacy recovery
(`recovery_transition_status: observed`) with `recovery_control: none`. The
restart hook was never reached after start
(`restart_hook_reachable_after_start: false`,
`restart_hook_count_after_start: 0`).

Before IC-26c the release gate counted the pre-start hook and the arbitration
passed the recovery, so this evidence was full. It must now be partial with
`interaction_unverified:restart_not_exercised`, because the restart control was
never exercised. The gate-level behavior (pass becomes partial, failed stays
failed) is covered in
`src/planner/interaction_qualification/restart_attribution_tests.rs`.

The URL is a loopback placeholder; no host paths or secrets are included.
