# Workflow node Config propagation audit (D-3a-3c commit 0)

Audit baseline: `e977ce65d6c1f2d06eb75f675fbde301e7a5294c`.

This is an exhaustive inventory of the resolved single-run `Config` consumed
by a workflow node. The four provenance classes are `node declaration`,
`global inheritance`, `origin derivation`, and `fixed value`. The audit-state
column records the implementation before D-3a-3c fixes; it is retained as the
regression inventory even after the implementation is corrected.

| Field | Provenance and required value | Audit state | Finding / remediation target |
|---|---|---|---|
| `workspace_root` | origin derivation: canonical `--origin` | correct | Already rebound and confinement-checked. |
| `state_dir` | origin derivation: inside the concrete node run | **wrong** | Global state directory could receive node session output. |
| `eval_events_path` | origin derivation: concrete node run `events.jsonl` | **wrong** | Retained repository-root path; caused circle-001 leakage. |
| `completion_contract_path` | global inheritance | correct | Read-only explicit contract remains caller policy. |
| `yes` | fixed value: `true` | correct | Non-interactive node execution is forced. |
| `offline` | global inheritance | correct | Caller network policy is preserved. |
| `context_budget` | global inheritance | correct | Node schema v0.1 does not override it. |
| `model` | node declaration, else global inheritance | correct | v0.1 pair already propagated. |
| `provider` | node declaration, else global inheritance | correct | v0.1 pair already propagated. |
| `prompt_layout` | global inheritance | correct | Measurement layout is caller policy. |
| `plan_preset` | node declaration-derived default from intent/profile; preserve an explicit global override | **wrong** | Inherited the workflow action's pre-node default (`none`). |
| `intent_override` | node declaration | correct | Set from node intent. |
| `planner_model` | global inheritance | correct | Planner override is outside schema v0.1. |
| `planner_provider` | global inheritance | correct | Planner override is outside schema v0.1. |
| `ollama_host` | global inheritance | correct | Provider endpoint remains caller policy. |
| `num_predict` | global inheritance | correct | Executor budget remains caller policy. |
| `max_iterations` | global inheritance | correct | Bounded-run policy remains caller policy. |
| `chat_timeout_secs` | global inheritance | correct | Provider timeout remains caller policy. |
| `chat_timeout_source` | global inheritance | correct | Must remain paired with timeout value. |
| `field_sources` | fixed from its twelve member rules below | **wrong** | Container retained incorrect profile/plan provenance. |
| `chat_retries` | global inheritance | correct | Retry policy remains caller policy. |
| `stream` | global inheritance | correct | Output policy is preserved; node is still non-interactive through `yes`. |
| `resume` | fixed value: `None` | **wrong** | A workflow CLI `--resume` could leak into an independent node run. |
| `fresh_session` | fixed value: `true` | **wrong** | One node must be one fresh run/session. |
| `no_footer` | global inheritance | correct | Presentation policy only. |
| `narration` | global inheritance | correct | Presentation policy only. |
| `profile` | node declaration | **wrong** | circle-001 ran all nodes as `generic`. |
| `profile_explicit` | fixed value: `true` | **wrong** | A declared node profile must be explicit. |
| `profile_inference` | fixed value: `None` | **wrong** | Inference must not compete with a declared node profile. |
| `style` | global inheritance | correct | Style remains caller policy. |
| `action` | origin derivation plus node declaration: `Prompt(derived origin goal)` | **wrong** | Used the invented `起点run` placeholder. |
| `field_sources.model` | node declaration (`workflow_node`) or global inheritance | correct | Already paired with effective model. |
| `field_sources.provider` | node declaration (`workflow_node`) or global inheritance | correct | Already paired with effective provider. |
| `field_sources.planner_model` | global inheritance | correct | Planner fields are out of schema v0.1 scope. |
| `field_sources.planner_provider` | global inheritance | correct | Planner fields are out of schema v0.1 scope. |
| `field_sources.context_budget` | global inheritance | correct | No node override. |
| `field_sources.chat_timeout_secs` | global inheritance | correct | No node override. |
| `field_sources.prompt_layout` | global inheritance | correct | No node override. |
| `field_sources.plan_preset` | node-derived default or explicit global source | **wrong** | Did not record `default_investigate_data`/`default_fix_data`. |
| `field_sources.profile` | fixed value: `workflow_node` | **wrong** | Retained the workflow launcher source. |
| `field_sources.narration` | global inheritance | correct | No node override. |
| `field_sources.footer` | global inheritance | correct | No node override. |
| `field_sources.stream` | global inheritance | correct | No node override. |

## Count and value of the exhaustive audit

- Audited rows: 43 (31 top-level Config fields plus all 12
  `ConfigFieldSources` members; the container row is included because it is a
  top-level field).
- Correct: 31
- Missing: 0
- Wrong: 12

The five externally observed circle-001 defect classes are not one-to-one
with Config rows. Profile/default-resolution failure expands to seven rows
(`plan_preset`, `profile`, `profile_explicit`, `profile_inference`, the
`field_sources` container, and its profile/plan members); event leakage maps to
`eval_events_path`; placeholder goal maps to `action`. The other two observed
classes (run-id divergence and sparse circle evidence) are runtime/evidence
structures outside Config. The exhaustive audit additionally found three
latent wrong Config fields not required to explain the five observations:
`state_dir`, `resume`, and `fresh_session`. Thus the audit records 12 wrong
rows versus five observed classes, a difference of seven rows, and adds three
independent latent confinement/session defects to the remediation queue.

All twelve wrong rows must be corrected by commits 1–3. Runtime run-id and
circle-evidence completeness are corrected separately by commits 2 and 4.
