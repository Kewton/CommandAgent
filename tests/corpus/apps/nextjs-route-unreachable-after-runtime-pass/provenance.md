# Provenance

This is a synthetic fixture derived from the shape of the G1 stage 3a12 and
3a13 CONTENT Q runs (IC-40, Issue #893). It is not a copy of generated output.
The entry page `/` carries the contract hooks and keeps every note action
(new, preview, save, delete) on itself, with no link or router call toward
`/notes/[id]`; that page carries hooks and is only left through a link to `/`.
In the original runs the runtime acceptance (browser readiness and
interaction) passed, and the release gate failed only on this unreachable page.

No secrets or absolute paths are kept.
