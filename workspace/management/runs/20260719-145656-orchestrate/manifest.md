# Orchestration Manifest

- Run ID: `20260719-145656-orchestrate`
- Created at: `2026-07-19T14:56:56+00:00`
- Repository: `CommandAgent`
- Start branch: `develop`
- Start commit: `2ea60c5`
- Requested issues: `11, 12, 13, 14, 15, 16, 17`
- Phase: `plan`
- Max parallel: `2`
- Dry run: `true`
- Develop base: `origin/develop`
- CommandMate Codex agent: `codex`
- Dependency source: `explicit`
- Dependency overrides: #11<-none; #12<-#11,#13,#14,#15; #13<-#11,#15; #14<-#11,#13,#15; #15<-none; #16<-#12,#13,#14,#15; #17<-#16
- Approved decisions: #17: Adopt Option A. Preserve data-anvil-*, <anvil_tool_call>, anvil_app, .anvil/, JSON keys, event names, and schemas. Change only docs/mechanism-ledger.md to record this decision; make no production-code changes.

## Generated Artifacts

- `issue-analysis.md`
- `dependency-plan.md`
- `scheduler-report.md`

## User Questions

See `issue-analysis.md`.
