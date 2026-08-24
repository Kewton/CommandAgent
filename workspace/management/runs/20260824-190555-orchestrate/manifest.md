# Orchestration Manifest

- Run ID: `20260824-190555-orchestrate`
- Created at: `2026-08-24T19:05:55+00:00`
- Repository: `CommandAgent`
- Start branch: `feature/issue-377-gui-trial-failed`
- Start commit: `5a89f1d3`
- Requested issues: `375, 370, 374, 376, 377`
- Phase: `plan`
- Max parallel: `1`
- Dry run: `true`
- Develop base: `origin/develop`
- CommandMate Codex agent: `codex`
- Dependency source: `explicit`
- Dependency overrides: #375<-none; #370<-#375; #374<-#370; #376<-#374; #377<-#376
- Approved decisions: #377: Propagate exact finalized dependency heads 375=1acfc81aa0ba7d7f338db4013d94df95e0d7d779, 370=9e8e178b97b49c78411ad9d2ba1783168227cdd9, 374=839f9c335a4af780a72433130e452ad984b87c3e, 376=810dd041a20cf73b03c1979cc66015d26cf65a6e into the existing feature/issue-377-gui-trial-failed worktree; preserve Issue 377 behavior; verify and commit only; do not push, mutate PRs or Issues, dispatch workers, or start/stop CommandMate.
- Worktree rows: one Issue per worktree

## Generated Artifacts

- `issue-analysis.md`
- `dependency-plan.md`
- `scheduler-report.md`

## User Questions

See `issue-analysis.md`.
