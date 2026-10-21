# Issue #479 verifier formation

`original-commands.json` copies the exact five commands from the saved R0
Recovery observations. `src/lib/types.ts`, `src/lib/store.ts` and `package.json`
copy the original app bytes; no original files were changed. No dependencies,
runtime state or historical logs are included.

The saved package has no `type` field. On Node v24.1.0 the interface-only types
module has namespace `["default","module.exports"]` through CommonJS interop.
Its 786 bytes have SHA256
`cbc67d6d031f3c029e3379e622471e0b1b1f47ac769a561e3bbc287a31530793`.
The store's six exported functions are listed explicitly in
`formed-commands.json`. These proposed expectations measure module loading and
the runtime export boundary, not interface fields or application behavior.

Focused `cargo test issue479 --lib` tests execute both imports and refinements
against these copied modules, missing modules, evaluation failures and unexpected
runtime exports. An explicit ESM empty-namespace variant is a separate changed
context. Structural route/page predicates run all original failure conditions;
an independent target-function assertion supplies a genuine Test control.

The actual Runner consumes refusal variants and preserves #478's full model and
host duties, exact package checks and nine resulting step commands. The six
registered final commands include the four source checks, build and the smoke
script; existing profile coverage handles the three package checks. A missing
smoke script is contingent on its retained owner and subsequent execution.

Recovery and final-acceptance tests distinguish immutable weak inline evidence,
frozen weak scripts, legitimately creatable missing scripts and input-dependent
Test/structural failures. The tests run in temporary workspaces; no live model,
full app build, browser observation or parent UAT is claimed by this corpus.
