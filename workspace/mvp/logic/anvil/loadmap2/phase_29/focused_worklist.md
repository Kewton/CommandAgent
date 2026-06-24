# Phase29 Focused Worklist

Date: 2026-06-23 JST

Status: completed / closed_proven

Focused proof is required for Phase29 rows that change model-facing repair
instructions, recovery packets, tool policy visible to the model, or eval
assertion fields. Pure internal boundary audits can close with unit/report
tests only when `row_closure_matrix.md` records why focused proof is not
needed.

Focused cases live under:

```text
eval/cases/focused/control-recovery/runtime-support/
```

## Candidate Cases

| case id | rows | purpose | required assertions |
| --- | --- | --- | --- |
| `phase29-language-repair-adapter` | C34 | Language diagnostic maps to an admitted mechanical proposal. | `language_repair_adapter_status=projected`; adapter remains evidence/proposal. |
| `phase29-effective-tool-policy` | C35 | Selected owner/action projects allowed and disallowed tool categories. | expected tool policy fields, rejected disallowed category, no provider/model branch. |
| `phase29-tool-failure-recovery` | C36 | Prose-only/schema/stale-edit/tool-parse failure becomes bounded correction or safe stop. | `tool_failure_recovery_status=bounded_correction`; existing tool-protocol fields remain visible. |
| `phase29-setup-command-classification` | C37 | Bash command is classified as verifier/setup/blocked with setup authority. | command class, setup authority, evidence binding, explicit setup policy. |
| `phase29-workspace-candidate-policy` | C38 | Workspace walk records candidates and ignored-dir exclusions. | candidate status, ignored-dir policy, exclusion reason, no ownership bypass. |
| `phase29-job-report` | C39 | Job report exposes active owner, action status, attempt outcome, and stop reason. | report fields are present and match expected owner/action/lifecycle. |
| `phase29-scaffold-contract` | C40 | Scaffold failure routes as setup/artifact obligation. | scaffold artifact role, owner/action, completion evidence, no scaffold workflow engine. |
| `phase29-noncoding-evidence` | C41 | Docs/data/research/ops deliverable binds to generic evidence. | evidence kind, binding status, completion authority, no prose-only closure. |
| `phase29-answer-work-mode` | C42 | Answer-only request is admitted without mutation, coding request is not over-blocked. | mode gate, allowed mutation status, final-answer contract result. |
| `phase29-lifecycle-projection` | C43 | Recovery lifecycle emits explicit stopped/exhausted/interrupted state. | lifecycle state, terminal reason, no hidden continuation. |
| `phase29-provider-boundary` | C44 | Provider request plumbing remains transport-only. | prompt/request boundary fields or offline provider test evidence; no behavior policy in provider. |

## Expected Field Families

Focused cases may assert existing or newly added fields from these families:

- active job and recovery owner/action fields;
- tool policy / allowed tool category / disallowed action;
- setup lifecycle and command authority;
- workspace scope, candidate, and exclusion reason;
- artifact role, ownership, completion evidence, evidence binding;
- repair task status, correction status, safe-stop reason;
- job report lifecycle and attempt outcome;
- provider boundary / parser / transport-only evidence when reportable.

Do not add broad string assertions when a structured field can express the
same expectation.

## Recheck Command

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/runtime-support \
  --out eval/runs/loadmap2-phase29-runtime-support-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture

python3 scripts/eval_report.py \
  <phase29-focused-root> \
  --cases-dir eval/cases/focused/control-recovery/runtime-support \
  --recheck
```

Executed root:

```text
eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335
```

Recheck result: `passed_recheck: 11`.

## Acceptance Criteria

- Every added focused case passes recheck.
- Any Phase29 row without focused proof has an explicit no-focused-needed
  rationale in `row_closure_matrix.md`.
- Unknown/raw failures are absent or mapped to a later blocker with owner and
  proof.
- Focused cases do not rely on provider/model-specific behavior.
- Focused cases do not require network or dependency installation.

## Review Result

Review findings applied:

- Made focused proof conditional by behavior type, not by convenience.
- Listed candidate cases for all C34-C44 rows so omissions must be justified.
- Required structured assertions and avoided prose-only report checks.
- Preserved offline/no-network evaluation constraints.
