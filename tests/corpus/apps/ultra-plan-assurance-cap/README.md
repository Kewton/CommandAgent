# ultra-plan-assurance-cap

Issue #985. A run that passes final acceptance ends with `ultra_plan_complete`, whose
`assurance_level` and `assurance_reason` come from the profile runtime. Before #985
that value was recorded without the admission cap that final acceptance applies, so a
draft profile could end with `ultra_final_acceptance` at `static` and
`ultra_plan_complete` at `full`. The runner now passes the value through
`profile_admission::cap_assurance` before the event, keyed by the same
`ProfileId::parse(&plan.profile)` that final acceptance uses.

## Cases (`cases.json`)

| Case | Profile | Status | Runtime value before the cap | Expected |
|---|---|---|---|---|
| `standalone` | `cap-standalone` (manifest, no base runtime) | draft | `full`, `""` | both events `static`, `profile_not_admitted`; `ok: true` |
| `standalone-failed` | `cap-standalone` | draft | `full`, `""` | final acceptance fails; no `ultra_plan_complete` |
| `overlay` | `cap-python-cli` (overlay on `python-cli`) | draft | `full`, `""` (from the base) | both events `static`, `profile_not_admitted`; `ok: true` |
| `admitted` | `python-cli` | admitted | `full`, `""` | `ultra_plan_complete` keeps `full`, `""` |

Each case holds the `UltraPlan`, the fake planner replies (one step plan per phase),
the fake executor replies (the files each phase writes), the status and uncapped
runtime value it pins before the run, and the event fields the run must record.
`moved::ultra_plan_complete_caps_draft_profiles_in_the_corpus_cases` in
`src/planner/runner/tests/ultra_plan_flow_tests.rs` registers `extension-root/` in a
process of its own and drives each case through `run_ultra_plan` to the events.

What does not change: event names, field names and types, `ok: true` and when the event
is written, and levels at or below `static` (`static`, `failed`, `reduced` keep their
level and reason; `profile_admission::tests::draft_caps_only_assurance_above_static`).
The two events still compute their values separately; only the cap is shared, so their
values are not required to match for every profile.
