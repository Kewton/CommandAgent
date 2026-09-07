# Lazy JSON Recovery fixture

The executable root fixture is synthetic, modeled on the import and local
`filePath` shape of S2's storage writer. Its JavaScript files do not copy
original source or runtime data. The reopened real-source fixtures are described
separately below.
`data/` is intentionally absent. The plain-JavaScript probe invokes a synthetic
GET handler directly, without an HTTP listener, Next.js installation, or a live
service. Its response is a plain status/body object, so it also needs no Fetch
globals. The executable sources use standard Node ESM and filesystem promises;
no TypeScript stripping, transpiler, or Node 24 flag is required. The separate
`fixtures/storage-shape.ts` retains typed writer syntax for static recognition.
The route lazily writes two synthetic JSON files; the failure argument then
reports an independent business failure. Tests copy only these fixture sources
into temporary workspaces and run the existing isolated Recovery preflight.

The unsupported-writers fixture contains opaque writer lookalikes. Regex and
template text cannot supply output authority, and interpolation code cannot
authorize paths. Harmless templates, regex, and ordinary division beside real
writers are now supported. Interpolation references conservatively invalidate
bindings (property names are not lexical references); dynamic evaluation markers
including eval/Function/constructor reject the source. Ambiguous slash contexts,
escaped identifiers, dynamic paths, and unproven helpers still grant nothing.

The reopened fixture under `fixtures/campaign/` contains unchanged complete
S1/S3/E3 source trees, hashes, and explicit provenance/limitations. Recognition
and isolated-effect replay tests cover missing/existing JSON and retain business
failure, source protection, unknown-output rejection, and original-session
immutability. `hidden-evaluation.json` holds six executable-mutation negative
sources, including the review's eval example and Function/constructor variants.
The event fixtures document existing schema projections, not live recordings.
