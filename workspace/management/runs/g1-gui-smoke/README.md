# G-1 GUI smoke evidence

Date: 2026-08-15 (Asia/Tokyo)

This directory records a managed-Playwright (`1.61.1`) browser probe of the
G-1 trial-run flow. Each case used the release `commandagent` binary, the local
Ollama `qwen3:8b` model for planner and executor, and an isolated copy of
`tests/corpus/apps/test0725_cli_elev_003/fixtures`.

## Result

| basePath | session | Gate 1 negative | terminal | verdict / assurance | events | elapsed |
| --- | --- | ---: | --- | --- | ---: | ---: |
| `/` | `01a004a9-3aee-74f2-9ae9-3b423e7234b2` | HTTP 428 | Gate 4 | `static` / `static` | 97 | 163.110 s |
| `/proxy/commandagent/` | `01a004ab-b3f5-7163-a428-8269a77ce4d9` | HTTP 428 | Gate 4 | `static` / `static` | 125 | 239.163 s |

Both cases returned HTTP 200 for the dashboard, asset, measurement, run-detail,
trial, seven read-only API, and SVG probes. Every internal link used the built
base path. The launch button was disabled before explicit Gate 1 confirmation.
The intentional unconfirmed API probe was rejected with HTTP 428 in both
cases; no unexpected browser console error occurred.

Both real runs honestly stopped at Gate 4. The existing verification floor
rejected model-generated work rather than promoting it: the root case ended on
a failing Python unit-test fixture path, and the proxy case rejected a shell
command used as a natural-language step instruction. The GUI displayed the
resulting D-3c acceptance sheet and D-3d next-action surface. No run was
interrupted or converted to a synthetic success.

The successful smoke removed its temporary runtime after both terminal events.

## Event evidence

- Root events SHA-256:
  `1f62405468515ee3b9ebb55c19c5ec0633eaf811bf48a6b77e9826aae67950b8`
- Proxy events SHA-256:
  `a4d440e526829e5d908c6eeac1e1bd2cd5245ac305ce55977c89d57e7e69aaa2`
- Byte-compatibility fixture SHA-256:
  `6ef3b432bbc3aea044c09196cc068d2a9090e953b69892c24959c23a1ca44e74`

`cargo test --features gui --test gui_read_only_guard --test gui_server`
passed 5 protection tests and 3 server tests. The server test executes the
same fixture once directly and once through the confirmed GUI delegate, then
asserts exact byte equality before parsing the delegated terminal projection.

## Files

- `browser-smoke.json`: aggregate machine result and exact measured values.
- `*-api-log.json`: observed session API status sequence, 428 response, and
  final polling body.
- `*-events.jsonl`: exact session event stream copied after the terminal gate.
- `*-dashboard.png`: dashboard/API/SVG browser state.
- `*-gate-1.png`, `*-gate-2.png`, `*-gate-terminal.png`: one-lap UI evidence.

Invocation:

```text
npm run smoke -- --output ../workspace/management/runs/g1-gui-smoke --commandagent-bin ../target/release/commandagent --model qwen3:8b
```
