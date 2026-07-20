# UX demo assets and provider-backed recording

The committed [`ux-demo.svg`](ux-demo.svg) is a compact animated excerpt based
on the real offline `commandagent --ux-demo` output. The neighboring
[`ux-demo.tape`](ux-demo.tape) records the complete terminal walkthrough as a
GIF with [VHS](https://github.com/charmbracelet/vhs). Both are scripted
documentation assets: the SVG is hand-authored, and `--ux-demo` never contacts
a provider.

[`repl-ultra-plan-run.rec`](repl-ultra-plan-run.rec) is different: it is a
timestamped `script(1)` recording captured from the current locally built
`commandagent` binary running an actual REPL `/ultra-plan-run` against Ollama.
It shows the accepted long CJK instruction, Goal/profile/port/Run ID, phase and
step progress, a long provider wait in the live fixed footer, interrupt/recovery
feedback, and `/status`. The capture uses a 24x120 PTY with spinner, Markdown,
color, and footer enabled; the run ends through the documented recovery path
after a user interrupt.
Replay it on BSD/macOS with:

```bash
script -p docs/assets/repl-ultra-plan-run.rec
```

From the repository root:

```bash
cargo install --path .
vhs docs/assets/ux-demo.tape
```

The tape writes `docs/assets/ux-demo.gif`. It requires `commandagent`, VHS, and
VHS's `ffmpeg`/`ttyd` runtime dependencies on `PATH`. The demo contacts no model
provider and normally takes about 20 seconds. This is separate from the real
provider-backed recording above.

For quick terminal-only iteration without recording the real pacing:

```bash
COMMANDAGENT_UX_DEMO_FAST=1 commandagent --ux-demo
```

When the presentation output changes, update the SVG excerpt and regenerate the
full GIF from the tape in the same change.
