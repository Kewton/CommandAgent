# Development Guardrails

## Runner Growth Tripwire

`src/planner/runner.rs` and `src/minimal_loop/loop_run.rs` are interim group-3
chokepoints. The extracted planner and minimal-loop leaf modules also have
file-level growth budgets so they do not become replacement chokepoints.

The CI/test guard records current baselines and fails if any file grows
above baseline +2%:

- `src/planner/runner.rs`: 4,557 lines
- `src/planner/runner/phase.rs`: 2,660 lines
- `src/planner/runner/acceptance.rs`: 2,523 lines
- `src/planner/runner/tests/**/*.rs`: 15,206 lines in aggregate
- `src/minimal_loop/loop_run.rs`: 7,444 lines
- `src/minimal_loop/repair_pressure.rs`: 746 lines
- `src/planner/repair_targeting.rs`: 597 lines
- `src/planner/final_acceptance.rs`: 2,235 lines
- `src/planner/adjudication/create.rs`: 2,186 lines
- `src/planner/ultra_plan_flow.rs`: 1,570 lines
- `src/planner/assurance.rs`: 50 lines
- `src/planner/profiles/nextjs.rs`: 3,684 lines
- `src/minimal_loop/evidence.rs`: 6,702 lines
- `src/planner/capability_catalog.rs`: 614 lines

The same guard also measures production code and `#[cfg(test)]` code
separately. The total baseline above remains enforced; these split baselines
prevent test growth from hiding production growth, or production shrink from
masking test bloat:

| file | production baseline | test baseline |
| --- | ---: | ---: |
| `src/planner/runner.rs` | 4,544 | 13 |
| `src/planner/runner/phase.rs` | 2,660 | 0 |
| `src/planner/runner/acceptance.rs` | 2,502 | 21 |
| `src/minimal_loop/loop_run.rs` | 4,960 | 2,485 |
| `src/minimal_loop/repair_pressure.rs` | 278 | 468 |
| `src/planner/repair_targeting.rs` | 459 | 138 |
| `src/planner/final_acceptance.rs` | 2,230 | 5 |
| `src/planner/adjudication/create.rs` | 2,172 | 14 |
| `src/planner/ultra_plan_flow.rs` | 1,570 | 0 |
| `src/planner/assurance.rs` | 50 | 0 |
| `src/planner/profiles/nextjs.rs` | 2,361 | 1,323 |
| `src/minimal_loop/evidence.rs` | 4,088 | 2,694 |
| `src/planner/capability_catalog.rs` | 407 | 207 |

When adding behavior, put new subsystems in new modules and call them from the
runner. Refactors that shrink these files are allowed; lower the baseline only
after the shrink is intentional and reviewed. Do not raise a baseline to admit
growth.

The runner test aggregate includes 8,440 lines mechanically transferred from
the former 8,429-line inline `runner.rs` test module (the delta is module
wiring) and all 6,765 lines already externalized under
`src/planner/runner/tests/`. The transferred tests are split by driver, phase,
acceptance, step-repair, compile-repair, and support responsibility; no new
support file exceeds 499 lines. Each test file also has its own baseline in
`tests/generality_guardrails.rs`; the aggregate and per-file checks prevent
relocation from becoming a growth-guard bypass.

Declarative Next.js and evidence knowledge must be changed in
`src/planner/profiles/nextjs/knowledge.toml` and
`src/minimal_loop/evidence_knowledge.toml`, respectively. Before adopting a
knowledge change, run and record the Space, Breakout, and Quiz scenario matrix
so vocabulary or contract drift is measured independently from Rust logic.

Profile manifests may reference only capabilities registered in
`src/planner/capability_catalog.rs`; free-form shell or logic-bearing templates
must not be registered. New capabilities require implementation, schema, golden
update, and tests in the same change.

CI is an acceptance necessary condition, not a substitute for the authorized
full-suite green run. The local acceptance conditions, including privileged
full verification where required, remain in force.
