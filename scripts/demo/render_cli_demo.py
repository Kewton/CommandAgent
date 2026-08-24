#!/usr/bin/env python3
"""Render a cast recorded by record_cli_demo.py into an animated GIF.

The bytes are replayed through a VT100 emulator (pyte); every frame is drawn
with Pillow and assembled with ffmpeg. Idle gaps longer than --max-gap seconds
are shortened so the GIF stays watchable, but no frame content is edited.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path

import pyte
from PIL import Image, ImageDraw, ImageFont

PALETTE = {
    "default": None,
    "black": "#1f2430",
    "red": "#f28779",
    "green": "#a6e3a1",
    "brown": "#f9e2af",
    "yellow": "#f9e2af",
    "blue": "#89b4fa",
    "magenta": "#f5c2e7",
    "cyan": "#94e2d5",
    "white": "#e6e9ef",
    "brightblack": "#6c7086",
    "brightred": "#f38ba8",
    "brightgreen": "#b5f0b0",
    "brightbrown": "#fae8b8",
    "brightyellow": "#fae8b8",
    "brightblue": "#a3c5ff",
    "brightmagenta": "#f8d3ee",
    "brightcyan": "#aeeee3",
    "brightwhite": "#ffffff",
}
BACKGROUND = "#0f1117"
FOREGROUND = "#d9dee8"
CHROME = "#181b23"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cast", required=True)
    parser.add_argument("--out", required=True, help="output .gif path")
    parser.add_argument("--poster", help="optional PNG of the final frame")
    parser.add_argument(
        "--snapshot",
        action="append",
        default=[],
        metavar="TEXT=PATH",
        help="also save a PNG of the screen shortly after TEXT first appears on it",
    )
    parser.add_argument("--snapshot-delay", type=float, default=1.0)
    parser.add_argument(
        "--fast-after",
        metavar="TEXT",
        help="once TEXT is on screen, play the rest at --speed (time-lapse of the run)",
    )
    parser.add_argument("--speed", type=float, default=1.0)
    parser.add_argument("--font-size", type=int, default=15)
    parser.add_argument("--max-gap", type=float, default=1.5)
    parser.add_argument("--min-frame", type=float, default=0.08)
    parser.add_argument("--fps", type=int, default=10)
    parser.add_argument("--title", default="commandagent — real terminal recording")
    parser.add_argument("--tail-hold", type=float, default=4.0)
    parser.add_argument("--mono-font", default="/System/Library/Fonts/Menlo.ttc")
    parser.add_argument(
        "--cjk-font", default="/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc"
    )
    return parser.parse_args()


def color(value: str, fallback: str | None) -> str | None:
    if value in PALETTE:
        return PALETTE[value] if PALETTE[value] is not None else fallback
    if len(value) == 6:
        return f"#{value}"
    return fallback


def is_wide(char: str) -> bool:
    return unicodedata.east_asian_width(char) in ("W", "F")


def needs_cjk_font(char: str) -> bool:
    code = ord(char)
    return code > 0x2E7F and not (0xFF61 <= code <= 0xFF9F)


class Renderer:
    def __init__(self, cols: int, rows: int, args: argparse.Namespace) -> None:
        self.cols = cols
        self.rows = rows
        self.font = ImageFont.truetype(args.mono_font, args.font_size, index=0)
        self.bold = ImageFont.truetype(args.mono_font, args.font_size, index=1)
        self.cjk = ImageFont.truetype(args.cjk_font, args.font_size)
        ascent, descent = self.font.getmetrics()
        self.cell_w = round(self.font.getlength("M"))
        self.cell_h = ascent + descent + 2
        self.pad = 18
        self.chrome_h = 34
        self.width = self.cols * self.cell_w + self.pad * 2
        self.height = self.rows * self.cell_h + self.pad * 2 + self.chrome_h
        self.title = args.title

    def draw(self, screen: pyte.Screen) -> Image.Image:
        image = Image.new("RGB", (self.width, self.height), BACKGROUND)
        canvas = ImageDraw.Draw(image)
        canvas.rectangle([0, 0, self.width, self.chrome_h], fill=CHROME)
        for index, dot in enumerate(("#ff5f57", "#febc2e", "#28c840")):
            x = 14 + index * 20
            canvas.ellipse([x, 11, x + 12, 23], fill=dot)
        canvas.text((82, 9), self.title, font=self.font, fill="#9aa3b5")
        top = self.chrome_h + self.pad
        for row in range(self.rows):
            line = screen.buffer[row]
            col = 0
            while col < self.cols:
                cell = line[col]
                char = cell.data or " "
                if cell.data == "":
                    col += 1
                    continue
                wide = is_wide(char)
                span = 2 if wide else 1
                x0 = self.pad + col * self.cell_w
                y0 = top + row * self.cell_h
                fg = color(cell.fg, FOREGROUND)
                bg = color(cell.bg, None)
                if cell.reverse:
                    fg, bg = (bg or BACKGROUND), (fg or FOREGROUND)
                if bg:
                    canvas.rectangle(
                        [x0, y0, x0 + self.cell_w * span, y0 + self.cell_h], fill=bg
                    )
                if char.strip():
                    font = (
                        self.cjk
                        if needs_cjk_font(char)
                        else (self.bold if cell.bold else self.font)
                    )
                    canvas.text((x0, y0 + 1), char, font=font, fill=fg or FOREGROUND)
                    if cell.underscore:
                        canvas.line(
                            [
                                x0,
                                y0 + self.cell_h - 2,
                                x0 + self.cell_w * span,
                                y0 + self.cell_h - 2,
                            ],
                            fill=fg or FOREGROUND,
                        )
                col += span
        if not screen.cursor.hidden:
            cx = self.pad + screen.cursor.x * self.cell_w
            cy = top + screen.cursor.y * self.cell_h
            canvas.rectangle(
                [cx, cy + 2, cx + self.cell_w, cy + self.cell_h - 1],
                outline="#c6d0f5",
                fill="#3b4261",
            )
            cell = screen.buffer[screen.cursor.y][screen.cursor.x]
            if cell.data and cell.data.strip():
                canvas.text((cx, cy + 1), cell.data, font=self.font, fill="#ffffff")
        return image


def main() -> int:
    args = parse_args()
    cast = json.loads(Path(args.cast).read_text(encoding="utf-8"))
    cols, rows = cast["cols"], cast["rows"]
    screen = pyte.Screen(cols, rows)
    stream = pyte.ByteStream(screen)
    renderer = Renderer(cols, rows, args)

    snapshots = [
        (text, path, None)
        for text, path in (item.split("=", 1) for item in args.snapshot)
    ]
    frames: list[tuple[Image.Image, float]] = []
    last_capture_time = 0.0
    pending: Image.Image | None = None
    clock = 0.0
    previous_t = 0.0
    fast = False
    for t, payload in cast["events"]:
        gap = min(max(t - previous_t, 0.0), args.max_gap)
        if fast:
            gap /= args.speed
        previous_t = t
        clock += gap
        stream.feed(base64.b64decode(payload))
        if (
            args.fast_after
            and not fast
            and args.fast_after in "\n".join(screen.display)
        ):
            fast = True
            print(f"time-lapse x{args.speed} from {clock:.1f}s")
        if snapshots:
            visible = "\n".join(screen.display)
            for index, (text, path, seen_at) in enumerate(snapshots):
                if seen_at is None and text in visible:
                    snapshots[index] = (text, path, clock)
                elif seen_at is not None and clock - seen_at >= args.snapshot_delay:
                    renderer.draw(screen).save(path, optimize=True)
                    print(f"snapshot {path} at {clock:.1f}s")
                    snapshots[index] = (text, path, float("inf"))
        if pending is None or clock - last_capture_time >= args.min_frame:
            if pending is not None:
                frames.append((pending, max(clock - last_capture_time, args.min_frame)))
            pending = renderer.draw(screen)
            last_capture_time = clock
    if pending is not None:
        frames.append((pending, args.tail_hold))

    # merge identical consecutive frames
    merged: list[tuple[Image.Image, float]] = []
    for image, duration in frames:
        if merged and merged[-1][0].tobytes() == image.tobytes():
            merged[-1] = (merged[-1][0], merged[-1][1] + duration)
        else:
            merged.append((image, duration))
    merged[-1] = (merged[-1][0], max(merged[-1][1], args.tail_hold))

    ffmpeg = shutil.which("ffmpeg")
    if ffmpeg is None:
        print("ffmpeg is required", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory() as directory:
        manifest = []
        for index, (image, duration) in enumerate(merged):
            path = os.path.join(directory, f"f{index:05d}.png")
            image.save(path, optimize=False)
            manifest.append(f"file '{path}'\nduration {duration:.3f}")
        manifest.append(f"file '{path}'")
        concat = os.path.join(directory, "frames.txt")
        Path(concat).write_text("\n".join(manifest) + "\n", encoding="utf-8")
        filters = (
            f"fps={args.fps},split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];"
            "[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle"
        )
        subprocess.run(
            [
                ffmpeg,
                "-y",
                "-loglevel",
                "error",
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
                concat,
                "-vf",
                filters,
                "-loop",
                "0",
                args.out,
            ],
            check=True,
        )
        if args.poster:
            merged[-1][0].save(args.poster, optimize=True)
    total = sum(duration for _, duration in merged)
    print(
        f"wrote {args.out}: {len(merged)} frames, {total:.1f}s, {os.path.getsize(args.out) / 1024:.0f} KiB"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
