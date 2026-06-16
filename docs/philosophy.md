# Philosophy

CommandAgent is a minimal local-first coding agent for local and API-backed
LLMs. The design favors small deterministic control surfaces over large
agent-side orchestration.

## Principles

- Keep one execution engine. There is no legacy engine switch.
- Prefer deterministic checks before adding feedback mechanisms.
- Keep runtime guards narrow and observable.
- Do not turn profiles into hidden applications.
- Split large work into explicit steps instead of relying on a single long
  conversation.
- Treat evaluation scripts and docs as part of the product.

## Non-goals

- Historical compatibility with older agent engines.
- Sidecar routing.
- Memory retrieval, case memory, or anti-pattern corpora.
- Complex autonomous project managers.
- Provider-specific behavior that cannot be expressed through the shared
  provider contract.

## Admission Rule

New mechanisms must start from observed failures. A change is preferred when it
removes ambiguity, makes deterministic facts visible, or narrows an existing
contract. Adding another feedback loop is the last resort.
