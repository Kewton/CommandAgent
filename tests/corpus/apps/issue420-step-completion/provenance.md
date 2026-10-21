# Issue #420 reopened regression fixture

The read-only source is `/Volumes/SSD_NX/tmp/commandagent-capability-0905.vkR3MS`.
The supplied 2026-09-07 reference re-audit is authoritative. Each of the nine
original events files was SHA-256 checked against the corresponding
`CommandAgent-develop/workspace/tmp/0905/runs/<Run>/result.json` before projection.
`fixtures/campaign-events.json` records those hashes, session IDs, and exact
1-based event lines. It projects all 19 implement short-circuits with their
matching start/terminal events, successful tool counts before the shortcut, and
relevant completion requirement and page-role fields. It is not a raw log copy.
Missing historical step IDs are retained in the original short-circuit objects;
the separately recovered IDs come from matching step boundaries.

The disjoint historical counts are 8 skipped, 2 failed, 3 completed with changes,
6 completed without changes; 11 shortcut events lack step IDs. All 11 iteration
probes observed the page role as scaffold. Later repair writes account for the
three changed completions; they are not counted as unchanged failures. Tool names
alone do not establish filesystem changes. Historic outcomes remain separate
from the new replay expectations.

The source page is copied byte-for-byte from S3 and is identical to E3's page.
Its SHA-256 is `013b9e9015d3da335226c7f55a34c9da4cd4523921674b25e5210a36b88e2ed8`.
The API route is an explicitly minimized stand-in for previously implemented
backend routes: it reproduces their route-bound implementation role without
copying irrelevant business logic. This fixture's intentionally passing run-wide
acceptance reproduces the #422-only false positive; the step-local guard must
still reject the placeholder. Other runtime/release requirements are not waived.

Focused tests replay the iteration decisions and model Read/Write/Bash turns
against this source. They also exercise proven existing satisfaction and
configuration pre-satisfaction separately. This is not a rerun of the nine-run
campaign, a build/browser measurement, or proof of full business correctness.
Historical originals and runtime namespaces were not changed.

The S3 `package.json` is also copied unchanged to retain the historical
`build_command_or_dependency_missing_boundary` source evidence (the `next build`
script is declared, not executed). SHA-256: `9b897bb8402d38f57ca730a2b1265757a4fc8181528daa63d49729253d4b61c7`.
