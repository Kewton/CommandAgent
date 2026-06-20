# Next.js Profile Contract Focused E2E - 2026-06-20

## Context

This run validates the Next.js profile-contract split from generic plan lint:

- profile-owned planning guidance
- profile-specific plan lint through the shared profile boundary
- manifest facts used to avoid repeating already-satisfied package obligations
- stable TypeScript 5.x planning and verification guidance
- `@/*` import guidance tied to `tsconfig.json` path aliases

## Command

```bash
set -a
source .env
set +a
scripts/eval_agent_slice.sh \
  --cases-dir /private/tmp/commandagent-nextjs-focused-cases \
  --out /private/tmp/commandagent-nextjs-focused-e2e-gemini-env4 \
  --runs 1 \
  --binary target/release/commandagent \
  --provider gemini \
  --model gemini-3.1-flash-lite \
  --timeout-secs 900
```

## Result

- commit: `b218a4a271391911fffd18ada53bdbea6372cba8`
- dirty: `true`
- run root: `/private/tmp/commandagent-nextjs-focused-e2e-gemini-env4/20260620T212852`
- case: `large-nextjs-app-new`
- result: `success=true`
- reason: `ok`
- elapsed: `118807 ms`

## Notes

Two earlier focused runs exposed profile-contract issues during this change:

- an already-compliant `package.json` caused later manifest edit steps to
  restate satisfied obligations; profile lint now consults current manifest
  facts before requiring missing package literals.
- a TypeScript plan correction produced `typescript ^5.4.0`, but the lint rule
  accepted only the textual form `5.x`; profile lint now treats `5.x`, `^5.*`,
  and `~5.*` as the same stable TypeScript 5.x contract while still rejecting
  ambiguous `typescript, 5` and profile-verifying `typescript@5.0.0` drift.

The final focused run succeeded without weakening Next.js profile verification,
adding hidden retries, or adding provider-specific behavior.
