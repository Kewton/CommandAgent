# G-0 GUI smoke evidence

- Date: 2026-08-15 (Asia/Tokyo)
- Target: `develop`
- Scope: static read-only GUI and optional Axum server
- Required base paths: `/`, `/proxy/commandagent/`

## Automated static and HTTP checks

The following checks were green before this record was created:

- Next.js static export at both required base paths
- TypeScript type check
- internal link/fetch basePath source audit
- optional-feature GUI server compile and Clippy
- read-only server source audit
- loopback static HTML, JSON API, report detail, evidence detail, pack pin,
  and SVG responses under `/proxy/commandagent/`

The checked response set was `runs`, `bands`, `maps`, `packs`, `contracts`,
`suites`, `reports`, `maps/score-time.svg`, one run detail, one evidence file,
and one report document. All returned their expected 200 responses after the
server path fixes.

The root-path matrix returned 200 for all of the following:

| Surface | Content type |
| --- | --- |
| `/`, `/assets/`, `/runs/`, `/measurements/` | `text/html; charset=utf-8` |
| `/api/runs`, `/api/bands`, `/api/maps`, `/api/packs` | `application/json` |
| `/api/contracts`, `/api/suites`, `/api/reports` | `application/json` |
| `/api/maps/score-time.svg` | `image/svg+xml; charset=utf-8` |

The proxy-path matrix returned 200 for the dashboard, all seven JSON indexes,
the score/time SVG, the pack-pin inventory (four packs), `p2f-0` run detail,
its `measurement-results.json` evidence, and its `report.md` report view.
Exported HTML referenced `_next` assets with `/proxy/commandagent/` in the
proxy build and `/` in the root build.

## Real-browser probe

The repository-managed Playwright package at version `1.61.1` executed the
prepared two-case probe on 2026-08-15. Both `/` and
`/proxy/commandagent/` passed with:

- dashboard status 200 and the expected heading
- all seven JSON indexes returning 200
- a completely loaded 1400-pixel-wide score/time SVG
- basePath-correct internal links
- assets, measurements, and run-detail pages returning 200 with their expected
  headings
- zero browser console errors

Machine-readable results are in `browser-smoke.json`. The rendered dashboards
are preserved as `root-dashboard.png` and
`proxy-commandagent-dashboard.png`. Both screenshots were inspected after the
run and show the complete dashboard, SVG, formal-band summary, and run ledger
without missing content or layout breakage.

Acceptance status: **green — both required real-browser basePath probes
passed**.
