# Progress Report Core Prompt

Generate a concise status report from current repository evidence.

## Data To Collect

- current branch and dirty state
- recent commits
- open PRs or issues when accessible
- test status from recent local commands if available
- source/test line counts when `cloc` is available
- active worktrees when relevant
- blockers, risks, and recommended next actions

## Suggested Report

```markdown
# Progress Report

## Summary
<1-2 sentence status>

## Numbers
| Item | Value |
| --- | --- |
| Branch | ... |
| Dirty state | ... |
| Recent commits | ... |
| Rust source LOC | ... |
| Rust test LOC | ... |

## Recent Work
- ...

## In Progress
- ...

## Risks
- ...

## Next Actions
- ...
```

Use `N/A` when data cannot be fetched. Separate evidence from inference.
