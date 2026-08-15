# G-1 GUI workspace-isolation acceptance

Date: 2026-08-16 (Asia/Tokyo)

This immutable record covers the remediation of GUI trial session
`01a005e0-ddad-74f1-9987-68ef0883409a`. The incident event stream was retained
outside this run record with SHA-256
`237f1a36a842dbc8306a8d5a99f8b37d659b85942ca5f0648fc288fb250c8f80`.
Repository-root artifacts created by that trial were moved to the recoverable
quarantine `/private/tmp/commandagent-gui-incident-01a005e0-quarantine`; no
historical run record was rewritten.

## Implemented boundary

- Trial execution is disabled unless an existing `--execution-root` is supplied.
- The canonical trial workspace must be disjoint from the repository root and is
  revalidated before every Trial API operation and CLI delegation.
- All Trial APIs require a runtime bearer token. Mutating requests additionally
  require an allowed same-host or explicitly configured origin.
- One process may delegate at a time. A non-terminal prior process puts the
  workspace into recovery-required state after restart.
- Gate 1 shows the canonical filesystem write boundary and requires explicit
  confirmation. The token remains in browser memory only.
- The GUI server still delegates exclusively to the existing CLI argument path;
  provider and runner calls remain prohibited by the six-case guard.
- Generated Next.js `tsconfig.json` coverage is constrained to the selected app.
  An unbound compile error is forbidden as an automatic edit target only when it
  crosses a nested `package.json` project boundary; same-app route attachment
  repair remains allowed.

## Verification

- Repository CI script: exit 0; Rust 2,087 passed / 34 ignored, Python 210
  passed, 28 repository skills validated, shellcheck green.
- GUI job equivalent: basePath audit green, TypeScript green, root and proxy
  static builds green, GUI guard 6/6, GUI integration 7/7, GUI clippy green.
- CLI delegation fixture: direct CLI and GUI-delegated `events.jsonl` bytes are
  identical.
- Release binaries: `commandagent 0.1.0` and `gui_server` built with the `gui`
  feature.

## Managed-browser smoke

Managed Playwright 1.61.1 drove both `/` and `/proxy/commandagent/`. Each case
loaded the dashboard, all seven read APIs, the SVG, assets, measurements, run
detail, and Trial page. Each showed Gate 1, rejected an unconfirmed launch with
HTTP 428, delegated after confirmation, and reached Gate 4 with no unexpected
console error.

The local qwen3:8b model produced invalid plan JSON in both small CLI trials, so
the product correctly terminated at `static/failed` instead of claiming full
acceptance. This is an honest model-output failure, not a transport or GUI
failure. The exact terminal stop reasons remain in the temporary raw evidence;
only hashes and scrubbed status fields are committed here.

| Base path | Session | Elapsed | Gate | Verdict | Events |
| --- | --- | ---: | --- | --- | --- |
| `/` | `01a00671-a7d5-79e0-b096-11afd008544b` | 182.154 s | Gate 4 | static / failed | `sha256:e16bc22da184f055fcf9c111663b4b856384e131b0cf6b8e00810139d733ba35` |
| `/proxy/commandagent/` | `01a00674-6f55-73d0-9894-b13b1fee260f` | 103.001 s | Gate 4 | static / failed | `sha256:700c62935a1741385b4a4399023ff232b94166db4d18ce21b2346131f0fd4355` |

Screenshots are paired by base path and Gate 1 / terminal state. Raw API and
event logs were not committed.

## Cross-scenario isolation matrix

Three canonical Next.js goals were run sequentially in three explicit temporary
workspaces with the release CLI and qwen3.6:27b-coding-nvfp4. No run changed the
CommandAgent repository or its `gui/` project.

| Scenario | Elapsed | Result | Events SHA-256 |
| --- | ---: | --- | --- |
| Space | 395.689 s | honest partial failure: route/restart evidence unresolved | `7add8f6803c874d5c73680564efcd601ce99ded42dd2c5b868a4a2fd4ebff7c2` |
| Breakout | 860.280 s | full success, four phases | `865e682dddd60c2d40190aee4be9a81504e39aa49c5d7c7b95195582e97c79cd` |
| Quiz | 648.975 s | full success, four phases | `bce30efbccdd99e5bcc377a64253b6fe512c633c49f4f54492a61dbe4715d309` |

The Space run also exposed an existing recursive `snapshots/latest` evidence
path. It stayed inside the disposable trial workspace and is recorded as a
follow-up observation; this remediation does not alter snapshot semantics.
