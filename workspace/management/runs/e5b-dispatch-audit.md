# E-5b profile dispatch audit

Date: 2026-07-29
Baseline: `1e44ab69fc3adc4a248c70c6f263568ecfaca45d`

## Scope and counting rule

This is the stage-1, investigation-only inventory for E-5b. No production
behavior is changed here.

The production portion of `src/planner/runner.rs` ends immediately before the
top-level test module at line 9843. Within that range, a **site** is one source
expression which either:

1. selects behavior from a profile value or profile literal; or
2. dispatches profile-dependent behavior through an existing helper/hook.

Imports, function definitions, error prose, event payload serialization,
profile values passed through without selecting behavior, and `#[cfg(test)]`
fixtures are excluded. Under this rule the baseline contains exactly **110
runner sites**. Comma-separated line numbers below are separate sites; ranges
denote one multi-line expression unless a site count says otherwise.

Outside `runner.rs`, related expressions are reported as **dispatch clusters**:
one cluster is a single responsibility in one module, and its location cell
lists every production line anchor in that responsibility. This keeps the
runner's review success metric (`110 -> residual`) exact without pretending
that a wrapper plus its registry implementation are independent migrations.

The inventory was built with `rg` searches for profile literals, canonical
profile conversion, the existing `DomainProfile`/`profile_behavior` hooks, and
the profile helper names imported by `runner.rs`, followed by a production/test
boundary review of every hit.

## Existing dispatch nucleus

`src/planner/profile.rs:127-443` already defines `DomainProfile`.
`src/planner/profile.rs:445-459` registers five implementations
(`nextjs`, `python-cli`, `data`, `ingest`, `generic`), and
`src/planner/profile.rs:941-946` performs the current lookup.

This is a useful nucleus, but it is not yet the requested single runtime:

- `src/planner/profile_behavior.rs:13` bypasses `DomainProfile` for `cli`.
- `src/planner/assurance.rs:8-19,34-58` branches directly for `data` and
  `generic`.
- `src/completion_metadata/{cli,data,ingest}.rs` performs a second projection
  dispatch.
- preset, setup, repair, browser, build, and verifier policy still contain
  direct profile tests outside the registry.
- the registry accepts `&str`, silently falls back to `generic`, and is looked
  up repeatedly instead of resolving one typed runtime at the boundary.

Therefore “already behind a hook” below means reusable migration material, not
that the site is already fully centralized.

## `runner.rs` inventory: 110 sites

### Count by branch kind

| Branch kind | Sites |
|---|---:|
| projection | 1 |
| acceptance runtime | 35 |
| repair boundary | 17 |
| preset selection | 14 |
| guidance injection | 23 |
| probe selection | 9 |
| other | 11 |
| **Total** | **110** |

## Migration progress

This TOML block is the machine-readable consumption ledger for the 110 runner
sites. A site moves to `consumed` only in the batch that removes its string
dispatch or makes the operation profile-independent; adding an adapter alone
does not consume a site.

```toml
[[migration_batch]]
batch = 0
kind = "typed runtime foundation"
status = "complete"
runner_sites = []

[[migration_batch]]
batch = 1
kind = "projection and probe selection"
status = "complete"
runner_sites = [3782, 219, 1449, 3800, 4035, 4087, 4199, 4927, 5486, 7946]

[[migration_batch]]
batch = 2
kind = "preset selection"
status = "pending"
runner_sites = [612, 622, 632, 809, 832, 2174, 2221, 2228, 8358, 8359, 8362, 8373, 8396, 8420]

[[migration_batch]]
batch = 3
kind = "repair boundary"
status = "pending"
runner_sites = [1804, 2306, 2563, 2732, 4617, 4633, 4667, 4670, 4723, 4741, 4755, 5091, 5096, 5098, 5546, 7074, 7327]

[[migration_batch]]
batch = 4
kind = "guidance injection"
status = "pending"
runner_sites = [530, 8945, 8948, 9117, 9126, 9195, 9204, 9258, 9271, 9349, 9358, 9587, 9589, 9591, 9592, 9594, 9596, 9701, 9703, 9705, 9706, 9708, 9710]

[[migration_batch]]
batch = 5
kind = "acceptance runtime and non-residual other"
status = "pending"
runner_sites = [1340, 1348, 1377, 1380, 1473, 1475, 1476, 1489, 1502, 1516, 1530, 1578, 1586, 2042, 2044, 2046, 2065, 3670, 3672, 3674, 3711, 3744, 3763, 4025, 4027, 4030, 4066, 4067, 4071, 4079, 4082, 5425, 5433, 5446, 5450, 1032, 1039, 1307, 1308, 1312, 1350, 1440, 1573]

[[migration_batch]]
batch = 6
kind = "intentional residual and guard"
status = "pending"
runner_sites = [1301, 1755, 3639]
```

### Projection

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/runner.rs:3782` | projection | all; specialized `data`/`cli`/`ingest` downstream | `assurance_for_completion` helper; downstream dispatch remains direct/hybrid | migrate to `ProfileRuntime::completion_projection`; remove runner lookup |

### Acceptance runtime

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/runner.rs:1340,1380,1489,1502,1516,1530,1578,5450` | acceptance runtime | `generic`, `nextjs`, `react`, `vite`, `web`, other interactive profiles | direct capability/evidence/obligation mapping or local helper | migrate to typed runtime contract methods |
| `src/planner/runner.rs:1348,1377,1473,1475,1476,1586` | acceptance runtime | all | existing `DomainProfile` inference/completion hooks | retain semantics; dispatch through resolved runtime |
| `src/planner/runner.rs:2042,2044,2046,2065` | acceptance runtime | all | inferred capabilities/evidence/obligations plus contract binding helpers | migrate to runtime contract object |
| `src/planner/runner.rs:3670,3672,3674,3711,3763` | acceptance runtime | all | same contract helpers in the final path | migrate to runtime contract object |
| `src/planner/runner.rs:3744` | acceptance runtime | browser-capable profiles | `final_acceptance_release_gate`; helper is still profile-aware | make release-gate policy a runtime method |
| `src/planner/runner.rs:4025,4027,4030,4066,4067,4071,4079` | acceptance runtime | `nextjs` and all profiles with invariants | mixed `DomainProfile` hooks and direct `is_nextjs_profile` | move invariant/excerpt policy behind runtime |
| `src/planner/runner.rs:4082` | acceptance runtime | all | `verify_profile_final` existing hook | dispatch through resolved runtime |
| `src/planner/runner.rs:5425,5433,5446` | acceptance runtime | all | existing expected-path and inferred-contract helpers | dispatch through resolved runtime |

### Repair boundary

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/runner.rs:1804` | repair boundary | `nextjs`, `python-cli`, generic Node/Python | direct dependency-setup family branch (continuation at 1806-1845) | runtime dependency policy |
| `src/planner/runner.rs:2306,2563` | repair boundary | all | `profile_auto_repair` existing hook | resolved runtime method |
| `src/planner/runner.rs:2732` | repair boundary | all | profile hook-snapshot target helper | resolved runtime method |
| `src/planner/runner.rs:4617,4633,4667,4670,4723,4755` | repair boundary | all | mixed repair prompt, auto-repair, and post-step hooks | resolved runtime repair policy |
| `src/planner/runner.rs:4741` | repair boundary | all | hook-snapshot helper | resolved runtime method |
| `src/planner/runner.rs:5091,5096,5098` | repair boundary | `nextjs` plus profile-owned paths | direct excerpt selection plus helper | runtime repair-target policy |
| `src/planner/runner.rs:5546` | repair boundary | all | inferred profile repair guidance helper | resolved runtime method |
| `src/planner/runner.rs:7074` | repair boundary | all | runtime acceptance repair guidance helper | resolved runtime method |
| `src/planner/runner.rs:7327` | repair boundary | `nextjs`, `next-js`, `next.js` | direct literal release-verification fallback | remove; runtime release policy |

### Preset selection

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/runner.rs:612,632,832` | preset selection | `data`, `ingest`, `nextjs`, other | existing step-policy helpers, not one registry | runtime plan policy |
| `src/planner/runner.rs:622` | preset selection | `data` | direct literal branch | runtime plan policy |
| `src/planner/runner.rs:809` | preset selection | all | `profile_deterministic_step_plan` existing hook | resolved runtime method |
| `src/planner/runner.rs:2174,2221,2228` | preset selection | profile-owned setup artifacts | existing setup/preset helpers | runtime plan policy |
| `src/planner/runner.rs:8358,8359,8362,8373` | preset selection | `nextjs`, `python-cli`, generic fallback | direct/hybrid fallback selection | registry-owned fallback policy |
| `src/planner/runner.rs:8396,8420` | preset selection | `data` | direct literal fallback branches | registry-owned fallback policy |

### Guidance injection

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/runner.rs:530,9358` | guidance injection | all | `profile_guidance` existing hook | resolved runtime method |
| `src/planner/runner.rs:8945,8948` | guidance injection | `nextjs`, other | mixed direct Next.js and existing ultra-plan rule hook | runtime plan guidance |
| `src/planner/runner.rs:9117,9195,9349,9587,9701` | guidance injection | all | expected-path helper repeated by phase | resolve once and ask runtime |
| `src/planner/runner.rs:9126,9204,9271,9589,9703` | guidance injection | all | quality-expectation helper repeated by phase | resolve once and ask runtime |
| `src/planner/runner.rs:9258` | guidance injection | all | setup-scaffold helper | resolved runtime method |
| `src/planner/runner.rs:9591,9592,9594,9596,9705,9706,9708,9710` | guidance injection | all | runtime contract, generation rules, inferred capability/evidence helpers | one runtime guidance bundle |

### Probe selection

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/runner.rs:219,1449,3800,7946` | probe selection | `nextjs` and browser-capable aliases | requested-port helper contains direct profile policy | runtime probe policy |
| `src/planner/runner.rs:4035,4087,4199,4927` | probe selection | `nextjs` | direct route/import-closure helper selection | runtime probe policy |
| `src/planner/runner.rs:5486` | probe selection | `cli`, `data`, `ingest`, other | `profile_behavior::run`; `cli` bypasses registry | fold into runtime behavior probe |

### Other

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/runner.rs:1032,1039,1301,1307,1308` | other (inference/promotion) | `generic` -> inferred profile | mixed direct canonical tests and `infer_profile` | registry inference and typed promotion |
| `src/planner/runner.rs:1312,1350,1440` | other (promotion contract) | all | existing setup-path/capability helpers | resolved runtime |
| `src/planner/runner.rs:1573` | other (promotion carry) | `generic` and promoted profiles | local generic-profile rule | typed promotion state |
| `src/planner/runner.rs:1755,3639` | other (telemetry identity) | all | canonical string normalization for emitted lifecycle data | intentionally retain the emission sites, but render from typed `ProfileId` |

The 110-site set is:

```text
219,530,612,622,632,809,832,1032,1039,1301,1307,1308,1312,1340,1348,
1350,1377,1380,1440,1449,1473,1475,1476,1489,1502,1516,1530,1573,1578,
1586,1755,1804,2042,2044,2046,2065,2174,2221,2228,2306,2563,2732,3639,
3670,3672,3674,3711,3744,3763,3782,3800,4025,4027,4030,4035,4066,4067,
4071,4079,4082,4087,4199,4617,4633,4667,4670,4723,4741,4755,4927,5091,
5096,5098,5425,5433,5446,5450,5486,5546,7074,7327,7946,8358,8359,8362,
8373,8396,8420,8945,8948,9117,9126,9195,9204,9258,9271,9349,9358,9587,
9589,9591,9592,9594,9596,9701,9703,9705,9706,9708,9710
```

`src/planner/runner.rs:9050` (`plan.profile != config.profile`) is deliberately
not in the set: it synchronizes model output with the configured identity and
applies the same operation for every profile value. It is not behavior
selection. It should still become a typed equality check in stage 2.

## Outside-`runner.rs` inventory

The table has 33 responsibility clusters. The line anchors inside each row are
the exhaustive production locations found for that responsibility; a cluster
can contain more than one expression. The count is therefore reported
separately from the runner's 110 raw sites.

### Outside count by branch kind

| Branch kind | Responsibility clusters |
|---|---:|
| projection | 5 |
| acceptance runtime | 9 |
| repair boundary | 4 |
| preset selection | 6 |
| guidance injection | 2 |
| probe selection | 2 |
| other | 5 |
| **Total** | **33** |

### Projection clusters

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/completion_metadata.rs:31`; `src/completion_metadata/intent/profile.rs:6-13` | projection | `generic`, `data`, other | direct dispatcher/literal checks | runtime projection |
| `src/completion_metadata/cli.rs:15-31` | projection | `cli`, `python-cli` | direct canonical match | `ProfileRuntime::completion_projection` |
| `src/completion_metadata/data.rs:14-18` | projection | `data` | direct canonical match | same |
| `src/completion_metadata/ingest.rs:15-29` | projection | `ingest` | two direct canonical matches | same |
| `src/planner/assurance.rs:8-19,34-58`; `src/planner/profile_admission.rs:21-22` | projection | `data`, `generic`, all registered profiles | direct/hybrid projection plus separate admission cap | runtime projection followed by registry metadata admission cap |

### Acceptance-runtime clusters

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/profile.rs:1010-1067` | acceptance runtime | all | existing `DomainProfile` verification/lifecycle hooks | move unchanged into expanded runtime trait |
| `src/planner/profile.rs:1122-1159,1171-1174` | acceptance runtime | all | existing expected-path, evidence-target, quality, completion hooks | move unchanged into runtime trait |
| `src/planner/profile_behavior.rs:12-22` | acceptance runtime | `cli`, all others | direct `cli` bypass followed by `DomainProfile` | eliminate bypass; runtime behavior probe |
| `src/planner/adjudication/create.rs:138-165,533-623,645-741,751-758,1055-1122` | acceptance runtime | all; browser aliases and `nextjs` specialized | mixed hook calls, canonical strings, admission, browser/release direct policy | adjudication consumes one resolved runtime |
| `src/minimal_loop/completion.rs:557-564,1912-1914`; `src/minimal_loop/hidden_path_feedback.rs:17-20` | acceptance runtime | generic vs known; all profile continuations | direct generic vocabulary plus `DomainProfile` hook | typed identity/runtime method |
| `src/minimal_loop/build_verifier.rs:388-419,446-474,660-669,1334-1370` | acceptance runtime | `nextjs`, `python-cli`, generic build profiles | existing build-oracle hook with two direct fallback lookups | runtime build policy; no literal fallback array |
| `src/planner/verify.rs:543-566,636-665,689-713,751-760,897-959,1029-1118,1338-1368,1453-1456,3068-3077`; `src/minimal_loop/verifier_env.rs:205` | acceptance runtime | `python-cli`, `nextjs`, all | mixed pass-through hook dispatch and direct literals | runtime verifier policy |
| `src/planner/profiles/ingest/phase_verify.rs:127-146`; `src/planner/profiles/data/step_policy.rs:72-84`; `src/planner/profiles/python_cli.rs:39-47` | acceptance runtime | `ingest`, `data`, `python-cli`/`cli` | profile-module-local literal guards | runtime implementation owns these checks; caller passes typed identity |
| `src/preflight.rs:79`; `src/tui/presentation.rs:982` | acceptance runtime | `nextjs` | two direct wrapper checks outside planner | typed runtime capability (`browser_preflight`) |

### Repair-boundary clusters

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/fix_contract_predicate.rs:90`; `src/planner/repair_targeting.rs:267-276`; `src/planner/repair_targeting/cli.rs:14` | repair boundary | `nextjs`, `cli` | direct profile wrappers/literal | runtime repair targeting |
| `src/planner/fix_runtime.rs:145-160,308-322,663-667`; `src/planner/fix_reproducer.rs:14`; `src/planner/fix_diagnostics/reproducer_execution.rs:17-21` | repair boundary | all | mostly existing hooks, repeatedly resolved | one runtime reference through fix flow |
| `src/planner/fix_runtime/data_isolate.rs:61-69`; `src/planner/fix_runtime/data_isolate/presence_filter.rs:61`; `src/planner/fix_runtime/data_role.rs:26,114,132,175`; `src/planner/profiles/data/pre_satisfied.rs:10-12` | repair boundary | `data` | direct literals and ID comparisons | data runtime repair/phase policy |
| `src/planner/profiles/data/repair_policy.rs:113-136`; `src/planner/profiles/data/repair_policy/claims_binding_guidance.rs:14-32`; `src/planner/final_acceptance.rs:410-417,965,2153-2200`; `src/planner/hook_snapshot.rs:204-230` | repair boundary | `data`, `nextjs`, all | mixed direct ID checks and existing hooks | runtime repair guidance/targets |

### Preset-selection clusters

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/profile.rs:492-517,989-1003,1074-1089` | preset selection | inferred `nextjs`/`python-cli`; all preset profiles | inference literals, canonical aliasing, deterministic-plan hooks | typed `ProfileId` parser plus registry inference/preset |
| `src/config.rs:491-507,551-554` | preset selection | `data`, `ingest`, inferred profiles | direct profile/intent default-preset matrix | registry metadata |
| `src/planner/fix_plan_synthesis.rs:103`; `src/planner/investigation_plan_synthesis.rs:122`; `src/planner/ingest_plan_synthesis.rs:81-82` | preset selection | `data`, `ingest` | direct registry-ID comparison | runtime plan synthesizer |
| `src/planner/setup_step_policy.rs:20-69,115-161` | preset selection | `data`, `nextjs`, other | mixed local data helper and direct Next.js wrapper | runtime setup-step policy |
| `src/planner/setup_step_policy.rs:170-317` | preset selection | `data`, `nextjs` | repeated artifact-knowledge branch | runtime-owned artifact policy |
| `src/planner/step_material.rs:10`; `src/planner/profiles/ingest/phase_verify.rs:33-62`; `src/planner/lint.rs:432-481` | preset selection | `ingest`, `nextjs` | direct ID/literal gates | runtime plan canonicalizer/linter |

### Guidance-injection clusters

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/profile.rs:768-781,1070-1071,1092-1118,1162-1168,1187-1214` | guidance injection | `rust`, `python`, `nextjs`, all registered profiles | generic-profile direct match plus existing guidance/repair hooks | runtime guidance bundle; generic is a normal implementation |
| `src/minimal_loop/loop_run.rs:1144,2921-2945,2978-3048` | guidance injection | all; `python-cli` direct cleanup gate | existing guidance/scaffold hooks plus one direct literal | resolve runtime once for minimal loop |

### Probe-selection clusters

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/minimal_loop/import_scan.rs:173`; `src/planner/adjudication/create.rs:168-235,304-309,761-762,1136-1165` | probe selection | `nextjs` and browser-capable profiles | direct literal/browser helper policy | runtime probe capabilities |
| `src/planner/profile.rs:958-985`; `src/planner/profile_behavior.rs:12-22` | probe selection | build-capable profiles and `cli` behavior | registry hooks plus hard-coded fallback pair and `cli` bypass | one registry-provided probe set |

### Other clusters

| Location | Branch kind | Profiles | Existing hook or direct branch | Proposed disposition |
|---|---|---|---|---|
| `src/planner/profile.rs:127-131,445-459,540-544,760,941-946` | other (registry/alias) | all | current registry plus data alias override and generic catch-all | **retain one central lookup**, replace strings/fallback with typed `ProfileId` |
| `src/planner/profile_manifest/validation.rs:7-18`; `src/planner/profiles/data/manifest.rs:164`; `src/planner/profiles/python_cli/manifest.rs:60`; `src/planner/profiles/ingest/manifest.rs:75` | other (manifest self-consistency) | manifest owner | direct identity equality inside schema validation | intentionally retain as schema invariant; use typed ID when schema migrates |
| `src/eval_events.rs:2251-2256,2358`; `src/planner/adjudication/core.rs:175` | other (event identity/fail-closed validation) | all | canonical string normalization and generic/empty checks | retain responsibilities; consume typed ID/rendered bytes |
| `src/tui/slash.rs:337-349` | other (user-boundary inference) | inferred profiles | calls existing inference helper | retain boundary call; registry owns inference |
| `src/planner/profile_admission.rs:9-18`; `src/workflow/schema.rs:141` | other (policy/schema registry) | all declared profiles | separate manifest/admission lookup and identity validation | keep policy metadata in registry; schema equality remains a validation |

## Direct versus existing-hook result

The runner's 110 sites split as follows:

- **73 existing-hook/helper sites**: behavior is already behind
  `DomainProfile`, `profile_behavior`, or a profile helper, but lookup is
  repeated and the helper can still branch internally.
- **35 direct/hybrid behavior sites**: profile literals, canonical-string
  matches, or helpers whose body contains a profile switch.
- **2 intentional telemetry sites**: canonical identity is rendered into
  lifecycle events (`runner.rs:1755,3639`).

“Existing hook” therefore measures reuse potential, not completion. Stage 2
must prove that each helper either becomes a `ProfileRuntime` method or is
profile-independent and can lose its profile argument.

## Proposed migration partition (awaiting adjudication)

### Branches that should disappear

1. Completion projection selection in `completion_metadata/*` and
   `assurance.rs`.
2. `profile_behavior.rs`'s `cli` bypass.
3. Direct preset matrices in `runner.rs`, `config.rs`, synthesis modules, and
   setup-step policy.
4. Direct repair/profile targeting in fix runtime and data-specific leaf
   modules.
5. Browser/import/build/verifier profile literals.
6. Repeated `domain_profile(profile)` and canonical-string dispatch at runtime
   call sites.

The proposed shape is:

```text
external string
  -> ProfileId::parse/canonicalize
  -> ProfileRuntimeRegistry::resolve(ProfileId)   # the one dispatch point
  -> &dyn ProfileRuntime threaded through plan/run/acceptance/repair/projection
```

`ProfileRuntime` should grow from `DomainProfile`; it should not create a
parallel second trait. Existing methods and byte-emitting code remain the
semantic implementation during migration.

### Branches intentionally retained

| Responsibility | Reason |
|---|---|
| One registry lookup in `profile.rs` | This is the requested dispatch point. Unknown-profile fail-closed behavior must be explicit; the current silent generic fallback needs adjudication. |
| `ProfileId` parsing and legacy aliases | External CLI/config strings must remain byte-compatible while becoming typed internally. |
| Registry-owned profile inference | Inference necessarily selects a profile, but should be data/methods on registered runtimes rather than a distributed literal chain. |
| Manifest ID/self-profile equality checks | They validate declarative schema consistency; they do not select runtime behavior. |
| Admission-policy lookup | Admission is registry metadata, applied after earned assurance. It must remain independently visible, not be hidden inside evidence evaluation. |
| Event/persistence rendering | Existing emitted strings must remain byte-identical. The sites render `ProfileId`; they do not choose behavior. |
| `plan.profile` versus configured-profile equality | Identity synchronization, not profile-dependent behavior. It becomes typed equality. |

### Proposed runner success metric

Subject to review adjudication:

- baseline behavioral sites: **110**;
- target profile-literal branches in `runner.rs`: **0**;
- target intentional non-dispatch identity/rendering sites from the baseline:
  **3** (`runner.rs:1301` inference boundary and telemetry at `1755,3639`);
- target registry resolution sites in the process: **1**, outside
  `runner.rs`, in the expanded `profile.rs` registry;
- helper calls may remain only when the helper is profile-independent or a
  method on the already-resolved runtime. They are not counted as profile
  branches after the string/profile switch has been removed.

## Stage-2 batch boundaries proposed for review

1. **Type and registry shell**: introduce `ProfileId`, expand
   `DomainProfile` into `ProfileRuntime`, preserve all current adapters.
2. **Projection + behavior probe**: fold `assurance.rs`,
   `completion_metadata/*`, and the `cli` bypass into runtime methods.
3. **Acceptance + probes**: final/invariant checks, browser/import/build/
   verifier selection.
4. **Preset + guidance**: synthesis, setup ownership, expected paths, phase
   guidance.
5. **Repair boundaries**: targeting, deterministic repair, snapshots,
   profile-owned reproducer/build policy.
6. **Call-site collapse**: resolve once at the boundary and thread the runtime;
   remove adapters only after byte-equivalence tests are green.
7. **Guard**: reject new profile literals/branches in `runner.rs`; explicit
   allowlist contains only typed identity rendering if still necessary.

Every batch should run `cargo check` first, then the existing snapshot,
conformance, and full suite unchanged. No snapshot update is part of this
migration unless separately adjudicated as an authorized schema change.

## Fifth-profile contact baseline

The current fourth-profile history records **26 touched files** for adding a
profile. Stage 2 should remeasure the same definition after the registry
migration. The target is not merely fewer files: a new profile should add one
runtime implementation, one manifest/contract family, focused fixtures, and a
registry entry without touching runner, completion projection, acceptance
dispatch, repair selection, or probe selection.

## Review questions before stage 2

1. Should unknown profile IDs continue to resolve silently to `generic`, or
   should typed parsing fail closed while preserving `generic` only for an
   explicit/default ID?
2. Is the proposed runner residual of three identity/rendering sites accepted,
   or must inference and event rendering also be moved fully outside runner?
3. Is admission metadata part of `ProfileRuntimeRegistry` while remaining a
   post-projection cap, as proposed?
4. Is the seven-batch order above accepted, or should repair be migrated before
   preset/guidance?

No stage-2 production change should begin until these points and the
disappear/retain partition above are adjudicated.
