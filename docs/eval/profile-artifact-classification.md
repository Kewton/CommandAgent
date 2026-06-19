# Profile Artifact Classification

Date: 2026-06-20

## Purpose

This slice addresses a Next.js profile false positive where the generated
framework declaration `next-env.d.ts` was treated as a route-integration
artifact and caused:

```text
profile_verification:nextjs_route_not_integrated
```

The change adds a deterministic classified-artifact boundary. Profile
obligations and verification now consume classified artifacts instead of broad
path-extension checks or `workspace.entries` token scans.

## Design Reading

Responsible layer: Profile Contract.

Not responsible:

- provider transport
- minimal-loop execution
- retry budget
- Recovery Task Contract

The relevant design rule is:

```text
structured facts -> classified artifacts -> obligations/verification
structured facts -> rendered text -> prompts/reports only
```

Workspace observation alone must not create route/source integration
obligations.

## Implementation Summary

- Added `src/agent/step_runner/profile_artifact.rs`.
- Added `ClassifiedArtifact`, `ArtifactProvenance`, `ArtifactKind`, and
  `ArtifactEligibility`.
- Added `classify_profile_artifact(...)` dispatch.
- Added Next.js/Python/Rust/Docs/Data classifiers.
- Replaced Next.js route-integration candidate extraction with classified
  artifact eligibility.
- Stopped using `workspace.entries` and rendered profile obligation text as
  route-integration artifact sources.
- Added regression tests for `next-env.d.ts`, `*.d.ts`, workspace-observed
  artifacts, and the preserved real unintegrated UI-source failure.

## Local Checks

Repository state at evaluation time:

- base commit: `9f52f75`
- dirty flag: `dirty`
- binary: `target/release/commandagent`

Commands:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test profile_artifact
cargo test profiles
cargo test
cargo build --release
```

Result:

```text
all passed
```

## Focused Gemini UAT

Run root:

```text
/private/tmp/commandagent-profile-classification-uat
```

Command:

```text
target/release/commandagent --yes --context-budget 65536 \
  --provider gemini \
  --model gemini-3.1-flash-lite \
  --planner-model gemini-3.5-flash \
  "/ultra-plan-run --profile nextjs Create a minimal Next.js app that can run on port 3011."
```

Result summary:

```text
ultra plan: 4 phases
phase initialize-project: ok
phase configure-port: ok
phase create-app-routes: ok
phase verify-build: ok
```

Generated workspace included:

```text
app/
.next/
node_modules/
next-env.d.ts
next.config.mjs
package-lock.json
package.json
tsconfig.json
```

The run completed successfully even though `next-env.d.ts` existed. No
`profile_verification:nextjs_route_not_integrated` failure was emitted for the
generated declaration.

## Remaining Limits

This slice does not evaluate visual quality, gameplay quality, audio quality,
or richer Next.js app semantics. It only validates that generated declarations
and workspace-only observations do not become route-integration artifacts, and
that real explicit UI/source artifacts remain eligible for route-integration
checks.
