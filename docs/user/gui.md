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
planner/executor model pins. Goal and both model fields start empty so a demo
request or model cannot be delegated accidentally. **Check contract and
price** validates those empty fields in the browser before making a proposal
request.

The browser obtains profile and provider choices from `GET api/trial-options`.
Profiles are the server's current `admitted_profiles()` set and include a short
scope description. Provider choices include model-ID guidance. Changing the
provider does not rewrite either model pin, so the form shows a warning to
review the executor model before Gate 1.

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
   session's JSONL events; there is no GUI state database. The launch identity
   fields remain read-only throughout Gate 2 and the terminal result so the
   confirmed contract cannot be invalidated by an in-flight edit. Gate 2
   reports the execution state separately from monitoring health (`connected`,
   `degraded`, or `lost`) and shows the last successful update time. A transient
   monitoring failure retries with capped exponential backoff while the
   delegated CLI keeps running.
4. At Gate 3 or Gate 4, inspect the generated acceptance sheet. You may end
   without another run, or persist an additional D-3d instruction. A D-3d
   instruction is credential-scrubbed, exact-byte hashed, displayed, and must
   be confirmed before the existing continuation path is delegated.
5. After **End without another run**, select **Start a new run** to return to
   an editable draft. The in-memory Trial token and launch fields are retained,
   while the previous proposal, session progress, and directive are cleared.

There is no cancel, interrupt, phase-edit, or gate-override control while a
session is running. Use the existing CLI/runtime operating procedures for
external process management. GUI confirmation cannot lower, replace, or
satisfy the contract checks.

## Workspace lease inspection and recovery

The Trial page's **Inspect workspace lease** action performs only authenticated
`GET api/trial-workspace`. It reports `Idle`, `Running`, or
`Recovery required`; the latter two states include the exact session ID. The
card cannot clear the lease, dispatch a process, or modify session artifacts.
Checking the Gate 1 contract also refreshes this read-only projection, and a
launch conflict refreshes it before displaying the HTTP 409 error.

If the configured CLI binary cannot be spawned, HTTP 500 names the configured
binary path and the operating-system cause. Because no child exists, the server
rolls back that new session and releases the lease. Correct
`--commandagent-bin` (or install/fix the binary at that path) and retry; the
same execution root does not require manual recovery.

`Recovery required` means a confirmed child may have existed but the event
stream has no current `tui_command_stop` or `run_stop`. Treat that state as a
possible live process. Recover it conservatively:

1. Record the session ID shown by the lease card, then stop `gui_server` so no
   new Trial can be admitted during recovery.
2. Use the operating system's process inspection to verify that no delegated
   `commandagent` remains for the execution root and the session's
   `.anvil/runs/<session-id>/state` directory. If one is still running, do not
   clear or archive the lease; follow the existing CLI/runtime procedure to let
   it finish or stop it, then inspect its events again.
3. When no child remains, preserve the whole
   `.anvil/runs/<session-id>` directory in an operator-chosen archive outside
   the execution root. Move the directory as one unit; do not delete individual
   confirmation files and do not append a synthetic terminal event.
4. Restart `gui_server` with the same execution root, re-enter the runtime
   token, and select **Inspect workspace lease**. It must report `Idle` before
   another Trial is launched. If another unfinished run is reported, repeat
   the same process check for that exact session instead of bypassing the
   single-process lease.

The archive remains the evidence for the incomplete session. Restoring it
under `.anvil/runs` will intentionally make startup require recovery again.

The page places the launched session ID, but never the token, in
`?session=<id>`. After a reload or navigation, re-enter the runtime Trial token
and select **Reconnect monitoring**. Reconnect calls only
`GET api/sessions/{id}` and cannot delegate another CLI process. A 409 response
that identifies an already running or recovery-required session fills this same
reconnect path.

Monitoring guidance distinguishes authentication and browser boundaries:

- HTTP 401/403 asks you to re-enter the runtime token and verify the allowed
  origin.
- An upstream manual redirect asks you to reload and re-authenticate with the
  access proxy.
- A thrown browser fetch asks you to check the proxy/network connection and
  reload or re-authenticate if required.

HTTP 413 and invalid event JSONL are retried only to the terminal-error limit,
then monitoring stops with an artifact inspection reason. Other failures keep
retrying at the capped interval. The token remains only in page memory: it is
not stored in `localStorage`, included in URLs, or compiled into the static
export.

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
| `api/runtime-status` | Trial availability and the current workspace lease state |

Paths are canonicalized below their allowed inventory root, symlinks are not
followed during listing, and individual text views are capped at 1 MiB.

Trial run adds these bounded routes:

`GET api/trial-options` is an unauthenticated, read-only projection of compiled
profile/provider metadata so the form can be populated before a Trial token is
entered. It neither inspects the execution workspace nor contacts a provider.

Every other route in this table requires
`X-CommandAgent-Trial-Authorization: Bearer <GUI_TRIAL_TOKEN>` (or the legacy
direct-client `Authorization` form). POST requests also require a same-host Origin or an origin admitted by
`GUI_TRIAL_ALLOWED_ORIGINS`.

| Route | Operation |
| --- | --- |
| `GET api/trial-options` | Return admitted profiles, providers, and model-ID guidance without executing anything |
| `POST api/session-proposals` | Render a deterministic Gate 1 identity and measured price tag |
| `GET api/trial-workspace` | Read the current workspace lease and active/recovery session ID |
| `POST api/sessions` | Require the exact Gate 1 hash, then delegate to the configured CLI binary |
| `GET api/sessions/{id}` | Read events and artifacts to project phase, gate, and terminal verdict |
| `POST api/sessions/{id}/directives` | Apply the existing credential scrub and persist a hashed D-3d proposal |
| `POST api/sessions/{id}/directives/{hash}` | Require that exact proposal, then delegate the existing continuation plan |

The two POST dispatch routes cannot accept an unconfirmed identity. The sole
process surface executes `commandagent` directly without a shell; provider and
runner calls are forbidden in the GUI server by the protection audit.

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
dashboard/API/SVG probes, desktop and mobile layout probes, Gate 1 before and
after confirmation, a rejected first poll followed by Gate 3/4 recovery,
proxy-access re-authentication guidance, token re-entry and GET-only reconnect,
the read-only launch identity, CLOSED-to-compose recovery and a second terminal
run, the first session event stream and its SHA-256, and an API log. The browser
script explicitly fills Goal plus executor and planner model fields for each
new run, so it does not depend on Trial form defaults:

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
