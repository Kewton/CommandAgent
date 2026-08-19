# Issue 119 design: visibility-aware GUI status and resource refresh

## Context and predecessors

- The Shell already owns one `useRuntimeStatus` instance and shares its result
  through React context, but that poller continues while the document is hidden.
- `useResource` currently loads only on mount/resource changes and replaces a
  successful value with `null` after a later failure.
- Issue 100 added lifecycle-driven Trial history refresh and a Terminal fragment
  link, but the runtime badge has no navigation and the Terminal target has no
  explicit highlight contract.
- Required predecessor Issue 122 reorganizes the GUI guide, and Issue 115 builds
  on it. The current branch is their direct ancestor, so fast-forward through
  `feature/issue-115-gui-pack-pin` before implementation and edit the new
  reader-oriented document owners rather than restoring the old monolith.

## Design

1. Keep `Shell` as the sole runtime-status owner. Change its timeout loop to
   avoid starting requests while `document.visibilityState` is `hidden`, resume
   immediately on the next visible transition, and retain the existing
   one-request-at-a-time sequencing and last successful result on failure.
2. Make `useResource` a small stale-while-revalidate hook. Revalidate on window
   focus and visible `visibilitychange`, abort superseded/unmounted requests,
   and preserve the last successful data when a refresh fails. Existing users
   on Overview, repository run records, Measurements, and Extensions gain the
   behavior without page-specific polling.
3. Extend `routePath("try")` with an optional session query. Render the runtime
   session badge as a base-path-safe link to that session. Pass a highlighted
   session ID from the current Trial observation to the history panel, mark
   rows with `data-session-id`, and let the Terminal link scroll to and
   temporarily highlight the exact row without adding navigation or mutation.
4. Add one exported ja-JP date-time formatter in `gui/lib/format.ts`; make the
   Overview/run list, Trial monitor freshness, and Trial history timestamps use
   that owner. Keep their context-specific unavailable labels.
5. Rename the repository-backed destination to **リポジトリ実行記録** in the
   navigation, page metadata, headings/copy, browser expectations, and the
   Issue 122 history guide. Keep the explicit source path
   `workspace/management/runs`, distinct from execution-root `.anvil/runs`.

## Verification strategy

- Extend the focused session-index smoke for both `/` and
  `/proxy/commandagent/`: delayed runtime responses prove max concurrency one;
  hidden time proves polling stops; visible restoration proves immediate
  runtime/resource refresh with stale data retained on failure; and runtime
  badge plus Terminal links prove session navigation/highlighting.
- Extend the Rust GUI source guard for the sole poller, visibility/focus refresh,
  shared formatter, link/highlight, source label, navigation, and page-title
  contracts.
- Run JavaScript syntax checks, GUI typecheck/lint/build, the focused two-base-
  path smoke, GUI/Rust focused guards, then repository formatting, Clippy, the
  default suite, and the GUI-feature suite because shared GUI contracts change.

## Non-goals

- No API/event schema, Rust runtime behavior, filesystem layout, `.anvil/`
  namespace, historical evidence, authentication, or write boundary changes.
- No independent page poller, forced lease action, or weakening of existing
  acceptance and read-only guards.
