# Lazy JSON Recovery fixture

Synthetic source-only fixture modeled on the import and local `filePath` shape
of S2's storage writer. No original source file or runtime data was copied.
`data/` is intentionally absent. The plain-JavaScript probe invokes a synthetic
GET handler directly, without an HTTP listener, Next.js installation, or a live
service. Its response is a plain status/body object, so it also needs no Fetch
globals. The executable sources use standard Node ESM and filesystem promises;
no TypeScript stripping, transpiler, or Node 24 flag is required. The separate
`fixtures/storage-shape.ts` retains typed writer syntax for static recognition.
The route lazily writes two synthetic JSON files; the failure argument then
reports an independent business failure. Tests copy only these fixture sources
into temporary workspaces and run the existing isolated Recovery preflight.

The unsupported-writers fixture is a negative example. Regex, division, and
template syntax cause this conservative recognizer to grant no outputs for the
file. Escaped strings, dynamic arguments, ambiguous bindings, and unsupported
constant/import forms also grant no output. See `dev-reports/issue-429/design.md`.
The event fixture documents the existing schema, not a recording from S2.
