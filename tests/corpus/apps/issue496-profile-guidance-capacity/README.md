# Issue #496 profile guidance capacity fixture

`saved-plan.json` is a reconstructed parsed-stage fixture extracted from the first
`recovery_verifier_plan_admission` event in the valid Issue #494 owner-scope C0 replay.
It is not a saved raw model response and does not support attribution before parsing.

The target step is `implement-layout-and-config`:

- model instruction: 529 Unicode scalar values
- Next.js host guidance: 2,237 Unicode scalar values
- combined with `\n\nProfile contract:\n`: 2,786 Unicode scalar values
- instruction limit: 2,500 Unicode scalar values
- host tail lost by the historical truncation:
  `d6e7649620e77e3565e430f70cb1a1fd63232302dd7b0a58766b8498ca9f353a`

The expected Issue #496 result is an explicit capacity rejection that retains the
complete model and host sources. This fixture is not expected to reach `Ready`.

`host-guidance.txt` contains the exact independent host source (no trailing
newline). Its SHA-256 is
`b2dd0b0c696f9c4b38489821dd2917273a8e139d0b43c8cd6bc7d66e0d0975d2`.
The model instruction SHA-256 is
`e32851275f8e288c11417d41915e6b7e6b4a42902114f136c7663160d73f33f2`.
The lost tail is exactly the final 344 bytes (344 characters) of the host text.
These hashes were checked against `stage-boundary-analysis.json` and the host
source in `P2-P0-comparison.json` from the referenced September 19 campaign.

Focused Rust tests execute this parsed-and-later fixture through profile
augmentation, canonicalization, sanitization, preset conversion and Admission.
They assert that rejection retains all 2,786 characters and both source scopes.
Removing an empty `Profile contract:` heading does not constitute a positive
case. Missing model text and the 344-byte host tail are independent negative
controls.

`refusals.json` enumerates duty/output/result/check/order loss controls after a
capacity retry. The bounded same-owner control starts with overlapping model and
host text, then preserves both verbatim in one complete host instruction. It
passes the existing preservation and lint checks. Existing Issue #478 and #484
fixtures cover the previously legal independent-owner splits; this fixture adds
no four-output split exception.

Additional focused controls cover 2,499/2,500/2,501 Unicode scalar boundaries with
readiness notes and delimiters, oversized host-only guidance, all augmentation
shapes, generic model-only truncation, real Python canonicalization movement and
deletion, ambiguous IDs, scaffold mutations, immutable package/marker capture,
configured contracts and Recovery origin, mixed schema/lint/empty retries,
last-valid and setup fallback rejection, and deterministic-template errors.
Application steps and verifiers remain unexecuted when the three proposals fail.
