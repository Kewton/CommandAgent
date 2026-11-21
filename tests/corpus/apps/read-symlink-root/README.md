# read-symlink-root

Synthetic corpus for Issue #966: the heading line of a `Read` result is
workspace-relative whether the workspace root is spelled canonically or
through a symlink. It contains no task text, model output, or run records.

## Origin

`fixtures/read-headings.jsonl` was written by hand for this case. Each row is
one `Read` route of the public tool entry (`ToolRegistry::execute("Read", …)`):
a whole file, a line range, a file over the 6000-byte summary threshold, a
directory, the workspace root itself, and an in-workspace symlink. `heading`
is the expected first line of the result. Paths contain spaces and non-ASCII
characters on purpose.

`tests/read_symlink_root.rs` builds the workspace in a temporary directory and
reads every row from three roots: the canonical path, a path below a
symlinked parent directory, and a symlink that points at the workspace itself.
It requires the same heading and body from all three, and that no result
contains the temporary directory in either spelling. The headings for the
canonical root are what the code produced before Issue #966, so the existing
canonical-root output is unchanged.

The same test keeps the refusals unchanged under a symlinked root: an
outward symlink, a `..` path, a credential file, and an unreadable directory
are still refused, with the same error kind as under the canonical root.

## What it does not cover

Windows symlinks; roots that cannot be canonicalized keep the old absolute
heading (covered by unit tests in `src/tools/read/display.rs`).
