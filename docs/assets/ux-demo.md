# Reproducing the UX demo

The committed [`ux-demo.svg`](ux-demo.svg) is a compact animated excerpt based
on the real offline `commandagent --ux-demo` output. The neighboring
[`ux-demo.tape`](ux-demo.tape) records the complete terminal walkthrough as a
GIF with [VHS](https://github.com/charmbracelet/vhs).

From the repository root:

```bash
cargo install --path .
vhs docs/assets/ux-demo.tape
```

The tape writes `docs/assets/ux-demo.gif`. It requires `commandagent`, VHS, and
VHS's `ffmpeg`/`ttyd` runtime dependencies on `PATH`. The demo contacts no model
provider and normally takes about 20 seconds.

For quick terminal-only iteration without recording the real pacing:

```bash
COMMANDAGENT_UX_DEMO_FAST=1 commandagent --ux-demo
```

When the presentation output changes, update the SVG excerpt and regenerate the
full GIF from the tape in the same change.
