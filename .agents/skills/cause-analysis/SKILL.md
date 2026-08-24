---
name: cause-analysis
description: Produce an evidence-backed root-cause analysis for a CommandAgent defect without implementing a fix. Use when the user asks why a failure occurs, requests a reproduction path, or needs quick, permanent, and preventive countermeasures.
---

# CommandAgent Cause Analysis

Diagnose first; do not edit production code in this skill.

## Workflow

1. Read the issue/report and extract the observed behavior, reproduction, expected result, impact, environment, and cited files.
2. Reproduce when safe, or identify the strongest available evidence when reproduction is unavailable.
3. Trace the code and data path from trigger to symptom. Record relevant files and line-level evidence.
4. Separate:
   - trigger conditions
   - direct cause
   - root cause
   - contributing design or state assumptions
   - similar risks elsewhere
5. Try to falsify the leading hypothesis with focused tests, logs, history, and alternative paths.
6. Propose a minimal fix, a durable fix, and prevention/tests; compare scope, risk, and compatibility.
7. Recommend one course with reasons and explicitly list remaining unknowns.

If the user explicitly requests a CommandMate cross-review, use the Codex agent/instance and keep that task read-only. Do not start or stop CommandMate automatically, and treat sandbox localhost failures as `unreachable`, not proof that the server is stopped.

## Output

Write `dev-reports/issue-<number>/cause-analysis.md` when an issue number exists. Include reproduction path, evidence table, causal chain, falsification attempts, similar risks, countermeasures, recommendation, confidence, and unknowns. Update the GitHub issue only with explicit authorization.
