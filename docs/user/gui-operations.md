# GUI operations

[GUI index](gui.md) | [Setup](gui-setup.md) | [Trial](gui-trial.md) |
[Extensions](gui-extensions.md)

This page owns runtime access, API, recovery, backup, and smoke guidance for an
installed GUI. Run [GUI preflight](gui-setup.md#guided-setup-and-preflight)
before binding a listener.

The Shell owns one `runtime-status` poller for every page. It pauses while the
document is hidden and refreshes immediately when the tab becomes visible.
Repository records, measurements, and extension lists also revalidate on a
visible-tab transition or window focus. A failed refresh reports the new error
without removing the last successful list.

## Token and Origin boundaries

`--trial-token-auth` accepts `on` or `off` and defaults to `off`. Off hides the
token field and removes bearer authentication, but every POST still requires a
same-host Origin or one listed by `GUI_TRIAL_ALLOWED_ORIGINS`. Use off only on a
trusted local loopback session.

On requires a 32–4096 character non-whitespace `GUI_TRIAL_TOKEN` in the server
process environment. The browser sends it as
`X-CommandAgent-Trial-Authorization: Bearer`; direct legacy clients may use
`Authorization: Bearer`. Startup fails closed if the token is missing/invalid.

`GUI_TRIAL_ALLOWED_ORIGINS` is a comma-separated exact allowlist for proxy
origins. A token never substitutes for upstream Cloudflare/tunnel access
policy. The listener remains loopback-only and does not trust forwarded host
headers.

## Trial token lifetime and rotation when authentication is on

The token survives reload/navigation only in `sessionStorage` for the current
tab. Independent tabs require entry. Root and `/proxy/commandagent/` use
different keys. Clearing or editing the field updates that tab immediately.

Browsers may clone a duplicated tab or restore session state after a crash; do
not treat browser exit as guaranteed deletion. Same-origin script compromise
or an unlocked device can exercise the stored authority.

After suspected disclosure, revoke upstream access, stop the server, generate
a fresh token, restart with the replacement, and close/clear affected tabs.
Distribute the replacement only through an approved secret channel. A
definitive rejection removes the old value from that tab.

## Extension-root operations

Keep the entire extension root owner-private (directory mode 0700; secret-like
files 0600 where applicable) and disjoint from repository/execution roots.
Back up packs, profiles, exact-byte pins, `RETIRED` markers, and append-only
`journal.jsonl` together. A backup lacking retirement or journal state is not a
complete supply record.

Retirement is non-destructive and terminal for that version. Do not delete
bytes/history or edit a pinned version; stage a new version. Review the journal
for `stage|verify|pin|retire|profile_register`, actor, outcome, exact identity,
and scrubbed detail. A profile record includes only its normalized relative
path and exact hash, never the submitted TOML. See
[GUI extensions](gui-extensions.md#lifecycle-workflow).

## API

Evidence routes are same-origin GET requests below the configured base path:

| Route | Projection |
| --- | --- |
| `api/runs`, `api/runs/{id}` | repository run index and acceptance/evidence inventory |
| `api/runs/{id}/evidence?path=…` | one bounded text evidence member |
| `api/bands`, `api/maps`, `api/maps/score-time.svg` | measured-band and score/time projections |
| `api/packs`, `api/contracts`, `api/suites` | resolved pack catalog and reviewed documents |
| `api/reports`, `api/reports/view?path=…` | measurement report archive |
| `api/runtime-status` | Trial readiness, extension-root state, authentication mode, and lease state |

Paths are canonicalized below inventory roots, listing does not follow
symlinks, and individual text views are capped at 1 MiB.

`GET api/trial-options` and `GET api/pack-options` are unauthenticated read-only
metadata; they do not inspect the execution workspace or contact providers.
Other Trial routes require authentication when enabled. All POSTs require an
allowed Origin.

| Route | Operation |
| --- | --- |
| `POST api/session-proposals` | render deterministic Gate 1 identity and measured price tag |
| `GET api/sessions` | list up to 100 Trial sessions and the lease snapshot |
| `GET api/trial-workspace` | inspect the read-only workspace lease |
| `POST api/sessions` | require exact Gate 1 hash, then delegate the CLI |
| `GET api/sessions/{id}` | project events/artifacts to phase, gate, and terminal result |
| `GET api/sessions/{id}/artifacts` | list/read bounded non-symlink text artifacts |
| `GET api/sessions/{id}/events?tail=N` | read `1..=2000` tail lines within 1 MiB |
| `POST api/sessions/{id}/directives` | scrub and persist a hashed D-3d proposal |
| `POST api/sessions/{id}/directives/{hash}` | require the exact proposal, then continue |

The process boundary executes `commandagent` directly without a shell. It
clears the child environment and restores basic process/locale variables plus
documented provider credentials. GUI access secrets and ambient
`COMMANDAGENT_PACK_*` selectors are not inherited. Extension lifecycle routes
are listed separately in [GUI extensions](gui-extensions.md#extension-supply-api).

## Error responses and recovery

Failures use JSON with a stable additive code plus diagnostic `error` text.
The GUI translates the code into a next action without hiding the server detail.

| Status / code | Recovery |
| --- | --- |
| `401 trial_token_invalid` | Re-authenticate upstream and enter the runtime token again. |
| `403 trial_origin_not_allowed` | Add the exact browser Origin, then restart. |
| `409 trial_workspace_running` | Use the displayed ID and GET-only reconnect link. |
| `409 trial_workspace_recovery_required` | Inspect events and use the conservative Trial recovery; never delete `.anvil/` to bypass it. |
| `409 trial_workspace_conflict` | Restore the startup path and root separation. |
| `412 trial_confirmation_stale` | Request and confirm the current Gate 1 card. |
| `428 trial_confirmation_required` | Check contract/price and explicitly confirm. |
| `503 trial_execution_disabled` | Restart with a valid execution root and required token. |
| `500 trial_internal_error` | Check the CLI path/server log; reconnect rather than double-dispatch. |
| `401 profile_auth_failed` | Enter the current Trial token, then preview again. |
| `403 profile_origin_not_allowed` | Use the configured GUI Origin or add its exact value and restart. |
| `400 profile_invalid_request` | Rebuild the JSON request with only the documented fields. |
| `413 profile_body_too_large` | Reduce the manifest or overlay to 256 KiB or less. |
| `422 profile_validation_failed` | Correct the relative path, closed schema/capability, ID, or additive overlay. |
| `409 profile_confirmation_stale` | Preview the current bytes again and reconfirm the returned exact hash. |
| `409 profile_conflict` | Choose a new ID/path or preserve the existing file; it will not be overwritten. |
| `500 profile_io_failed` | Check extension-root ownership, free space, managed paths, and journal; retrying identical bytes is safe. |

For unreadable repository records, reload inventory, then verify repository
root, canonical path, and permissions. For proxy/network rejection, assume an
existing child may still run and restore observation before taking action.

Run the provider-free focused recovery smoke with:

```bash
cd gui
npm run smoke:errors
```

## Troubleshooting checklist

- `static.base_path` is `ng`: rebuild with `GUI_BASE_PATH` matching
  `--base-path` exactly.
- Trial is disabled: configure an existing, disjoint execution root and the
  correct product binary.
- Token rejected: verify authentication mode and rotate only after checking
  upstream/proxy responses.
- Origin rejected: use the exact browser scheme/host/port in the allowlist.
- Catalog mutation fails: verify extension ownership, 0700 permissions, space,
  and journal state; do not bypass `SupplyRoot` or `ProfileSupplyRoot`.
- A profile was saved but is absent from Trial: honor `restart_required`, restart
  with the same extension root, then match its exact hash in Layer 2 and Gate 1.
- Lease is non-idle: use the exact session and the
  [read-only recovery guide](gui-trial.md#workspace-lease-inspection-and-recovery).
- Monitoring is lost: inspect network/proxy and reconnect; do not infer child
  termination from the browser alone.

## Two-basePath browser smoke

The smoke runner reuses the repository-managed Playwright installation and
does not alter the live `.anvil/` namespace. Build the delegate first:

```bash
cargo build --release --bin commandagent
```

Then run both `/` and `/proxy/commandagent/` against isolated workspaces:

```bash
cd gui
npm run smoke -- \
  --output ../dev-reports/gui-smoke \
  --commandagent-bin ../target/release/commandagent
```

Use `--overview-only` for the provider-free landing/first-use check and
`--read-only` for all read-only projections. The Overview smoke verifies
base-safe Trial/status/history/detail CTAs, direct reload, absence of duplicated
map/band/run dashboards and their API fetches, real and unavailable runtime
states, heading structure, focus visibility, WCAG axe rules, reduced motion,
help copy, and desktop/mobile layout on both base paths. The broader smoke also
verifies runtime concurrency, list revalidation, extension labels/handoff,
Trial workflow, and reconnect. It records
screenshots and `browser-smoke.json`; it never makes a successful result by
weakening Gate 1 or terminal acceptance.
