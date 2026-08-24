---
name: github-lifecycle-write
description: Perform explicitly authorized, narrowly scoped GitHub Issue or pull-request lifecycle mutations for CommandAgent with read-before-write checks and an audit of exact targets. Use for closing or reopening Issues, changing PR draft/readiness state, or other Issue/PR state changes. Do not use for implementing review feedback or resolving review threads.
---

# GitHub lifecycle write

Apply only the lifecycle mutation the user authorized. Merge authorization does not imply Issue-close authorization, and Issue-close authorization does not imply label, milestone, assignee, PR-review, or branch mutation.

## Capability discovery

Use the available skill/tool catalog by stable capability name. Never locate or read a versioned plugin cache directory to discover a GitHub skill. Prefer the purpose-built GitHub connector when its callable mutation is available; otherwise use authenticated `gh` from the repository root. If neither is available, report the target and blocker without substituting browser automation.

For review comments, implementation changes, thread replies, review submission, or thread resolution, use `github:gh-address-comments`; those actions are outside this skill. Use `github:github` for broad read-only repository inspection when needed, while keeping the mutation here narrowly scoped.

## Mutation contract

1. Record the repository, object kind, exact number, intended transition, and the user instruction that authorized it.
2. Read current state immediately before writing. Treat an already-satisfied state as `already-applied`, not as a reason to issue another write.
3. Reconfirm any prerequisite that justified the transition. For orchestration Issue closure, require the mapped PR result to be `merged`; do not infer this from a branch name or from an Issue reference alone.
4. Execute one narrowly scoped mutation. Do not bundle labels, comments, milestones, assignees, review actions, or branch deletion unless each was authorized.
5. Read back the resulting state or require a successful structured response. Record `applied`, `already-applied`, or `blocked`, including the exact target and non-sensitive evidence.

Use `--repo Kewton/CommandAgent` when repository context is not unambiguous. Never expose auth tokens in commands, output, or evidence.

## Issue close

For an authorized close after a mapped merge:

```bash
gh issue view ISSUE --repo Kewton/CommandAgent --json number,state
gh issue close ISSUE --repo Kewton/CommandAgent --reason completed \
  --comment "Implemented and merged into develop via PR #PR."
gh issue view ISSUE --repo Kewton/CommandAgent --json number,state
```

Skip the close when the Issue is already closed. Block when its PR mapping is absent, the mapped PR was not merged, current state cannot be read, or the write/readback fails.
