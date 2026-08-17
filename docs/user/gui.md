# Management GUI

The CommandAgent GUI projects repository evidence into four read-only views
and adds one confirmed trial-run view. The dashboard, verification/operations
reports, admitted
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
cargo run --features gui --bin gui_server -- \
  --port 4173 \
  --base-path / \
  --static-dir gui/out \
  --repository-root . \
  --execution-root /path/to/trial-workspace \
  --trial-token-auth off \
  --commandagent-bin target/release/commandagent
```

`--trial-token-auth` accepts `on` or `off` and defaults to `off`. In the default
mode the page hides the **Trial access token** field and Trial APIs do not
require a bearer token. POST requests still require a same-host Origin (or an
origin admitted by `GUI_TRIAL_ALLOWED_ORIGINS`). Use this mode only for a
trusted local loopback session.

To require the runtime-only token, set `--trial-token-auth on` and export a
32–4096 character non-whitespace `GUI_TRIAL_TOKEN` before startup. The page
then keeps the entered value in tab-scoped `sessionStorage` and sends it in
`X-CommandAgent-Trial-Authorization`. The server also accepts the legacy
direct-client `Authorization: Bearer` form. When token authentication is on,
startup fails if `GUI_TRIAL_TOKEN` is missing or invalid.

If `--execution-root` is omitted, the dashboard remains available but all
Trial APIs fail closed with HTTP 503 regardless of the authentication mode.

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
  --trial-token-auth on \
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

The page shows a compact **依頼 → 確認 → 実行 → 結果** step indicator and only
the current workflow state. Completed forms and progress cards do not remain
stacked above the next action. At 390 px, the request and confirmation action
bars remain visible above the bottom navigation without scrolling; at Terminal,
the D-3d controls precede the longer result evidence.

The **LM Studio** provider selection maps to the existing CLI spelling
`--provider lm-studio`; it is not an alias or a GUI-side provider
implementation. Enter the exact model identifier exposed by LM Studio. The
delegated CLI uses its default `--lm-studio-host http://localhost:1234` and
inherits `LM_STUDIO_API_TOKEN` from the GUI server environment when LM Studio
authentication is enabled.

1. Enter the runtime Trial token, then select **契約と見積りを確認**.
   Gate 1 renders the server-provided `card_markdown`. It explains each
   required check, including Python CLI C1-C4, and states the comparable runs
   that passed every check instead of exposing only the internal rate/window
   labels. Recorded mean duration/cost and the canonical filesystem write
   boundary remain visible beside the card.
2. Select the confirmation checkbox. The launch button stays disabled until
   this explicit confirmation, and the API independently requires the exact
   card hash.
3. Select **確認して CLI を実行**. The server starts the existing
   non-interactive boundary command. Progress is reconstructed by reading that
   session's JSONL events; there is no GUI state database. The launch identity
   fields remain read-only throughout Gate 2 and the terminal result so the
   confirmed contract cannot be invalidated by an in-flight edit. Gate 2
   reports the execution state separately from monitoring health (`connected`,
   `degraded`, or `lost`) and shows the last successful update time. A transient
   monitoring failure retries with capped exponential backoff while the
   delegated CLI keeps running. Independently, Gate 2 advances a
   browser-observed elapsed clock once per second from receipt of the accepted
   session, keeps the measured mean beside it as a comparison rather than an
   ETA guarantee, and shows `Phase x / N` only when the file-backed phase
   projection reports a nonzero total.
4. During Gate 2, use **Recent events** or **Browse artifacts** to inspect
   bounded, read-only session evidence without leaving the GUI. At Gate 3 or
   Gate 4 the inventory opens automatically. The result, assurance level, and
   execution status are shown under separate labels; if no final verdict was
   recorded, the result says so instead of presenting an assurance identifier
   such as `static` as the verdict. Select `summary.md`, recent
   `events.jsonl`, or an acceptance-related text file to investigate a
   failure. You may then end without another run, or persist an additional
   instruction with **追加の依頼を確認用に準備**. The instruction is
   credential-scrubbed, exact-byte hashed, displayed, and must be confirmed;
   it cannot lower the fixed contract checks. Reaching Gate 3/4 also changes
   the browser tab title to the plain-language result so completion is visible
   while the Trial tab is in the background.
5. After **End without another run**, select **Start a new run** to return to
   an editable draft. The tab-scoped Trial token and launch fields are retained,
   while the previous proposal, session progress, and directive are cleared.

The navigation item **検証・運用レポート** reads the repository-side
`workspace/management/runs`. It is for persisted verification and operations
evidence; it does not list GUI Trial sessions. The **GUI Trial 実行履歴** panel
instead reads `.anvil/runs` below the configured execution root. Both pages
show their source path explicitly.

Entering or restoring a complete runtime token loads up to 100 confirmed Trial
run directories, with the active lease session first and the remaining rows
ordered by latest update. The list is revalidated after a launch is accepted,
on a Gate 3/4 transition, after reconnect succeeds, when the shared runtime
lease leaves `running`, on window focus or tab visibility, and by **セッションを
更新**. It does not run an independent short-interval list poll. A launch row
appears immediately with the returned ID and `starting` state while the file
projection catches up. Terminal results link directly to their matching
history row.

The panel shows the last successful refresh separately from a current refresh
error. A failed refresh therefore leaves the last successful rows visible. A
missing or incomplete token is shown as authentication pending, never as an
authenticated empty session list. Each row shows its UUID-v7-derived start time
(or file-creation fallback), latest update, file-backed gate/status, and a link
to the existing
`?session=<id>` reconnect flow. Following that link does not issue a POST or
delegate another process; the runtime token is restored from the same tab's
session storage before the session-status GET runs.

The existing workspace lease card displays `idle`, `running(<session-id>)`, or
`recovery_required(<session-id>)`, and is refreshed from the same index
response. A non-idle snapshot disables confirmed launch and explains which
session owns or blocks the workspace. This snapshot is advisory and read-only:
the server independently enforces the lease during POST, and the GUI provides
no clear, reset, cancel, or force-idle action.

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

The page places the launched session ID, but never a token, in
`?session=<id>`. With token authentication on, a same-tab reload or navigation
restores the runtime Trial token for **Reconnect monitoring**. Reconnect calls
only `GET api/sessions/{id}` and cannot delegate another CLI process. A 409
response that identifies an already running or recovery-required session fills
this same reconnect path.

Monitoring guidance distinguishes authentication and browser boundaries:

- With token authentication on, HTTP 401 asks you to re-enter the runtime token.
- HTTP 403 asks you to verify the allowed origin in either mode.
- An upstream manual redirect asks you to reload and re-authenticate with the
  access proxy.
- A thrown browser fetch asks you to check the proxy/network connection and
  reload or re-authenticate if required.

HTTP 413 and invalid event JSONL are retried only to the terminal-error limit,
then monitoring stops with an artifact inspection reason. Other failures keep
retrying at the capped interval. A response with the definitive
`trial_token_invalid` code removes the rejected value from the field and this
tab's storage; a generic proxy 401/403 does not. The token is never stored in
`localStorage`, included in URLs, or compiled into the static export.

### Trial token lifetime and rotation when authentication is on

The token survives reloads and navigation only through `sessionStorage` for the
current tab. An independently opened tab is not synchronized by CommandAgent
and requires manual entry. Clearing or editing the password field immediately
updates that tab's stored value. Root-path and `/proxy/commandagent/`
deployments use different storage keys, so they do not reuse one another's
token on the same origin.

Closing the tab is the ordinary lifetime boundary, but browsers may clone
session storage when a tab is duplicated and may restore it after a crash or
session restore. Do not treat browser exit as guaranteed deletion. A
same-origin XSS can read the stored value or issue authenticated Trial requests,
and an unlocked lost device with the tab available can exercise the same
authority.

After device loss or suspected disclosure, revoke the upstream Access session,
stop `gui_server`, generate a fresh token containing 32–4096 non-whitespace
characters (for example, 32 random bytes encoded as hex), set the new
`GUI_TRIAL_TOKEN`, and restart the server. Close affected tabs or clear their
Trial token fields; the old stored value will also be removed when the restarted
server definitively rejects it. Redistribute the replacement token only through
the operator's approved secret-transfer channel.

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
| `api/runtime-status` | Trial availability, token-authentication mode, and the current workspace lease state |

Paths are canonicalized below their allowed inventory root, symlinks are not
followed during listing, and individual text views are capped at 1 MiB.

Trial run adds these bounded routes:

`GET api/trial-options` is an unauthenticated, read-only projection of compiled
profile/provider metadata so the form can be populated before a Trial token is
entered. It neither inspects the execution workspace nor contacts a provider.

When `--trial-token-auth on` is selected, every other route in this table
requires `X-CommandAgent-Trial-Authorization: Bearer <GUI_TRIAL_TOKEN>` (or the
legacy direct-client `Authorization` form). With the default
`--trial-token-auth off`, those routes accept requests without a token. POST
requests require a same-host Origin or an origin admitted by
`GUI_TRIAL_ALLOWED_ORIGINS` in both modes.

| Route | Operation |
| --- | --- |
| `GET api/trial-options` | Return admitted profiles, providers, and model-ID guidance without executing anything |
| `POST api/session-proposals` | Render a deterministic Gate 1 identity and measured price tag |
| `GET api/sessions` | List up to 100 execution-root Trial sessions and the current read-only lease snapshot |
| `GET api/trial-workspace` | Read the current workspace lease and active/recovery session ID |
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

### Error responses and recovery

API failures use JSON with an additive stable code while retaining the existing
HTTP status and `error` text:

```json
{
  "code": "trial_token_invalid",
  "error": "a valid GUI trial bearer token is required"
}
```

The GUI translates the code into a next action and keeps the server detail
visible for diagnosis:

| Status / code | Recovery |
| --- | --- |
| `401 trial_token_invalid` | Reload the page, re-authenticate at the upstream access layer, and enter the runtime Trial token again. |
| `403 trial_origin_not_allowed` | Add the exact browser Origin to `GUI_TRIAL_ALLOWED_ORIGINS`, then restart the GUI server. |
| `409 trial_workspace_running` | Use the displayed session ID and reconnect link to resume GET-only monitoring. |
| `409 trial_workspace_recovery_required` | Inspect that session's events and complete the existing CLI/runtime recovery procedure before reconnecting. Do not delete `.anvil/` state to bypass the lease. |
| `409 trial_workspace_conflict` | Verify that the execution root is still available at its startup path and remains disjoint from the repository. |
| `412 trial_confirmation_stale` | Request the current Gate 1 card and confirm it again. |
| `428 trial_confirmation_required` | Check the contract and price, then explicitly confirm the displayed Gate 1 card. |
| `503 trial_execution_disabled` | Restart the GUI server with a valid `--execution-root`; also set `GUI_TRIAL_TOKEN` when `--trial-token-auth on` is selected. |
| `500 trial_internal_error` | Verify that `--commandagent-bin` points to an existing executable, inspect the server log, and reconnect to an already-created session instead of dispatching another process. |

Read-only pages use the same descriptor for missing or unreadable repository
records. Reload the inventory first; if the error remains, verify
`--repository-root`, the selected path, and file permissions. A proxy or
network rejection is reported as a connection/reload action instead of the
browser's implementation-specific exception text.

To verify these recovery paths without a provider call, run the focused smoke:

```bash
cd gui
npm run smoke:errors
```

It builds the static GUI, starts a loopback server with an isolated temporary
workspace and bounded fake CLI, then uses Playwright to check a wrong token, a
foreign Origin header, and a live workspace 409 with its reconnect link. It
removes the temporary workspace after the probe.

The event-tail reader scans backward, so it remains useful after the complete
stream exceeds the 4 MiB status-polling limit. Artifact listing uses the same
text-extension allowlist, depth-four walk, skipped directories, ordering, and
entry cap as the repository run viewer. All Trial file routes require a
canonical session UUID and the runtime token. The bounded index exposes only
session identity and lifecycle projection; artifact content remains behind the
per-session routes.

Delegated stdout and stderr are intentionally not saved by this GUI path. The
CLI-owned `events.jsonl` and `summary.md` are the structured diagnostic
records, and the GUI server only reads them. If an unstructured log is later
required, it must be introduced as a CLI-owned output contract rather than a
GUI-server write.

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
run, a mocked elapsed/phase/title feedback probe, the first session event
stream and its SHA-256, in-page recent-events and summary viewing, and an API
log. The browser script explicitly fills Goal plus executor and planner model
fields for each new run, so it does not depend on Trial form defaults:

```bash
cd gui
npm run smoke -- \
  --output ../workspace/management/runs/g1-gui-smoke \
  --commandagent-bin ../target/release/commandagent \
  --model qwen3:8b
```

Use `--feedback-only` to run only the deterministic browser feedback probe
for both base paths. It uses mocked Trial responses and does not dispatch a CLI
process.

If the managed package is elsewhere, set `COMMANDAGENT_PLAYWRIGHT_PATH` to its
Playwright package directory. Use `--trial-timeout-ms` only to raise or lower
the per-run wait bound. A missing browser, package, model, or terminal event is
an honest smoke failure; the runner does not install or substitute one. A
successful run removes its temporary runtime. A failed run preserves the
exact temporary path in `browser-smoke.json` for investigation instead of
interrupting the delegated CLI.
