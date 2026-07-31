# nextjs-t1-001 UAT report

## Scope and pin

- Campaign: `nextjs-t1-20260731-060218`
- Suite: `nextjs-t1` (`sha256:96accf52490d0aa3f54e5660637d1c2d3e17539e174769b4dec11664b4d2abd5`)
- Profile / intent / preset: `nextjs` / `create` / `profile`
- Planner: `qwen3.6:27b-coding-nvfp4` (`ollama`)
- Executor: `qwen3.6:35b-a3b-coding-nvfp4` (`ollama`)
- Family: Quiz, three runs with the same goal and no input source
- Measurement root: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0731_nextjs_t1_001`
- Final preflight binary: `commandagent 0.1.0 416953b 2026-07-31T06:25:14Z`
- Preflight: `cargo test` exit 0; release build exit 0.

Run 1 completed under the earlier commit-3 object `74dbc1f`; the post-product
acceptance-sheet reader then encountered an array-valued JSON file in `.next`.
Resume recovered its already-recorded product terminal without rerunning it.
Runs 2–3 used `416953b`. `git diff --quiet 74dbc1f 416953b -- src` returned 0:
the intervening changes were bench consumer/archive fixes and the Rust product
tree was identical.

## Run matrix

| Run | Product exit | Verdict | Assurance | Terminal | T1 | Recognized / matched / violations / unrecognized | Duration |
|---|---:|---|---|---|---|---:|---:|
| `quiz_qwen35_001` | 0 | complete | full | completed | claims_absent | 0 / 0 / 0 / 0 | 281s |
| `quiz_qwen35_002` | 0 | complete | full | completed | claims_absent | 0 / 0 / 0 / 0 | 489s |
| `quiz_qwen35_003` | 0 | complete | full | completed | claims_absent | 0 / 0 / 0 / 0 | 202s |

- Honest terminal: 3/3.
- Existing Next.js browser acceptance: 3/3 ready and 3/3 interaction passed;
  each interaction recorded `interaction_success=true`, `state_changed=true`,
  `visible_state_changed=true`, and `input_state_change=true`.
- T1 production startup: 3/3. Each artifact contains
  `.anvil/evidence/nextjs-testimony-binding.json`.
- T1 claim-bearing reach: 0/3. None of the three workspaces contained one of
  the contracted testimony sources (`README.md`, `GOAL_RESPONSE.md`,
  `goal-response.md`, or `output/response.md`).
- Total product time: 972s. Campaign wall interval from first product start to
  last product end: 1,905s.

## T1 evidence audit

The following complete material result is byte-identical in shape across all
three runs (the envelope epoch is run-specific):

```json
{
  "check_id": "nextjs_testimony_binding",
  "status": "claims_absent",
  "claims_absent": true,
  "recognized_claim_count": 0,
  "matched_claim_count": 0,
  "violation_count": 0,
  "unrecognized_prose": 0,
  "claims": [],
  "source_files": [],
  "compared_evidence": [
    ".anvil/evidence/browser-readiness.json",
    ".anvil/evidence/browser-interaction.json"
  ],
  "violations": [],
  "evidence_envelope": {
    "envelope_version": 1,
    "family": "T",
    "kind": "testimony_binding",
    "claims": [],
    "nearest_miss": [],
    "source_refs": [
      ".anvil/evidence/browser-interaction.json",
      ".anvil/evidence/browser-readiness.json"
    ]
  }
}
```

This is the intended staged-assurance behavior from the fixed contract:
`claims_absent` records T1 but neither promotes nor lowers the existing
Next.js assurance. Consequently, `full` here means the pre-existing build and
real-browser contract passed; it is not evidence that a model-authored
testimony claim was matched. A T1 violation would project to `failed`.

The extractor/comparator’s recognized paths were fixed separately with the
real Quiz fixture under `tests/corpus/apps/nextjs-testimony-quiz/`: route and
interaction matched, score and retry claims violated, and unrelated prose was
counted as unrecognized. This campaign adds the live `claims_absent` branch as
the v0.1 adjudication input.

## Harness observations

Two post-product harness gaps were exposed without changing product outcomes:

1. acceptance-sheet scalar rendering assumed every JSON file was an object;
   `.next` includes array-valued JSON. The reader now treats non-object JSON as
   absent for labelled-field lookup, and run 1 was recovered from its persisted
   terminal rather than rerun.
2. source-less archives copied reproducible `.next` and `node_modules` trees,
   causing scrub findings. Empty-workspace archives now exclude `.next`,
   `node_modules`, and `target`; focused tests fix both exclusion and
   idempotent re-archive behavior.

After deterministic re-archive, the scrub command returned
`{"ok": true, "findings": []}` for all 3/3 artifacts. No credentials or raw
provider material are included in this repository record.

## Adjudication

- P0 honest terminal: pass (3/3).
- False success: zero. Existing browser readiness/interaction evidence passed
  in every full run.
- T1 startup existence: pass (3/3 evidence files).
- T1 extraction distribution: `claims_absent=3`, `matched=0`, `violation=0`;
  recognized claims 0, unrecognized prose 0.
- v0.1 material: the current Quiz creation path does not itself guarantee a
  testimony-source artifact. Whether such a document becomes a contract floor
  is a later review decision; this measurement does not infer one.

