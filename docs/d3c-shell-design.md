# D-3c PM Router and Boundary Dialogue Shell

Status: **fixed (2026-07-31)**

This document is the fixed D-3c implementation contract. Product-code changes
remain limited to the typed router, boundary gates, presentation, and REPL
wiring specified here; it does not authorize a new acceptance comparator,
automatic dispatch, mid-run intervention, or a pack-selection default.

The starting point is
[`docs/demo/d3c-handoff.md`](demo/d3c-handoff.md): the acceptance sheet is not
merely an end report. It is machine-produced material distributed across the
human boundary gates.

## 1. Question and answer

The question is how a user can ask for work in ordinary task language without
having to trust the assistant's conversational confidence.

The proposed answer is a thin PM router plus four boundary gates:

```text
request
  -> route proposal (profile × intent × task family)
  -> human confirmation
  -> unattended product execution
  -> full acceptance sheet
  -> close, or human-selected next-action proposal
  -> human confirmation again
```

The router is deliberately lightweight. It proposes where a request should be
sent; it does not earn completion, assurance, or even route correctness. A
wrong route is caught by the selected profile/intent contract and produces an
honest failure. Therefore the router should optimize for deterministic,
inspectable suggestions rather than act as another adjudicator.

The shell's core principle is:

> Every consequential claim displayed at a human boundary comes from a typed
> registry, fixed contract, pinned pack, event, evidence file, capability band,
> or generated acceptance sheet. Conversation is navigation, not authority.

## 2. Route proposal

### 2.1 Typed output

A route proposal has the following logical shape. Field names are design names,
not a committed serialization schema.

| Field | Source | Rule |
|---|---|---|
| `profile` | `ProfileId` + `ProfileRuntimeRegistry` | Closed registered value; aliases normalize before display |
| `intent` | `IntentId` + strict `IntentSchema` registry | One of create/fix/investigate in v0 |
| `task_family` | proposed `TaskFamilyId` catalog | Closed value for the selected profile × intent, or typed `unknown` |
| `basis` | deterministic observation list | File type, verb, workspace state, or explicit user binding; never an opaque score |
| `alternatives` | same typed catalogs | Sorted candidates that survived deterministic rules |
| `classifier_used` | fixed boolean + model pin when true | True only after deterministic ambiguity |
| `contract_ref` | resolved profile/intent contract | Required before confirmation |
| `full_meaning` | tracked band label | Exact profile-specific sentence, including testimony state |
| `pack_candidates` | admitted compatible pack registry | ID, version, exact-byte hash; no invented ID |
| `status` | fixed `proposal` | A route proposal is never `earned`, `full`, or `accepted` |
| `confirmation_required` | fixed `true` | No proposal may directly dispatch execution |

E-5b already provides the single typed profile runtime resolve point.
Implementation should add a read-only iterator to that registry rather than
copy profile strings into the shell. `IntentId` is already typed; its runtime
catalog must likewise expose the strict registered schemas instead of
duplicating create/fix/investigate strings.

E-5a's typed stop-class and violation IDs remain failure vocabulary. They may
explain workspace state or a failed run, but must not be reused as task-family
IDs. Doing so would conflate “what job is this?” with “how did it stop?”.

### 2.2 Task-family catalog gap

Task family is the one route dimension that is not currently centralized in
Rust. Formal bands use code-owned Python classifiers and suite names:

| Profile × intent | Current formal families |
|---|---|
| nextjs × create | Quiz, Breakout, Space |
| data × create | aggregation, timeseries |
| python-cli × create | stats, filter |
| ingest × create | list, table |
| nextjs × fix | compile-error fix, contract-hook fix |
| data × investigate | pipe, schema |

The fixed design does not hide that gap and does not solve it with free text.
Implementation batch 0 adds a typed `TaskFamilyId`/catalog whose entries
are derived from the existing formal band family definitions and guarded
against the cross-language classifiers. Until a profile × intent has such an
entry, the only legal result is typed `unknown`; the router may not invent a
new family to make the card look complete.

This catalog is descriptive and pricing-oriented. Selecting a family does not
alter a profile contract or award assurance.

### 2.3 Deterministic rule layer

Rules run in fixed precedence order and emit their observations. A later rule
does not silently overwrite an explicit earlier binding.

1. **Explicit typed binding.** A valid user-supplied profile, intent, or family
   narrows candidates. An unknown value is rejected with the registered list;
   it is not passed through as an `Other` route.
2. **Workspace file structure.** Inspect a bounded, sorted inventory. Examples:
   a Next.js dependency plus an App Router tree proposes `nextjs`; `cli/main.py`
   plus CLI usage artifacts proposes `python-cli`; `pipeline/main.py` with
   tabular inputs proposes `data`; `data/snapshots/` plus ingest inspection
   proposes `ingest`.
3. **Request verbs and objects.** Create/build/make language proposes `create`;
   repair/fix language with a failing artifact proposes `fix`; diagnose,
   investigate, reproduce, or root-cause language proposes `investigate`.
   These rules use a reviewed bilingual lexical table, not a model-generated
   synonym list.
4. **Workspace state.** An empty requested workspace strengthens create.
   Persisted failure/recovery evidence strengthens fix or investigate.
   Bound diagnosis carry strengthens fix. Existing successful deliverables do
   not by themselves imply fix.
5. **Family rules.** Run only inside the resolved profile × intent candidate
   set. They reuse the formal band classifiers and input shapes: for example,
   month-over-month/moving-average language distinguishes data timeseries,
   CLI count/sum/mean distinguishes stats, a frozen table/list snapshot
   distinguishes ingest table/list, and compile evidence distinguishes the two
   current fix families.

The rule result is:

- one candidate: propose it and show every basis;
- multiple candidates: send only those closed candidates to the ambiguity
  classifier;
- no candidate: use the relevant catalog plus `unknown` as the classifier
  choice set;
- contradictory explicit bindings: do not call the classifier; ask the human
  to correct the request.

Workspace inspection follows existing bounded-inventory rules: no symlink
traversal, no dependency/build trees, normalized relative paths, fixed caps,
and an explicit omission count.

### 2.4 LLM ambiguity classifier

The LLM is used only when the deterministic layer is ambiguous. Its prompt
contains:

- the original request;
- bounded deterministic observations;
- the closed candidate triples;
- one machine-readable output schema;
- the instruction that it is choosing a proposal, not adjudicating capability.

It cannot return arbitrary IDs. The response is decoded into the typed
candidate set; an invalid, missing, or multi-valued answer becomes `unknown`
with the parse reason. Provider, model, prompt version, candidate set, and raw
response hash are recorded as proposal provenance, not acceptance evidence.

Human confirmation is mandatory after both deterministic and LLM proposals.
The LLM never executes, changes a pack, expands a contract, or chooses a
failure recovery action.

## 3. Boundary dialogue lifecycle

The shell has a small state model separate from the E-5f execution-phase state
machine:

```text
collecting
  -> route_proposed
  -> awaiting_confirmation
  -> confirmed
  -> running
  -> acceptance_ready | failure_ready
  -> closed | next_action_proposed
  -> awaiting_confirmation
```

Only `confirmed -> running` dispatches product work. Any route, model tier,
provider, pack pin, workspace, goal, or next-action change creates a new
proposal and returns to `awaiting_confirmation`.

### 3.1 Gate 1 — request confirmation

The confirmation card contains:

1. the exact user request and workspace;
2. proposed profile × intent × family, deterministic bases, alternatives, and
   whether the ambiguity classifier was used;
3. the resolved contract and its checks before execution;
4. acceptance-sheet §3 “definition of done” material;
5. the applicable capability-band value, denominator, measurement date/arm,
   and exact **Full meaning** label;
6. planner/executor/provider/preset and any known measured limitation;
7. the selected pack or explicit `no pack`, including ID, version, exact-byte
   hash, compatible point, and whether a comparable band row exists;
8. unknown or unmeasured values rendered as such, never estimated silently.

The human can confirm, edit the request/bindings, choose an admitted compatible
pack, or cancel. Confirmation persists the exact card identity and selected
pins. The card does not claim that the task will pass; it states what will be
tested and what the current value tag means.

Why conversation need not be trusted: all consequential card values are
resolved from registries, contracts, bands, or pack conformance. The prose
router rationale cannot alter them.

### 3.2 Gate 2 — unattended execution

After confirmation the shell constructs the existing product command/config
from the frozen proposal and hands off to the normal runner. Execution has no
clarifying dialogue and no plan edits from the PM shell. Existing cancellation
and safety aborts remain available, but they stop the run; they are not an
interactive route or contract mutation.

The shell may display progress already emitted by the product. It may not
reinterpret progress as completion, auto-select a repair, swap models, change
packs, or answer a runner question on the user's behalf.

For StepPlan task progress, the shell projects only schema-versioned
`plan_step_started`, `plan_step_completed`, and `plan_step_failed` records.
`plan_execution_id` defines an execution interval and `step_execution_id`
defines one task attempt; neither Step ID nor event adjacency is a valid merge
key. Completed, short-circuited, failed, and interrupted states come only from
the matching typed terminal record. A terminal stream without a complete typed
contract is `unsupported`, not an inferred set of successes.

Why conversation need not be trusted: execution consumes the persisted typed
confirmation, not the latest chat text. Existing runner events, evidence,
budgets, and honest-failure terminals remain authoritative.

### 3.3 Gate 3 — acceptance

On a terminal result, the shell presents the complete generated acceptance
sheet, not a conversational summary in place of it. Navigation may highlight:

- verdict and assurance;
- the same Full meaning label shown at confirmation;
- every bound contract check and result;
- evidence paths/hashes and pack pins;
- elapsed epochs and cost fields that were actually recorded;
- discrepancies between the confirmed value tag and the executed identity.
- every typed StepPlan task interval, including bounded failure and verification
  evidence, while older sessions remain explicitly unsupported.

The user may acknowledge/close or inspect an evidence item. A friendly one-line
heading is allowed, but it cannot omit, rewrite, or upgrade the sheet.

Why conversation need not be trusted: the acceptance sheet is derived from the
tracked events/evidence/meta body. The shell neither recomputes nor substitutes
the verdict.

### 3.4 Gate 4 — failure and next action

For non-full or failed terminals, the shell presents the full sheet plus §5
plain-language stop reason and only these typed next-action proposals:

The GUI result-detail implementation first projects one bounded
`FailureExplanation` from the final continuation interval. Its public leaf
model keeps the exact schema-v1 Plan/Step identity, failure location/category,
command and verification evidence, completed progress, workspace/artifact
state, and `recovery_prompt_saved` handoff fields separate. An earlier failed
interval is never combined with a successful continuation, and legacy or
incomplete identity pairs degrade to `unknown`. Planning, execution,
verification, release-gate, infrastructure, interrupted, and unknown are
display classifications only; they do not change the terminal verdict or gate.

Recovery-document opens are authenticated GETs confined to the current
per-session workspace and to the exact projected repair-prompt or Recovery Plan
path. Copy actions never execute a command. Applying a recommendation only
prefills the existing additional-request draft, after which credential
scrubbing, exact-byte review, and the separate confirmation remain mandatory.

| Action | Required basis | Effect |
|---|---|---|
| retry | retry remains contractually allowed and the cause is not a deterministic repeat | Creates a new run proposal; never consumes a retry automatically |
| recovery circle | recovery YAML and an admitted workflow edge exist | Proposes the existing circle with its carry/evidence requirements |
| elevated model | an admitted provider/model configuration is available | Changes the model pin and value tag; returns to Gate 1 |
| pack change | an admitted compatible pack exists | `/pack <id@version>` changes exact pack identity; returns to Gate 1 and invalidates direct A/B comparability unless recorded |
| close | always | Records no further action |

The shell shows why unavailable actions are disabled. It does not infer that a
retry is wise, create a workflow edge, admit a pack, or classify a model failure
from conversational sympathy. The human chooses; every consequential choice
returns to confirmation.

Why conversation need not be trusted: availability is computed from typed
terminal/evidence/registry state, and selection alone is not execution.

## 4. Pack-selection surface

Gate 1 lists only packs that satisfy all of the following:

- loaded by the strict pack decoder;
- schema/conformance green;
- compatible with the selected profile, intent, source, and injection point;
- contract-floor merge green;
- exact-byte pinned and not retired;
- supply eligibility is explicit: an `admitted` pack exactly matches the
  in-repository admitted registry, a `repository` pack is labelled unapproved,
  or a `local` pack resolves from the configured extension-root and is labelled
  unapproved and band-unmeasured;
- identified by exact bytes, not merely ID/version.

`PackSource` is the single typed supply vocabulary:
`admitted | repository | local`. Repository and local packs are selectable only
by an explicit pin; they do not inherit admission from a matching ID/version.
Local GUI enumeration/selection requires the Trial token, and mutation also
requires the existing Origin check. An unpinned directory or merely present
YAML file is never a Gate 1 candidate.

`No pack` is an explicit choice and preserves the existing product path.
Builtin compatibility packs that preserve historical bytes are implementation
detail unless they change the user-visible measurement identity.

The REPL accepts `request --pack <id@version>` before the first Gate 1 and
`/pack <id@version>` from an available Gate 4 `pack_change`. Both freeze the
catalog hash, point, and source in a fresh confirmation. `/packs` uses the same
renderer as the direct `--packs` action, and confirmed dispatch records the
installed identity as `pack_injected` without replacing existing run events.

The shell displays:

```text
cli-assist@1.1.0
sha256:3d11e126...
source: 承認済み
point: cli-validation
source: 承認済み
band: measured / unmeasured
```

The required supply displays are `承認済み`, `リポジトリ（未承認）`, and
`ローカル（未承認・帯域未計測）`. A local card also displays
`pack 固有保証なし（既存 profile/intent の earned assurance のみ）`. If the
extension root shadows the same repository `id@version`, the card adds
`ローカル優先: 同名のリポジトリ pack より拡張ルートを優先`.

A pack added after a card was rendered invalidates that card only if the user
chooses it; the previous exact hash remains reproducible. Every source or hash
change creates a new Gate 1 identity and confirmation. Unsigned operator-local
supply is now a D-3c selection path, but signed/remote supply, publisher trust,
and revocation remain Phase G scope. Local supply is neither Phase G signature
work nor an admission path.

Next.js T1 is currently a Rust/contract acceptance floor, not an eval pack.
The shell must describe that fact rather than invent a `nextjs-eval` selection.

## 5. Product placement

D-3c integrates with the existing product REPL UX track; it is not a new bench
front end and not a second product runner.

Proposed leaf layout:

```text
src/tui/boundary_shell/
  mod.rs                 # dialogue lifecycle only
  route.rs               # typed proposal and deterministic rules
  ambiguity.rs           # closed-candidate LLM adapter
  confirmation.rs        # Gate 1 card + persisted identity
  acceptance.rs          # Gate 3/4 sheet and next-action presentation
  family_catalog.rs      # typed band-family catalog/guard
```

Minimal wiring belongs in `src/tui/repl.rs` and the existing slash/natural
request dispatch boundary. `src/repl.rs` remains the thin entry point.
`src/planner/runner.rs`, runner leaf modules, acceptance comparators, and
workflow adjudication do not gain dialogue logic. The shell calls existing
typed configuration and execution surfaces after confirmation.

Management-side readers may expose the existing acceptance-sheet and band
records through a small Rust adapter or a generated, versioned catalog. Runtime
must not import Python classifiers or scrape Markdown ad hoc. If a catalog is
generated, cross-language guard tests must fail when a band family, Full
meaning label, admitted profile, or pack identity is missing.

The additive audit events are
`route_proposed`, `route_confirmed`, and `next_action_selected`. Their payloads
carry typed IDs, deterministic bases, card/hash identity, and classifier
provenance. They do not carry assurance and do not change existing event bytes.
Event-baseline and ordered-lifecycle fixtures must be updated before enabling
the shell.

## 6. Size and calibration estimate

This is the fixed E-3/E-4-style implementation estimate. D-3c adds no
new comparator or assurance surface, so the “new comparator 500–1,000 lines”
term is zero. Its cost is typed routing, presentation plumbing, and calibration.

| Component | Production Rust | Tests/fixtures | Main risk |
|---|---:|---:|---|
| registry iterators + typed family catalog/guard | 180–320 | 180–300 | cross-language family drift |
| deterministic router + bounded workspace observations | 300–500 | 300–500 | false deterministic uniqueness |
| ambiguity classifier adapter | 150–260 | 180–300 | unregistered/unstable output |
| four-gate lifecycle and persisted confirmation | 350–600 | 350–600 | dispatch before confirmation |
| card/sheet/band/pack presenters | 260–450 | 260–450 | conversational omission or stale pin |
| REPL wiring | 80–150 | 100–180 | bypass path / UX regression |
| **Total estimate** | **1,320–2,280** | **1,370–2,330** | — |

The E-4 revised calibration rule applies: inspect the machine floor in the
three categories of transmission, semantics, and stage design, then budget
**5–10 campaigns** rather than assuming 1–2 turns. At minimum the campaign
matrix covers:

- deterministic unique, deterministic ambiguous, and no-family routes;
- invalid LLM output and human correction;
- each of the four boundary terminal paths;
- no-pack and admitted-pack confirmation;
- retry, circle, elevated, and pack-change next-action proposals;
- one third-party P-3 user loop after D-3c admission.

Success is not “the router guessed correctly once”. It is:

1. zero execution before a persisted human confirmation;
2. zero unregistered profile/intent/family/pack IDs;
3. exact agreement between confirmation identity, product run identity, sheet,
   and band label;
4. honest contract failure for wrong confirmed routes;
5. a user can make the next decision from machine evidence without trusting a
   conversational claim.

## 7. Verification plan

Before live measurement, implementation must provide:

- registry coverage guards for profile, intent, family, contract, Full meaning,
  and admitted pack identities;
- table-driven deterministic-route tests using real workspace/goal shapes;
- invalid/ambiguous classifier fixtures proving mandatory confirmation;
- a production-path test proving no runner call occurs before confirmation and
  exactly one occurs after it;
- Gate 1 golden cards for measured, unmeasured, no-pack, and pack-pin cases;
- Gate 3/4 goldens from existing full/failed/circle acceptance sheets;
- next-action tests proving every change returns to Gate 1;
- ordered event fixture and byte-compatibility checks for existing product
  execution/events/evidence;
- corpus coverage for any request or recovery contract whose event/corpus shape
  changes;
- fmt, clippy, full Rust suite, Python checks, scrub, CI, and acceptance green.

The first live arm should use an already admitted profile/intent and compare
the shell path with the direct CLI path. Product verdict, assurance, evidence,
and event bytes after dispatch must agree; only the additive boundary records
may differ.

## 8. Permanent scope exclusions

D-3c does not include:

- free-form assistant conversation as a product feature;
- intervention, replanning, or pack/model swapping during execution;
- learning or online adaptation of router rules;
- an LLM-created profile, intent, family, contract, pack, or next action;
- automatic retry, circle dispatch, elevated fallback, or pack admission;
- new comparator/evaluator semantics;
- changes to E-5f execution-phase control;
- signed external pack supply;
- replacing the full acceptance sheet with a summary.

## 9. Review adjudication

The six design questions are resolved as follows:

1. Human confirmation is mandatory even for a deterministic unique route.
   Explicit non-interactive product CLI actions remain outside the REPL shell;
   no REPL execution path bypasses confirmation.
2. Task families use the typed catalog and cross-language guard in §2.2.
   Catalog misses resolve only to typed `unknown`.
3. Implementation uses the leaf layout in §5 with minimal REPL wiring.
4. The three additive boundary audit events in §5 are accepted.
5. Hard cancellation remains a safety stop; semantic mid-run intervention
   remains out of scope.
6. The production estimate is fixed at **1,320–2,280 Rust lines**, with
   **5–10 calibration campaigns**. Tests and fixtures retain the separate
   **1,370–2,330 line** planning range.

## 10. GUI Trial route projection

The management GUI projects the same D-3c lifecycle through four fixed,
static-export-compatible routes:

- `try/` owns editable launch identity and exact Gate 1 confirmation.
- `try/status/?session=<id>` owns read-only observation of a nonterminal run.
- `try/history/` owns a compact session index without inline diagnosis.
- `try/history/detail/?session=<id>` owns terminal verdict, diagnosis,
  acceptance, events, and artifacts.

This is an information-architecture boundary, not a lifecycle or authority
change. Launch still requires the exact confirmation hash and active-lease
admission. Status, history, and detail use GET-only projections. Existing
verification, acceptance, honest-failure, event names, event schemas, and the
legacy `.anvil/` read namespace remain unchanged. The runtime Trial token stays
in base-path-scoped `sessionStorage`; only the session ID may appear in a URL.

Status and detail also show one per-session working-directory panel. Its
absolute `sessions/<id>` path is derived from the same `SessionPaths` value
used for delegated `current_dir` and `--cwd`, while the run-record directory,
`events.jsonl`, and `summary.md` are labeled separately. Absolute paths are
available only from the GET-only `api/sessions/{id}/paths` endpoint when Trial
token authentication is enabled and satisfied; they remain absent from public
projections. The endpoint rejects invalid IDs, traversal, symlinks, and
out-of-root resolution, and projects a deleted workspace as `missing` without
recreating it or claiming that artifacts remain.
