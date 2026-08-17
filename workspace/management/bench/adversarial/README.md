# CM-1a adversarial fixture seal

These fixtures are design inputs, not executable tests. They are intentionally
fixed before a validator exists so a later implementation can be audited for
overfitting. Every Phase 1 type has an attack input and a repair/re-entry pair.

Expected handling is fail-closed: the attack must produce `fail` or
`violation`; the repair may pass only after the same boundary remains enforced.
The fixture digest manifest is `sha256sums.txt`. CM-1a does not contain a
validator or any product-code change.

Types:

1. `core-edit-instruction`
2. `requirement-text-injection`
3. `forbidden-api`
4. `unapproved-package`
5. `build-time-egress`
