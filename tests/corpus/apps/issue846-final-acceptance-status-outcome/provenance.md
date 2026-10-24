# Issue #846 fixtures

Read-only source: campaign `G1-3a4`, stage 3a, case `cli-q` (python-cli), L-104 run
on develop `d72fb320`. Source file, with the user's home replaced by `<user>`:

`<user>/share/work/gen-eval-1022/campaign-G1-3a4/stage3a/cli-q/work/.commandagent/runs/01a16a6b-6432-7d70-805a-925b74b69537/events.jsonl`

Source `events.jsonl` sha256: `89a7be91f8d6c6d3cb7920cc57cf49dd6b3aaa69ab74342c2953c510fbe517d6`.

`fixtures/events.jsonl` keeps one selected-field line each of `ultra_final_acceptance`,
`ultra_final_acceptance_failed`, `final_acceptance_repair_exhausted`, and `run_stop`.
It is a portable regression fixture, not a raw log. Values are as recorded before the
fix: the run failed the profile invariant (two `src/<package>/main.py`), exit 1 and
`completion_status: failed`, yet `ultra_final_acceptance` and `run_stop` carried
`final_acceptance_status: "full_success"` with release gate `pass`. No secret and no
absolute path is stored. The historical run directory was not edited.

The lib test `recorded_cli_q_run_maps_failed_report_to_incomplete` in
`src/planner/adjudication/final_outcome/tests.rs` feeds the recorded gate status and the recorded
failed-report marker into `final_acceptance_status_for_outcome`, so the fixture fails
the test if the status falls back to the gate alone.
