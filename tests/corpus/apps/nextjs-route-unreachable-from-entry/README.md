# nextjs-route-unreachable-from-entry

Pins the Next.js final route-reachability check (Issue #863). The release gate
must be `failed` with `route_unreachable_from_entry:/notes/[id]` for this tree,
so it can never be `full`. See `expected-gate.json`.

The entry page also builds an external anchor in a JS string
(`'<a href="$2" ...>'`, Issue #868). A string `href` that is not route-shaped
counts as neither a navigation nor an unresolved one, so the result stays
`failed` rather than `partial`.
