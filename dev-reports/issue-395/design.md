# Issue 395 design: create oracle synthesis shadow coverage

## Scope

Add Phase 2 shadow evaluation for `create` VerificationSpec proposals. The
change measures whether an LLM proposal covers reviewed acceptance claims with
specific, executable-strength oracle bindings. It does not call a provider,
execute a proposed command, emit an event, or participate in authoritative
verification, browser release gating, profile admission, adjudication, or
terminal projection.

## Design

- Add a leaf `verification_spec::create_shadow` module. Its reviewed input is a
  list of gold claims, each with required/optional status, minimum strength,
  and one or more exact expected input/observation bindings. Multiple expected
  bindings represent matrices such as two CLI inputs or separate UI copy and
  computed-style observations.
- Evaluate a parsed `ShadowGeneration` against the gold list by stable claim
  ID and exact typed binding equality. A build-only, pass-through, wrong
  port/path, wrong expected value, or missing matrix row cannot satisfy a more
  specific gold binding.
- Keep proposal and execution distinct. Provider lifecycle/result fields are
  not accepted as execution evidence. A caller must separately supply bounded
  `OracleExecutionEvidence` with an oracle ID, observed strength, outcome, and
  workspace-relative evidence path. The evaluator performs no execution.
- Reuse the registered declarative argv verifier policy for command-like
  candidates. Shell interpreters, inline code, setup/install commands, dev
  servers, workspace escapes, and other commands rejected by existing verify
  policy remain rejected. HTTP/DOM proposals describe observations against an
  already managed runtime; they do not gain server-start authority.
- Emit a versioned shadow coverage report with one row per gold claim. Each
  row records strategy, strength, executed, outcome, evidence paths, matched
  oracle IDs, and an explicit unverified reason when coverage is absent,
  unsafe, under-strength, unsupported, or lacks execution evidence.
- Compute `all_required_passed` only from every required row being executed,
  sufficiently strong, and passed. A provider failure, malformed/empty
  generation, or model-declared pass without external execution evidence
  therefore cannot manufacture full coverage.
- Represent currently unsupported negative-condition oracles in the gold
  input with an explicit reviewed unverified reason rather than inventing a
  weak absence check.

## Fixtures and tests

- Add golden Next.js and Python CLI proposals plus expected coverage snapshots
  for UI copy/style/interaction, explicit port/path, CLI aggregation, and a
  multiple-input matrix.
- Add negative conformance cases for build-only substitution, missing matrix
  rows, unsafe install/dev-server/shell candidates, unsafe evidence paths,
  unsupported negative conditions, provider failure, and model-declared pass
  without execution evidence.
- Add an Issue corpus contract fixture. Run focused create-shadow tests first,
  then schema/Phase 0 compatibility, corpus and guardrails, formatting,
  Clippy, and the full Rust suite because the shared command policy is reused.

## Compatibility

`VerificationSpec v0`, its prompt and JSON Schema, existing events,
CompletionContract, `.anvil/`, authoritative profile verifiers, browser release
gate, and admission cap remain unchanged. The new report is additive and
shadow-only.
