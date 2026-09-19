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
