# Archived multi-file and assignment contracts (#457)

Base: `../issue456-create-recovery/original`, byte-identical to the approved
develop-root archive `workspace/tmp/0909/result/orchestrate-451452-resume-20260909/original`.
Its provenance records source hashes and event SHA-256. This directory adds overlays,
the matching archived lockfile and extracted diagnostic trials. Originals remain
unchanged. It does not replay the old #448 export/Promise fixture.

| Case | Type diagnostics | Build | Functional result |
| --- | ---: | --- | --- |
| original | 33 | fail | not run |
| role-only | 32 | fail | not run |
| missing-list | 0 | pass | no member can be selected |
| wrong-field | 0 | pass | assignment PATCH returns 400; previous assignment survives reload |
| aligned | 0 | pass | listing, selection, assignment, reload and unassignment pass |

These are diagnostic counts, not independent defect counts. `role` changes the one
optional annotation. `types` aligns store errors, result objects, assignment objects
and the generic mutex return; the corrected GET remains dynamic. `members` supplies
two documented sample members through the existing projects GET. `wrong-field`
restores the saved assignee/assigneeId field mismatch using a type-valid request
shape, isolating that functional defect from the original ID/object type error.

The original goal requires team assignment but specifies no directory provider.
The sample team is a fixture decision. Select controls hold IDs; POST/PATCH/store
hold complete `Member {id,name,role}` objects, null unassigns, and omission means no
assignment update. Successful mutations return `{item}` and list routes `{items}`;
the projects response also supplies `{members}`. Store errors are structured
`StoreError`; HTTP errors are `{error: string, details?}`. The archived UI helper
checks Response.ok and displays that string. Existing project/task JSON storage is
retained, without making JSON persistence a new R0 requirement.

Run from the repository root, using a verified installed Playwright module/browser:

```sh
node scripts/issue457_nextjs_contracts.mjs --work-root /tmp/NEW_457_WORK --output /tmp/NEW_457_RESULT.json --playwright-module /absolute/path/to/node_modules/playwright
ISSUE457_TOOLCHAIN_ROOT=/tmp/NEW_457_WORK/original cargo test --lib issue457_installed_toolchain -- --ignored --nocapture
```

The harness refuses existing work/output locations, installs the frozen lock once,
checks all controls with strict TypeScript, applies the same noEmit and Next build
to every variant, and runs the real compiled page/API/store through isolated
headless Chromium. The only build resource override is `experimental.cpus: 2`,
identical for every variant. No type/lint gate is disabled. Functional checks also
preserve status filtering/deletion and verify real corrupt-store 5xx responses,
readable error text and unchanged damaged/related files. Browser evidence includes
version/settings, initial read/fill/screenshot and result screenshots.

The normal Rust corpus test checks source manifests and focused profile tests
exercise real API-contract verification. The explicitly ignored toolchain test is
run separately against a materialized original, because normal cargo tests require
neither Node dependencies nor a browser. It observes the real build failure, compares
the old one-frame extraction with 33 retained diagnostics and inspects repair prompts.

The additional request check runs at final contract verification and as repair
guidance; existing intermediate invariants retain their scope so read-only Recovery
inspection can reach the repair phase without configuration mutation.

Static inference is intentionally partial: only a proven local checked JSON wrapper,
literal payload keys, a resolved route and exhaustive recognized request-body use
with guarded success establish a hard mismatch. Unknown helpers, parameters shadowing
the helper, direct/computed mixed body access, dynamic returns and default-only
handlers are not rejected. Optional collection supply is repair guidance, not a
failure. `controls.json` includes passing controls for these boundaries. Bounded
compiler output may be incomplete on larger applications. These fixture results are
not live-model improvement, full R0 acceptance, or permission to promote a candidate
on build success alone.
