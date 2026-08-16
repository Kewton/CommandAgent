# Management GUI

The CommandAgent GUI projects repository evidence into four read-only views
and adds one confirmed trial-run view. The dashboard, run detail, admitted
assets, and measurement reports remain file projections. Trial run can launch
an existing non-interactive `commandagent` CLI process after Gate 1; the GUI
does not call providers or runners in process. The server binds only to
`127.0.0.1` and does not use forwarded-host headers.

## Prerequisites

- Rust 1.88 or later
- Node.js 20.9 or later
- npm with the lockfile committed under `gui/`

Install exactly the pinned Node dependency graph:

```bash
cd gui
npm ci --include=dev
```

## Serve at `/`

Build the static export and start the optional GUI binary from the repository
root:

```bash
cd gui
GUI_BASE_PATH=/ npm run build
cd ..
export GUI_TRIAL_TOKEN="$(openssl rand -hex 32)"
cargo run --features gui --bin gui_server -- \
  --port 4173 \
  --base-path / \
  --static-dir gui/out \
  --repository-root . \
  --execution-root /path/to/trial-workspace \
  --commandagent-bin target/release/commandagent
```

The runtime-only token is not compiled into the static export. Enter it in the **Trial
access token** field; the page keeps it only in memory and sends it as a Bearer
token in `X-CommandAgent-Trial-Authorization` to Trial APIs. This dedicated
header survives same-origin proxies that intentionally remove the generic
`Authorization` header. The server still accepts `Authorization: Bearer` for
direct-client compatibility. If `--execution-root` is omitted, the dashboard remains
available but all Trial APIs fail closed with HTTP 503. If `--execution-root`
is present without `GUI_TRIAL_TOKEN`, the server refuses to start.

Open `http://127.0.0.1:4173/`.

The `gui` Cargo feature is not a default feature. Ordinary CommandAgent
builds, tests, and the product binary therefore do not compile the Axum GUI
target or its optional dependencies.

## Serve below a reverse-proxy path

The build-time path and server path must describe the same prefix. The build
accepts an optional trailing slash; the server uses the canonical form without
one:

```bash
cd gui
GUI_BASE_PATH=/proxy/commandagent/ npm run build
cd ..
export GUI_TRIAL_TOKEN="$(openssl rand -hex 32)"
GUI_TRIAL_ALLOWED_ORIGINS='https://admin.example.com' \
cargo run --features gui --bin gui_server -- \
  --port 4173 \
  --base-path /proxy/commandagent \
  --static-dir gui/out \
  --repository-root . \
  --execution-root /path/to/trial-workspace \
  --commandagent-bin target/release/commandagent
```

An nginx location can preserve that prefix when proxying to the loopback
listener:

```nginx
location /proxy/commandagent/ {
    proxy_pass http://127.0.0.1:4173;
}
```

Do not derive the GUI origin or prefix from `X-Forwarded-*`. Set the prefix
explicitly at build and startup.

`GUI_TRIAL_ALLOWED_ORIGINS` is a comma-separated allowlist for reverse-proxy
origins. Same-host browser requests are accepted automatically. A valid token
does not replace upstream authentication: when exposing Trial through
Cloudflare or another tunnel, require an administrator access policy at that
proxy as well.

The execution root must already exist and must be disjoint from the repository
root. The server rejects the repository itself, its parents, its children, and
symlink aliases of those paths. It canonicalizes the workspace again before
initial dispatch and D-3d continuation. Use one dedicated project workspace;
the GUI permits only one delegated process in that workspace at a time.

## Trial run: Gate 1 through Gate 3/4

Open **Trial run** and enter a goal, admitted profile, provider, and exact
planner/executor model pins.

The **LM Studio** provider selection maps to the existing CLI spelling
`--provider lm-studio`; it is not an alias or a GUI-side provider
implementation. Enter the exact model identifier exposed by LM Studio. The
delegated CLI uses its default `--lm-studio-host http://localhost:1234` and
inherits `LM_STUDIO_API_TOKEN` from the GUI server environment when LM Studio
authentication is enabled.

1. Enter the runtime Trial token, then select **Check contract and price**.
   Gate 1 shows the frozen contract checks, full rate and sample count, any
   recorded mean duration/cost, and the canonical filesystem write boundary.
2. Select the confirmation checkbox. The launch button stays disabled until
   this explicit confirmation, and the API independently requires the exact
   card hash.
3. Select **Confirm and delegate to CLI**. The server starts the existing
   non-interactive boundary command. Progress is reconstructed by reading that
   session's JSONL events; there is no GUI state database.
4. During Gate 2, use **Recent events** or **Browse artifacts** to inspect the
   bounded, read-only session evidence without leaving the GUI. At Gate 3 or
   Gate 4 the artifact inventory opens automatically; select `summary.md`,
   recent `events.jsonl`, or an acceptance-related text file to investigate a
   failure. You may then end without another run, or persist an additional D-3d
   instruction. A D-3d instruction is credential-scrubbed, exact-byte hashed,
   displayed, and must be confirmed before the existing continuation path is
   delegated.

There is no cancel, interrupt, phase-edit, or gate-override control while a
session is running. Use the existing CLI/runtime operating procedures for
external process management. GUI confirmation cannot lower, replace, or
satisfy the contract checks.

## API

The evidence routes are same-origin GET requests below the selected base path:

| Route | Repository projection |
| --- | --- |
| `api/runs` | Run directory index |
| `api/runs/{id}` | Acceptance sheet and evidence inventory |
| `api/runs/{id}/evidence?path=…` | One bounded text evidence file |
| `api/bands` | Formal band summaries |
| `api/maps` and `api/maps/score-time.svg` | Score/time map inventory and SVG |
| `api/packs` | Pack versions and exact-byte pins |
| `api/contracts` | Contract documents |
| `api/suites` | Measurement suite definitions |
| `api/reports` and `api/reports/view?path=…` | Measurement report archive |

Paths are canonicalized below their allowed inventory root, symlinks are not
followed during listing, and individual text views are capped at 1 MiB.

Trial run adds these bounded routes:

Every route in this table requires
`X-CommandAgent-Trial-Authorization: Bearer <GUI_TRIAL_TOKEN>` (or the legacy
direct-client `Authorization` form). POST requests also require a same-host Origin or an origin admitted by
`GUI_TRIAL_ALLOWED_ORIGINS`.

| Route | Operation |
| --- | --- |
| `POST api/session-proposals` | Render a deterministic Gate 1 identity and measured price tag |
| `POST api/sessions` | Require the exact Gate 1 hash, then delegate to the configured CLI binary |
| `GET api/sessions/{id}` | Read events and artifacts to project phase, gate, and terminal verdict |
| `GET api/sessions/{id}/artifacts` | List up to 256 text artifacts below the Trial run root |
| `GET api/sessions/{id}/artifacts?path=…` | Read one canonical, non-symlink text artifact up to 1 MiB |
| `GET api/sessions/{id}/events?tail=N` | Read the last `1..=2000` event lines, with a 1 MiB response limit |
| `POST api/sessions/{id}/directives` | Apply the existing credential scrub and persist a hashed D-3d proposal |
| `POST api/sessions/{id}/directives/{hash}` | Require that exact proposal, then delegate the existing continuation plan |

The two POST dispatch routes cannot accept an unconfirmed identity. The sole
process surface executes `commandagent` directly without a shell; provider and
runner calls are forbidden in the GUI server by the protection audit.

The event-tail reader scans backward, so it remains useful after the complete
stream exceeds the 4 MiB status-polling limit. Artifact listing uses the same
text-extension allowlist, depth-four walk, skipped directories, ordering, and
entry cap as the repository run viewer. All Trial file routes require a
canonical session UUID and the runtime token; no session-listing capability is
implied.

Delegated stdout and stderr are intentionally not saved by this GUI path. The
CLI-owned `events.jsonl` and `summary.md` are the structured diagnostic records,
and the GUI server only reads them. If an unstructured log is later required,
it must be introduced as a CLI-owned output contract rather than a GUI-server
write.

## Two-basePath browser smoke

The smoke runner reuses the Playwright package managed by CommandAgent's
existing interaction probe. It neither installs Playwright nor changes the
live `.anvil/` namespace. By default it reads:

`~/.anvil/tools/interaction-probe/node_modules/playwright`

First build the product binary used by the delegate:

```bash
cargo build --release --bin commandagent
```

Then run a real local-model lap for both `/` and
`/proxy/commandagent/`. The runner copies the small Python CLI corpus fixture
to an isolated temporary workspace. For each base path it records the
dashboard/API/SVG probes, Gate 1 before and after confirmation, Gate 2,
Gate 3/4, in-page events and summary viewing, the session event stream and its
SHA-256, and an API log:

```bash
cd gui
npm run smoke -- \
  --output ../workspace/management/runs/g1-gui-smoke \
  --commandagent-bin ../target/release/commandagent \
  --model qwen3:8b
```

If the managed package is elsewhere, set `COMMANDAGENT_PLAYWRIGHT_PATH` to its
Playwright package directory. Use `--trial-timeout-ms` only to raise or lower
the per-run wait bound. A missing browser, package, model, or terminal event is
an honest smoke failure; the runner does not install or substitute one. A
successful run removes its temporary runtime. A failed run preserves the
exact temporary path in `browser-smoke.json` for investigation instead of
interrupting the delegated CLI.
