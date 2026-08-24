# Issues #186, #194, and #199 Design

## Scope

The approved row combines the extension-wizard synchronization fixes from
#186 with the extension-catalog accessibility fixes from #194 and #199. The
change is limited to the existing GUI data hooks, `PackWizard`, the assets
page, its styles, and focused browser-smoke assertions. It does not change API
schemas, extension lifecycle rules, persisted pack bytes, or runtime state.

There are no required predecessors. The two commits by which this worktree is
behind `origin/develop` only change CommandMate orchestration behavior and do
not overlap this GUI surface.

## Design

- Load wizard profiles from the same `trial-options` resource used by Trial.
  Keep the profile selector disabled while options are unavailable, preserve
  the selected value when it remains valid, and fall back to the first returned
  profile if the server no longer advertises the current value. Intent remains
  the server's existing closed `create` / `fix` / `investigate` vocabulary.
- Read `trial_token_auth_enabled` from `Shell`'s runtime-status context. Match
  Trial by rendering the password field only while authentication is enabled;
  when the server explicitly reports authentication off, clear the in-memory
  and tab-scoped stored token and show a non-input explanatory note.
- Extend `useResource` with a stable explicit refresh operation. Pass the packs
  refresh operation into `PackWizard`, and invoke it after successful pin and
  retirement requests so the catalog reflects lifecycle changes immediately.
- Implement automatic-activation tabs: `tablist`, `tab`, `aria-selected`, one
  tabbable active tab, linked `tabpanel` regions, and Left/Right/Home/End key
  handling that both focuses and selects the destination tab.
- Give document disclosure buttons `aria-expanded` and `aria-controls`, and
  hide their decorative plus/minus glyph from assistive technology.
- Replace one alert live region per warning card with static `note` semantics
  and a single status message containing the warning count.
- Render each assist/eval presence value with distinct check/minus glyphs and
  explicit Japanese `あり` / `なし` text, retaining color only as redundant
  styling.

## Verification

Focused browser smoke coverage will compare wizard profile options with the
live Trial options, exercise authentication-off token visibility, require a
new catalog row immediately after pin, drive every tab using keyboard keys,
check disclosure state, assert one warning status with no warning alerts, and
verify explicit assist/eval presence text. The smoke will also run axe on the
assets page. Type checking, internal-path linting, and a production GUI build
will cover shared TypeScript and export behavior.
