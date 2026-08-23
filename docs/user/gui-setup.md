# GUI setup

[GUI index](gui.md) | [Getting started](getting-started-gui.md) |
[Operations](gui-operations.md)

This page is for operators preparing the static GUI and the optional
`gui_server` binary. The GUI Cargo feature is not a default feature.

## Guided setup and preflight

From a new checkout, the setup script can build the export and server, create
an independent private extension root, write a config example only when none
exists, and create a 0600 Trial token file without printing its value:

```bash
./scripts/setup.sh --gui \
  --base-path /proxy/commandagent/ \
  --extension-root /srv/commandagent/extensions \
  --write-config \
  --gui-token-file "$HOME/.config/commandagent/gui-token"
```

The final summary prints separate preflight and start commands. Run preflight
first. `gui_server --check` does not bind a port; it reports `ok` or `ng` for
the static export/base path, pairwise-disjoint roots, `commandagent --version`,
and token/Origin settings. Green exits 0, `ng` exits 1, malformed arguments
exit 2, and `--check --json` emits the same result as JSON.

An existing `.commandagent/config.toml` or token file is never overwritten.
The setup script displays a proposed config diff and leaves existing files
unchanged.

## The three roots

| Root | Purpose | Write owner |
| --- | --- | --- |
| repository root | Read-only documentation, run reports, contracts, packs, and measurement projections | normal repository workflows, never GUI Trial |
| execution root | Trusted container for isolated `sessions/<session-id>/` CLI workspaces and central `.commandagent/runs/<session-id>/` GUI run records | confirmed GUI Trial delegation only |
| extension root | Private `packs/`, `profiles/`, pins, retirement markers, and `journal.jsonl` | `SupplyRoot` lifecycle API only |

All three must be pairwise disjoint, including canonicalized symlink aliases.
The extension root must not be the Trial execution root.

Each confirmed Trial runs with `<execution-root>/sessions/<session-id>/` as
both its process working directory and CLI `--cwd`. Generated source plus that
workspace's `.commandagent/plans`, `evidence`, and `repairs` stay below the
session directory; they are never shared at the execution-root top level.
Confirmations, `events.jsonl`, `summary.md`, and directive state remain in the
central `<execution-root>/.commandagent/runs/<session-id>/` record used by the
GUI APIs. The legacy `.anvil/runs/<session-id>/` location remains read-only
compatibility input for older records.

## Serve at `/`

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
  --extension-root /path/to/commandagent-extensions \
  --trial-token-auth off \
  --commandagent-bin target/release/commandagent
```

Open `http://127.0.0.1:4173/`. The server binds only to `127.0.0.1`. With
authentication off, the token field is hidden, but every POST still requires a
same-host or allowlisted Origin. Use this only for a trusted loopback session.
If `--execution-root` is omitted, read-only dashboards remain available and
all Trial APIs fail closed.

## Serve below a reverse-proxy path

The build path and server path must name the same prefix. The build accepts a
trailing slash; the server uses the canonical form without it:

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
  --extension-root /path/to/commandagent-extensions \
  --trial-token-auth on \
  --commandagent-bin target/release/commandagent
```

The proxy must preserve the prefix:

```nginx
location /proxy/commandagent/ {
    proxy_pass http://127.0.0.1:4173;
}
```

Do not derive origin or prefix from `X-Forwarded-*`. Set both explicitly.

## CommandMate and Cloudflare route map

CommandMate is external orchestration and is not the GUI server. Keep the
three hops distinct:

| Layer | Example public/path value | Destination |
| --- | --- | --- |
| browser / Cloudflare Access | `https://admin.example.com/proxy/commandagent/` | authenticated tunnel route |
| tunnel or reverse proxy | preserve `/proxy/commandagent/` | `http://127.0.0.1:4173/proxy/commandagent/` |
| `gui_server` | `--base-path /proxy/commandagent` | static export built with `GUI_BASE_PATH=/proxy/commandagent/` |

Require an administrator access policy at Cloudflare or the chosen tunnel. A
valid GUI Trial token does not replace upstream authentication. CommandMate may
start or supervise a local process only through an explicitly authorized
operator workflow; it does not change base-path, Origin, token, or root
isolation contracts.

See [operations](gui-operations.md#token-and-origin-boundaries) for token and
Origin policy and [Two-basePath smoke](gui-operations.md#two-basepath-browser-smoke)
for verification.
