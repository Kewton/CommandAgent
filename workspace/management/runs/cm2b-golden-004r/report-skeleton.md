# bench report skeleton: community-golden-warikan-20260817-144751

This skeleton transfers mechanical observations only. A human reviewer must decide UAT pass/fail, failure class, retry consumption, and settlement.

## Preflight record

- HEAD: `5e9c60265e8fa885c2a424f4b4f9af7ae55d7a49`
- minimum ancestor: `not specified`
- NODE_ENV: `production`
- deviations: `1`

## Event search method

The harness recursively parses JSON lines from each run artifact using file glob `.anvil/runs/**/events.jsonl`, reads the exact `event` field, and applies these regular-expression patterns:


## Failure class display (non-adjudicating)

# Failure class classification

| run | class id | attribution | stop pattern |
|---|---|---|---|
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_001/.anvil/runs/01a01031-e8f3-7920-8825-38467f6ef160` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_002/.anvil/runs/01a01031-e964-7432-ab6b-fe9d0064f8fb` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_003/.anvil/runs/01a01031-e9cd-7790-affb-a2456eff5f31` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_004/.anvil/runs/01a01031-ea33-7a00-b5b2-cecf348a29b1` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_005/.anvil/runs/01a01031-ea98-7a73-b7e7-2ff69e418c45` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_006/.anvil/runs/01a01031-eaff-7d23-af4e-38a647216925` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_007/.anvil/runs/01a01031-eb64-7aa3-8401-db2acb88fe44` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_008/.anvil/runs/01a01031-ebca-7db2-b73f-fa668567144e` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_009/.anvil/runs/01a01031-ec34-7aa1-b603-24e574046b00` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_010/.anvil/runs/01a01031-ec9d-7522-b724-0e2f56289d27` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_011/.anvil/runs/01a01031-ed06-77e0-9053-33108ce98625` | process_failure | model | failure_kind:process_failure |
| `/private/tmp/cm2f-root/community-golden-warikan-20260817-144751/artifacts/warikan_012/.anvil/runs/01a01031-ed6b-71f1-a8fc-a413d454ac49` | process_failure | model | failure_kind:process_failure |

## UNKNOWN runs

- なし
- `intent_resolved`: `^intent_resolved$`
- `host_env_normalized`: `^host_env_normalized$`
- `fix_reproducer_suggested`: `^fix_reproducer_suggested$`
- `*_plan_synthesized`: `^[a-z0-9_]+_plan_synthesized$`
- `*_adjudicated`: `^[a-z0-9_]+_adjudicated$`

## Run matrix (mechanical transfer)

| run | harness status | product exit | seconds | verdict transfer | assurance transfer |
|---|---|---:|---:|---|---|
| warikan_001 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_002 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_003 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_004 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_005 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_006 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_007 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_008 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_009 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_010 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_011 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |
| warikan_012 | completed | 1 | 0 | failed | partial (acceptance_not_full_success) |

## Acceptance sheets

- `warikan_001`: `artifacts/warikan_001/acceptance-sheet.md`
- `warikan_002`: `artifacts/warikan_002/acceptance-sheet.md`
- `warikan_003`: `artifacts/warikan_003/acceptance-sheet.md`
- `warikan_004`: `artifacts/warikan_004/acceptance-sheet.md`
- `warikan_005`: `artifacts/warikan_005/acceptance-sheet.md`
- `warikan_006`: `artifacts/warikan_006/acceptance-sheet.md`
- `warikan_007`: `artifacts/warikan_007/acceptance-sheet.md`
- `warikan_008`: `artifacts/warikan_008/acceptance-sheet.md`
- `warikan_009`: `artifacts/warikan_009/acceptance-sheet.md`
- `warikan_010`: `artifacts/warikan_010/acceptance-sheet.md`
- `warikan_011`: `artifacts/warikan_011/acceptance-sheet.md`
- `warikan_012`: `artifacts/warikan_012/acceptance-sheet.md`

## Event firing counts

| run | intent_resolved | host_env_normalized | fix_reproducer_suggested | *_plan_synthesized | *_adjudicated |
|---|---:|---:|---:|---:|---:|
| warikan_001 | 1 | 1 | 0 | 0 | 0 |
| warikan_002 | 1 | 1 | 0 | 0 | 0 |
| warikan_003 | 1 | 1 | 0 | 0 | 0 |
| warikan_004 | 1 | 1 | 0 | 0 | 0 |
| warikan_005 | 1 | 1 | 0 | 0 | 0 |
| warikan_006 | 1 | 1 | 0 | 0 | 0 |
| warikan_007 | 1 | 1 | 0 | 0 | 0 |
| warikan_008 | 1 | 1 | 0 | 0 | 0 |
| warikan_009 | 1 | 1 | 0 | 0 | 0 |
| warikan_010 | 1 | 1 | 0 | 0 | 0 |
| warikan_011 | 1 | 1 | 0 | 0 | 0 |
| warikan_012 | 1 | 1 | 0 | 0 | 0 |

## Terminal reasons (verbatim transfer)

### warikan_001

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_002

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_003

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_004

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_005

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_006

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_007

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_008

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_009

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_010

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_011

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

### warikan_012

````text
provider request failed after 2 attempts: Ollama request failed: error sending request for url (http://localhost:11434/api/chat) Hint: Start Ollama with `ollama serve`, verify `--ollama-host http://localhost:11434`, then run `commandagent --doctor`.
````

## Human review fields

- UAT pass/fail: 
- Failure class / attribution: 
- Retry-consumption decision: 
- Settlement comment: 
