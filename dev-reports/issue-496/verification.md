# Issue #496 verification

- Status: `passed`

## Checks

- `cargo test --lib issue496`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

The focused Issue #496 suite passes all 22 tests; all 10 static guardrails pass.
The final full run exits 0, including library, integration and documentation
tests. Existing explicitly ignored tests keep their existing status.

## Verification scope

The saved parsed-stage fixture keeps the full 529-character model instruction,
2,237-character host guidance and 344-byte historical tail, rejecting all 2,786
combined characters for capacity. Its model, host and tail hashes match the
referenced campaign evidence.

The focused matrix covers Unicode 2,499/2,500/2,501 boundaries after readiness
notes and delimiters, all guidance augmentation shapes, host-only overflow,
model-only truncation, bounded same-owner preservation and lint, independent
saved model/host loss, outputs/results/checks/order loss, real Python
canonicalization movement/deletion, duplicate-ID provenance and source capture,
scaffold mutations, immutable package/marker evidence and acquisition failures,
capacity/formation ordering, schema/lint/empty retries, last-valid/setup fallback,
Recovery/configured contracts and deterministic templates.

## Environment and development findings

The sandbox full run encountered local HTTP-listener `Operation not permitted`
errors and was interrupted; the full verification therefore runs outside that
sandbox. No live provider/API probe is involved. A static guard initially counted
new test functions as production; explicit `#[cfg(test)]` attributes resolved the
classification without modifying guardrail code or baselines. No acceptance or
verification requirement was relaxed.
