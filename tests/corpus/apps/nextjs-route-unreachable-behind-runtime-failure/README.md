# nextjs-route-unreachable-behind-runtime-failure

Pins the early static repair guidance (Issue #890). When the runtime acceptance
does not pass (and the build did not fail), the unreachable page is still
reported to the repair as one line, taken from `expected-guidance.json`:

`repair guidance: route_unreachable_from_entry:/notes/[id] (src/app/notes/[id]/page.tsx): ...`

The release gate itself is unchanged: its status and reasons are decided as
before. The fixture is synthetic; see `provenance.md`.
