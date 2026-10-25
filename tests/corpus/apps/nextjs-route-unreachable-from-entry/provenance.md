# Provenance

Minimal shape of the G1 stage 3a CONTENT Q run (L-113, IC-30): the entry page `/`
carries the contract hooks, `src/app/notes/[id]/page.tsx` carries them as well,
and no link, router call, redirect, location assignment, or form action under
`src/` leads from `/` to `/notes/...`. The only navigation (`router.push("/")`)
sits in the edit page and goes the other way.

The sources are hand-reduced from that run; no generated output, secrets, or
absolute paths are kept.
