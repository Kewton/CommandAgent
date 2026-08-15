# Read-only GUI

The CommandAgent GUI projects repository evidence into four read-only views:
the score/time dashboard, run acceptance and evidence detail, admitted assets,
and measurement reports. The server binds only to `127.0.0.1`, exposes only
GET routes, and does not use forwarded-host headers.

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
  --repository-root .
```

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
cargo run --features gui --bin gui_server -- \
  --port 4173 \
  --base-path /proxy/commandagent \
  --static-dir gui/out \
  --repository-root .
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

## Read-only API

All API routes are same-origin GET requests below the selected base path:

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
followed during listing, and individual text views are capped at 1 MiB. There
are no POST, PUT, PATCH, or DELETE routes.

## Two-basePath browser smoke

The smoke runner reuses the Playwright package managed by CommandAgent's
existing interaction probe. It neither installs Playwright nor changes the
live `.anvil/` namespace. By default it reads:

`~/.anvil/tools/interaction-probe/node_modules/playwright`

Run both `/` and `/proxy/commandagent/` cases and store screenshots plus the
JSON result in a new evidence directory:

```bash
cd gui
npm run smoke -- --output ../workspace/management/runs/g0-gui-smoke
```

If the managed package is elsewhere, set `COMMANDAGENT_PLAYWRIGHT_PATH` to its
Playwright package directory. A missing browser or package is an honest smoke
failure; the runner does not install or substitute one.
