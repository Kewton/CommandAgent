# nextjs-route-unreachable-from-entry

Pins the Next.js final route-reachability check (Issue #863). The release gate
must be `failed` with `route_unreachable_from_entry:/notes/[id]` for this tree,
so it can never be `full`. See `expected-gate.json`.
