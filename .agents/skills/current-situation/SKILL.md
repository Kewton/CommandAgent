---
name: current-situation
description: Turn an observed CommandAgent defect or confusing behavior into a reproducible evidence-backed situation report and optional GitHub issue. Use when the user describes a failure and wants the current facts organized.
---

# CommandAgent Current Situation

Capture what happened before proposing a cause or fix.

## Workflow

1. Extract the observed behavior, trigger, expected behavior, actual behavior, frequency, and impact from the conversation.
2. Collect relevant command output, logs, versions, configuration shape, and source locations. Redact secrets and avoid copying unrelated environment data.
3. Distinguish facts, user observations, hypotheses, and unknowns.
4. Produce minimal reproduction steps. If reproduction is unavailable, state exactly which evidence substitutes for it.
5. Search current open issues for likely duplicates when GitHub access is available.
6. Draft a report containing:
   - situation summary
   - reproduction steps and frequency
   - expected and actual behavior
   - environment and version
   - sanitized logs/evidence
   - impact and safety concerns
   - related code paths
   - hypotheses clearly labeled as unconfirmed
7. If the user requested an issue, present the draft before creating it unless their request already authorizes immediate publication.

Use `Kewton/CommandAgent` for GitHub operations. Never publish secrets, absolute personal paths, tokens, or unredacted `.env` content.

## Completion Report

Return the evidence-backed summary, duplicate-search result, remaining unknowns, and issue URL only if one was created.
