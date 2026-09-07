# Issue #441 CI resolution summary

The CI and acceptance failures came from an integration-test expectation that
sliced 64 bytes unconditionally. A short host temporary path made the original
command 49 bytes long. The unchanged test reproduced the same panic locally
with `TMPDIR=/tmp` before this correction.

The expectation now compares exactly `min(64, original byte length)` bytes and
also checks the recorded full byte count. The test retains the real workspace
`cd` normalization/execution control, placeholder rejection before normalization,
the original whole-command SHA256 assertion, and the existing UTF-8 split-byte
case.

A new integration test exercises 13-byte, exactly 64-byte, and two 66-byte
commands independent of host temporary path length. It checks rejection, private
evidence permissions, full byte count, exact prefix contents/length, and the
whole-command hash. The longer commands share the exact 64-byte prefix but have
different tails; their hashes must differ from the prefix-only command and each
other.

Only `tests/issue441_placeholder.rs` and the three new CI resolution reports are
changed. Production policy and evidence, corpus contracts, dependencies, and
historical reports remain unchanged. No ignores were added. See the separate
CI resolution verification report for final checks and source identity.
