# CM-2i AppSpec schema v0.1 replacement ceremony

Recorded at `2026-08-18T08:10:06+09:00` before golden-006.

## Pins

| revision | schema_version | schema SHA-256 |
|---|---|---|
| removed v0 | `community.app-spec/v1` | `73a0ceba54802185f5210ed2bffce207c765fe02771cb4f216fe4f6f7d695527` |
| canonical v0.1 | `community.app-spec/v0.1` | `80e4cb41eeb0f60eb04640e2ac8beac7d1414e7f5a9aa9fa563fd08d17ac7e0b` |

Final schema manifest SHA-256 is `6242f3549c8b7eea08dd75067fd7e338e24659b76079d03c6ed5185fa58572c1`. The amendment seals the positive computed chain, self-cycle negative, mutual-cycle negative, and pin-mismatch pair with the schema and pin.

## Ordered ceremony evidence

1. Saved the exact v0 bytes and pin outside the repository before editing. The observed old schema SHA matched the declared v0 pin.
2. Added v0.1 with computed `entity`, `same_entity` reference scope, `topological` evaluation, cycle=`violation`, and QUEUED `global_reference`.
3. Temporarily admitted both schema versions in the Rust product and Python reference validator. Two complete synthetic workspaces were prepared with identical v0.1 AppSpec bytes and their respective pinned schema versions.
4. Ran the product's offline profile verifier against both workspaces. Both exited 0, including S, Z, and B verification. The exact command shape was:

   ```text
   target/debug/commandagent --offline --profile community-mini-app --cwd <v0-or-v01-workspace> --prompt 'Validate app.spec.yaml against the pinned Community AppSpec schema and exit non-zero on violation.'
   ```

5. Removed v0 admission from both implementations. The same product command then exited 1 for v0 with the exact terminal text:

   ```text
   error: community_profile_violation:community_schema_version_invalid
   ```

6. The v0.1 workspace remained exit 0 after old-version removal. Focused Rust/Python parity and sealed manifest checks are run again on the final state before commit.

The old schema is not present in the final repository. Historical run evidence remains immutable. Golden suite manifest `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86` and adversarial manifest `792c9696ca86127966810ec4a376a3815c4fb93de4ad2c9d6aa205dad09a2b0b` were not changed by this ceremony.
