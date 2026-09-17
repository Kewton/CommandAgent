# Issue #490: reader obligations and output ownership

Portable fixed-response replay based on the frozen 2026-09-17 ownership-protection
campaign at `b13ef093e462084956bb792e10616d6848e00f40`. No network, model service,
local SSD, earlier run directory or application implementation is needed.

`P01-attempt-{1,2}.json` and `P02-D-attempt-{1,2}.json` retain the first two raw
proposals. The third response repeats the second. `variants.json` contains the
frozen N01–N07 proposal steps and the expected targeted refusal. All use P01's
first acquired proposal and goal. P02-D uses the original explicit package paths:
there is no P02-B path deletion, injection or admission bypass.

The normal-flow tests restore only the witnessed completed project-setup
checkpoint, using the frozen scaffold, setup plan and contracts. Empty
`node_modules`/`.next` directories reproduce presence, not working dependencies.
They then use the product phase resolver, parser, host augmentation, sanitizer,
preset conversion, Admission/retries, plan save/reload, before_phase, and normal
register_plan. Test hooks observe returned/persisted/registered plans and stop
immediately after successful registration, before any implementation call.
Assertions distinguish this stop from success of setup registration or test exit.
N01–N05/N07 stop at the intended preservation failure with no core registration
or contract change. N06 recovers through host reaugmentation and is not refusal
evidence. `registration.json` fixes the additions, dedicated producer and
artifact-only README rule; both saved contract and live run scope are checked.

`acquired-before-after.json` derives a second history in which both package
readers are acquired in attempt 1 around the application/package writer. This
is different from P02's newly added later reader. The direct Admission/finish
matrix in `reader-cases.json` covers atomic attributes, instruction/check collage,
distributed commands, ID/path-only matching, both deletions, both cross-update
moves and both one-step merges. Its positive and deletion/movement controls also
run through normal registration. IDs may change without discharging attributes.
Unknown scripts, a side-effect command, additional creation instructions and a
model's read-only claim retain the old output-ownership judgment.

Read-only support is deliberately bounded: the complete existing profile
package-manifest inspection instruction plus nonempty commands recognized by
`nextjs::recovery_authority::is_package_check` (exact generated dev/start port
and build-script expressions). This is not a general shell/JavaScript analyzer
or provenance inferred from a name. Every required input/check remains in the
plan. Other instructions and command forms retain the existing rule.

`runtime-metadata.json` fixes every P02-D step's metadata, synthesized-check flag
and precheck applicability. The runtime test runs the original, retained and
additional readers with passing package inputs, wrong dev port and missing
package.json. It checks real command reports and the actual runner: pass skips
the executor; wrong values fail the declared command; absence records the input
and existing dependency boundary. Both failures reach the ordinary execution/
repair path and propagate its controlled error, never a successful short circuit.
The same test was run with original ownership wiring and with the fix.

`known-build-loss.json` extracts the `verify-build` step at raw, sanitized and
preset-converted stages of frozen full-plan attempt 2. Its diagnostic asserts the still-existing preset conversion
loss of `npm run build` and its original instruction. This is a separate known
issue, not an improvement claimed for #490. The P01/P02 fixtures instead keep
build on the application producer after its work.

The corpus discovery check validates fixture availability. The focused Rust
checks provide behavior evidence; neither corpus `acceptance_passed` metadata
nor fixed planner replies claim live app, GUI, build or #488 projection success.
