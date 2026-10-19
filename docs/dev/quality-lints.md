# Quality lints

Status: introduced by #707 (parent #536). #718 appends the crate-wide
`allow_attributes_without_reason` and `undocumented_unsafe_blocks` lints below.

## Enforced boundary

The panic-class clippy lints `unwrap_used`, `expect_used` and
`indexing_slicing` are `deny` for the non-test code of exactly two files:

- `src/tools/path_guard.rs` (the attribute sits at its top:
  `#![cfg_attr(not(test), deny(...))]`)
- `src/tools/path_guard/bash_pattern.rs` (a child module, so it inherits the
  attribute)

This is the only enforced boundary. The lints are not enabled crate-wide, and
no `allow` or `expect` attribute is used to silence a diagnostic there. A
rejected path, empty input, out-of-range index or brace-limit overflow must
stay a rejection; do not replace it with a default, an empty string or success.

Every other panic candidate in `src` (about 41 `unwrap`, 135 `expect` and 934
index or slice sites on `develop` `89d628bd`, counted statically with
tree-sitter and including safe index types) is classification material only,
not an enforced rule.

## Candidate classification (real clippy diagnostics)

Method: the three lints were set to `deny`, then `cargo clippy --all-targets`
was read. `unwrap`/`expect` candidates: 0 in both files. Slicing of a `str` by
byte range is not reported by `indexing_slicing`.

### `src/tools/path_guard.rs` (11 candidates)

| Site (before) | Form | Clippy diagnostic | Resolution |
| --- | --- | --- | --- |
| `split_absolute_glob_base` `raw[..meta]` | `str` slice | none (safe type) | unchanged |
| same, `&raw[split + 1..]` (split == 0 branch) | `str` slice | none | unchanged |
| same, `&raw[..split]` | `str` slice | none | unchanged |
| same, `&raw[split + 1..]` (else branch) | `str` slice | none | unchanged |
| `root_anchor_salvage` `root_components[len - anchor_len..]` | `Vec` slice | yes | `.get(..)?`; `None` (no salvage) as before |
| `root_anchor_salvage` `raw_components[index + anchor_len..]` | `Vec` slice | yes | `.get(..)?`; `None` as before |
| `looks_like_windows_absolute` `bytes[0]` | byte index | yes | slice pattern `[drive, b':', b'\\' \| b'/', ..]` |
| same, `bytes[1]` | byte index | yes | same pattern |
| same, `bytes[2]` (first) | byte index | yes | same pattern |
| same, `bytes[2]` (second) | byte index | yes | same pattern |
| `looks_like_near_root_digit_variance` `root_components[1..]` | `Vec` slice | yes | `.get(1..).is_some_and(..)`; `false` as before |

### `src/tools/path_guard/bash_pattern.rs` (4 candidates plus 2 diagnosed sites)

| Site (before) | Form | Clippy diagnostic | Resolution |
| --- | --- | --- | --- |
| `&input[..span.start]` | `str` slice | none | unchanged |
| `&input[span.end..]` | `str` slice | none | unchanged |
| `suffix = &input[span.end..]` | `str` slice | none | unchanged |
| `parse_brace_shape(&input[start + 1..close])` | `str` slice | none | unchanged |
| `find_brace` `bytes[index]` | byte index | yes | `while let Some(&byte) = bytes.get(index)` |
| `find_brace` `bytes[cursor]` | byte index | yes | `while let Some(&byte) = bytes.get(cursor)` |

The two `find_brace` byte indexes are not in the issue's static list of four;
clippy reported them, so they were fixed. The `str` slices are guarded by
`find`/`rfind` offsets on char boundaries and are left as they are.

## Crate-wide lints (#718)

`Cargo.toml` enables two lints for every target of the package, under
`[lints.clippy]`, at `deny`:

- `allow_attributes_without_reason`
- `undocumented_unsafe_blocks`

They run through the existing clippy calls (`scripts/ci.sh` and the GUI job),
which already pass `-D warnings`; no extra clippy invocation exists for them.
`deny` matches the #707 boundary lints. The panic-class lints above stay
limited to `path_guard`; these two do not widen them.

Writing a reason: every `#[allow(..)]` carries `reason = "..."` that says why
the lint fires and is accepted, not what the lint is. rustfmt keeps an
attribute on one line only when its arguments fit in 70 characters, so keep
the reason short or give each lint its own attribute:

```rust
#[allow(clippy::too_many_arguments, reason = "each run input is explicit")]
```

Writing a safety comment: every `unsafe { .. }` block is preceded by a
`// SAFETY:` comment that states the invariant the caller upholds, for
example which pointer is valid or which syscall precondition holds. Do not
silence either lint with a blanket `#![allow]` or a crate-level exclusion.

Enabled with 0 diagnostics on macOS, on `x86_64-unknown-linux-gnu`
(`cargo-zigbuild`), with `--features gui`, and for `gui_server`. The only
pre-existing site was `build.rs` (`dead_code` on the shared `env_compat`).

## Verification

`tests/issue536_path_guard_lints.rs` holds paired accept/reject cases (empty,
Windows-drive truncation, multi-byte names, salvage anchors, glob split,
unterminated braces, the 256-expansion brace limit). It passes with the same
results on the pre-change `develop` sources and on the fixed sources.
