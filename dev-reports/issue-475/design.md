# Issue #475 design

Read the complete GitHub Issue through the public API (gh authentication is
invalid), the worker skill, guardrails, saved R0 store/routes and prior discussion.
Fetched origin/develop: e43b2d86760a77b5250afbb260106ea21abba947. This clean
worker branch already includes verified predecessor #474 at
593649601397034563f6da1d2007d7a30d8dbdf3; inspected its commit and passed report.
Its closed hook recognition remains structural evidence and its independent
repair-diagnostic follow-up is outside #475.

## Investigation before repair

Copy the saved store, route imports/exports/calls and error paths into a new
corpus fixture, with original SHA256 provenance. Run them through the actual
product Source and RecoveryObservationPolicy entry points. Use controlled
variants to distinguish lexical path taint in template diagnostics, the generic
function argument boundary, multiple forwarding hops and rename destinations.
Do not infer that rename or process.pid alone explains this case.

Independently run controlled observers in temporary isolation. Correlate build,
startup, GET and business operation with source_file_sha256 and existing
preflight stage/control audit events. Historical stage is nextjs_capabilities;
historical individual writer/operation remains unknown. External scratch
success is not evidence of the internal historical writer or Recovery success.
Discuss this investigation with commandagent-develop / codex-2 via the read-only
cmate-delegate request already sent; save the reply and adopted decisions here.

## Intended smallest repair

If confirmed, extend the fail-closed leaf recognizer only for provable concrete
module-constant destinations passed through a bounded local collection/atomic
writer chain. Check lexical binding identity, all relevant call sites and complete
sink arguments. Pure diagnostic reads must not silently poison otherwise trusted
builtin bindings; dynamic templates must remain non-authoritative destinations.
Reject unknown/shadowed/reassigned/escaped parameters, ambiguous calls, chdir,
traversal/outside/symlink paths and all existing config/source/private/contract
protections. Do not infer a blanket data/JSON output directory. If the closed
proof cannot be made, retain rejection and document its limitation rather than
loosening the snapshot gate.

Reuse #467 stage/hash/control/restore audit and existing output filtering.
Observer business writes to proven output files are allowed only in the isolated
workspace; unregistered data/source/config changes and observation errors remain
unavailable. Exercise genuine control restore and restore failure separately;
no restore fields may claim execution when validation or unchanged control
prevented it. JSON registration is neither business acceptance nor adoption.

## Verification and ownership

Add focused corpus/recognizer/observer tests first, then existing #429/#467 and
Recovery regression, corpus/guardrails, fmt, clippy all targets, full cargo test,
local release build/version/SHA256 and diff checks. Worker-local tests include a
deterministic observer acceptance check. Parent explicitly owns targeted UAT
orchestration, PR/CI, merge and integrated release identity; report them as
pending parent gates, not completed worker evidence. No model campaign, R0 gate
change, app repair, historical evidence edit, live .anvil change, PR/push/merge or
Issue lifecycle mutation. Commit only the coherent Issue-owned change.

## Adopted decision and controlled evidence

The baseline real Source test confirms two separate barriers: the basename
interpolation taints path, and the rename destination is an unresolved generic
parameter. Changing only the diagnostic or pid does not produce outputs;
changing both the diagnostic and rename destination to a constant produces
projects.json. The implemented leaf supports a maximum of eight local parameter
boundaries; every reference must be an unaliased call and every actual path must
end at an immutable module constant. Foreign/opaque importers disable this new
grant. The basename exception validates the unshadowed builtin receiver and
keeps the resulting template opaque. No generic dynamic path value is granted.

The real product Next.js capability observer has now executed a scripted
build/start/HTTP transport with the byte-identical saved store on Node 24.
Import during scripted build/start and GET / preserve bytes. API GET seeds only
missing JSON, existing [] is retained, and a controlled POST changes projects.
These operations correspond to the existing nextjs_capabilities stage hashes;
original control is unchanged. This justifies output registration for the
concrete closed JSON files in isolation. It does not justify source/config edits,
control edits, business acceptance, or historical writer attribution. The
scripted adapter is not a real Next.js compilation/browser UAT. Parent owns
those integrated acceptance gates. A protected-output variant must reject the
same observed write and still audit control.

Codex2's second read-only review reported no blocking grant bug, retained the
false-negative and observation limits, and requested additional importer/depth
controls. Parent independently requested the escaped named-import counterexample;
record the focused before/after product-policy reproduction in this report set.
