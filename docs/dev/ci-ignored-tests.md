# CI ignored-test inventory

The shared CI suite runs every non-ignored Rust target. The 32 ignored tests
remain opt-in for explicit environmental reasons; none is silently filtered by
the workflow.

| Category | Count | Reason / opt-in path |
| --- | ---: | --- |
| subprocess child entry points | 12 | Invoked by their parent harnesses with controlled environment variables; running them directly would bypass the parent assertions. |
| browser and behavioral probes | 3 | Require a browser/probe environment that hosted CI does not provide. |
| provider HTTP smoke tests | 8 | Require live provider endpoints and credentials. |
| pseudo-terminal tests | 7 | Require a real PTY; run with `just test-pty`. |
| remaining environment harnesses | 2 | Fake-server and host-contamination helpers are run by their owning conformance tests. |

To audit the inventory after adding or removing an ignored test:

```bash
cargo test --all-targets -- --list --ignored
```
