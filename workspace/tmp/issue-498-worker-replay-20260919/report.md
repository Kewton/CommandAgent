# Issue #498 isolated saved L2 replay

Source parent: `981239c91f1ee97cb76c08620202a9f0d03056f8`, with the Issue #498
working-tree implementation. No previous run or saved application was changed.

Command:

```sh
COMMANDAGENT_ISSUE498_REPLAY_ROOT=/Volumes/SSD_NX/tmp/commandagent-issue498-worker-replay-20260919 cargo test --lib issue498_saved_capacity_failure_stops_without_another_request_or_fallback -- --nocapture
```

Result: passed, one test. Fresh isolated directory:
`/Volumes/SSD_NX/tmp/commandagent-issue498-worker-replay-20260919/saved-l2-6P7SoR`.
The directory contains only disposable test setup and replay evidence. There was
no live model request. The Replay client received one reconstruction from the
saved event's parsed-and-later plan. Source acquisition stage:
`profile_augmentation_before_sanitization`.

Observed assertions:

- Model requests: 1. Admission events: 1, at planner attempt 1.
- Status: `stopped`; terminal reason: `bounded_repair_infeasible`;
  retry allowed: false; remaining planner attempts: 2; recovery budget changed: false.
- Model: 529 Unicode scalars, SHA-256
  `e32851275f8e288c11417d41915e6b7e6b4a42902114f136c7663160d73f33f2`.
- Host: 2237 Unicode scalars, SHA-256
  `b2dd0b0c696f9c4b38489821dd2917273a8e139d0b43c8cd6bc7d66e0d0975d2`.
- Overlap: zero in both directions. Minimum: 2766, limit: 2500, excess: 266.
  The original augmented instruction retains all 2786 characters.
- The common normalized scope contains `package.json`, `src/app/global.d.ts`,
  `src/app/globals.css`, and `src/app/layout.tsx`. The package-only registered
  split is inapplicable.
- Application commands/verifiers, setup fallback and last-valid fallback: 0.
  All application paths retain their pre-replay bytes or absence.
- The final error states that bounded repair is not representable under current
  preservation and registered split rules. Its full cause chain contains no
  exhausted-budget, ProposalRepairable or ordering-failure claim.

Evidence files remain in the isolated directory, outside the commit:

- `replay-result.json`: SHA-256
  `3514cbff798066303e589e06d4431ea3b5dde08967be0c5b9c36172e582993a8`.
- `.commandagent/runs/issue466/events.jsonl`: SHA-256
  `ff9a2a2b6919ef1cf762a0afae74c56ca306382ebc45278d92be840509cf673d`.

This establishes capacity stopping and event/control correspondence. It makes
no finding about downstream ordering or application behavior.
