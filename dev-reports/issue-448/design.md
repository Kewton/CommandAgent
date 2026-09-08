# Issue #448 design

The approved scope is a fixed R0 source/diagnostic corpus and compile/Recovery
promotion regression tests. No product repair, model call, browser campaign,
integration-worktree edit, or live runtime change is included.

## Evidence and existing behavior

- Base HEAD is `5572467ee5028ae43f8497f875e1ef16030cf470`; the committed #442
  preset-routing change is already present and was inspected. No predecessor
  is listed for this task.
- Read the integration worktree's AGENTS.md (including Browser reliability),
  R0 stop report, review resolution, failure evidence and independent build.
  Campaign `20260908-2310-standard10`, session
  `01a08187-2bf0-7853-b0c9-814043569f26` retained the control after a UI-only
  treatment. Its Next.js 14.2.35 build showed a missing `isValidTaskStatus`
  export and a projects `Promise<void>` / release-function mismatch.
- `RunnerRecoveryDriver::finish` already rejects failed execution, preserves
  observer authority, re-observes registered verification and completion
  acceptance, and only then promotes. Use that implementation unchanged.

## Smallest coherent change

1. Freeze allowlisted original source/config/lock files, the original completion
   contract, selected diagnostics and the UI treatment, with source SHA-256
   provenance. Never copy runtime state, business data, secrets or raw logs.
2. Add explicit fixture-only overlays for the missing export and Promise typing.
   Keep all routes, UI, requirements and strict checking. If fixing the first
   error exposes another compile error, record it and repair only the necessary
   fixture source; do not hide it with `any` or compiler exemptions.
3. Exercise original, UI-only, export-only, Promise-only and repaired variants
   with the same locked independent `npm run build`. Keep this dependency-heavy
   check explicitly invoked, separate from the ordinary offline Rust suite.
4. Add an issue-specific Rust test module using the existing import scanner,
   compile diagnostic parser and actual Recovery transaction/promotion functions.
   Test control hashes, failed execution, post-repair revalidation, remaining
   required checks, missing acceptance evidence and unchanged observer authority.
   A deterministic positive boundary control demonstrates the normal promotion
   path; it must not be represented as full R0 business/browser acceptance.
5. Preserve reproducible commands and compact measured results. Distinguish
   actual compiler runs from scripted boundary observations/model-free replay.

## Verification

Run focused fixture/Recovery tests, the locked Next.js build matrix, corpus
validation, formatting, Clippy and the full Rust suite. Record exact commands and
outcomes in verification.md; use `passed` only if all required checks pass.

Runtime lock deadlock and data loss were not observed in R0. Compile success
and fixture-only type repair do not prove mutual exclusion, persistence,
business correctness or live-model repair effectiveness.
