# Phase32 Focused Worklist

Date: 2026-06-23 JST

Status: completed / reviewed

## Policy

Phase32 does not plan new model-facing behavior, focused assertions, or runtime
profile changes. Therefore no new focused eval case is required at planning
time.

Focused work becomes required only if implementation discovers one of these
conditions:

- final sign-off fails with a focused assertion mismatch;
- a phase-local proof root is missing or stale;
- a final report claim cannot be tied to a row-specific proof;
- a new distinct responsibility class is discovered.

## Existing Focused Evidence To Reuse

| purpose | root |
| --- | --- |
| focused control-recovery sign-off | `eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638` |
| runtime-support focused fixture | `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335` |

## Conditional Additions

If a focused case is added during Phase32, record:

| field | requirement |
| --- | --- |
| case id | Stable case path under `eval/cases/focused/...`. |
| expected assertion | Exact assertion field and expected value. |
| coverage row | C-row or FC-row that requires the case. |
| owner layer | Planning, execution, recovery task, profile, setup, verifier, eval/report, or provider transport. |
| proof root | Fresh eval root and recheck command. |
| closure condition | Assertion passes and broad sign-off remains green. |

## Review Notes

- Keeping this file explicit prevents hidden focused-case work during final
  closure.
- The default is no new focused work; final broad sign-off is still mandatory.
