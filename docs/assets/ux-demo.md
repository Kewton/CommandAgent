# Demo assets: real recordings, the scripted walkthrough, and the raw capture

This directory holds three kinds of demo material. Only the first kind is
embedded in the README.

## 1. Real recordings embedded in the README (`demo/`)

| Asset | What it shows | How it was captured |
| --- | --- | --- |
| [`demo/cli-demo.gif`](demo/cli-demo.gif) | The product REPL: banner, `/status`, a plain request, the Gate 1 card, `/confirm <hash>`, the phased `ultra-plan` run in the fixed footer, and the terminal result. | `scripts/demo/record_cli_demo.py` drives the real binary in a 100×28 PTY and stores every byte with a timestamp in [`demo/cli-demo.cast.json`](demo/cli-demo.cast.json); `scripts/demo/render_cli_demo.py` replays that cast through a VT100 emulator and writes the GIF. |
| [`demo/gui-demo.gif`](demo/gui-demo.gif) | The management GUI: the first-run card, the sample goal, the Gate 1 card, the running Gate 2 view, and the Gate 3/4 result. | `scripts/demo/record_gui_demo.mjs` walks the real GUI in the repository-managed Playwright browser against a running `gui_server`, captures frames at each step, and assembles the GIF. |

Neither recording is staged. The model replies, the plan, the phase outcomes,
and the verdict are whatever the delegated `commandagent` run produced at
capture time; idle gaps are shortened in the GIF but no frame is edited. The
capture metadata (binary, workspace, model, timestamp) is stored inside
`demo/cli-demo.cast.json`, and the GUI run is visible in the GIF itself.

### Regenerate the CLI recording

```bash
python3 -m venv .venv-demo && .venv-demo/bin/pip install pyte pillow
mkdir -p /tmp/commandagent-demo/cli-workspace /tmp/commandagent-demo/state
(cd /tmp/commandagent-demo/cli-workspace && git init -q && git commit -q --allow-empty -m init)
python3 scripts/demo/record_cli_demo.py \
  --bin target/release/commandagent \
  --workdir /tmp/commandagent-demo/cli-workspace \
  --state-dir /tmp/commandagent-demo/state \
  --model "<your-model>" --yes \
  --out docs/assets/demo/cli-demo.cast.json
.venv-demo/bin/python scripts/demo/render_cli_demo.py \
  --cast docs/assets/demo/cli-demo.cast.json \
  --out docs/assets/demo/cli-demo.gif \
  --snapshot "/confirm sha256:=docs/assets/tutorial/cli-gate1.png" --snapshot-delay 0.3 \
  --fast-after Dispatching --speed 6 \
  --title "commandagent — real terminal recording (<date>, <model> via Ollama)"
```

`--state-dir` must point at an empty directory so that no personal REPL
history is suggested on screen. `--yes` is passed because the recording runs in
a throwaway workspace; do not use it elsewhere. `ffmpeg` must be on `PATH`.
The renderer caps idle gaps at 1.5 s and, from the `Dispatching` line on,
plays the run at 6× so a six-minute run fits in about 75 s; `--snapshot`
writes the Gate 1 still used by the tutorial.

### Regenerate the GUI recording

Start `gui_server` as described in [GUI setup](../user/gui-setup.md#serve-at-)
with a throwaway execution root, then:

```bash
node scripts/demo/record_gui_demo.mjs \
  --base=http://127.0.0.1:4173 \
  --model="<your-model>" \
  --out=/tmp/commandagent-demo/gui-demo
cp /tmp/commandagent-demo/gui-demo/gui-demo.gif docs/assets/demo/gui-demo.gif
```

The script also writes the full-page screenshots used by the
[tutorial](../guide/en/tutorial.md) to `<out>/shots/`.

## 2. Scripted walkthrough (`--ux-demo`, `ux-demo.svg`, `ux-demo.tape`)

`commandagent --ux-demo` is a completely offline, scripted walkthrough that
never contacts a provider. [`ux-demo.svg`](ux-demo.svg) is a hand-authored
excerpt of that script and [`ux-demo.tape`](ux-demo.tape) records the complete
walkthrough as a GIF with [VHS](https://github.com/charmbracelet/vhs):

```bash
cargo install --path .
vhs docs/assets/ux-demo.tape
```

For quick terminal-only iteration without the recorded pacing:

```bash
COMMANDAGENT_UX_DEMO_FAST=1 commandagent --ux-demo
```

These are documentation assets for the scripted demo only; the README no
longer presents them as a recording.

## 3. Historical provider-backed raw capture (`repl-ultra-plan-run.rec`)

[`repl-ultra-plan-run.rec`](repl-ultra-plan-run.rec) is a timestamped
`script(1)` recording captured from an earlier locally built `commandagent`
binary running an actual REPL `/ultra-plan-run` against Ollama. It shows the
accepted long CJK instruction, Goal/profile/port/Run ID, phase and step
progress, a long provider wait in the live fixed footer, interrupt/recovery
feedback, and `/status`. The capture uses a 24x120 PTY with spinner, Markdown,
color, and footer enabled; the run ends through the documented recovery path
after a user interrupt. It is kept as provider-backed evidence for the TUI
contract tests. Replay it on BSD/macOS with:

```bash
script -p docs/assets/repl-ultra-plan-run.rec
```
