# Issue #441 review resolution summary

The reviewed head normalized a quoted path before seeing parent-directory
components after a space. The actual `display_text` regression reproduced
`cat './some dir/../../secret.txt'` for the reported absolute input.

The repair display leaf now scans the complete suffix word, tracking its quote
context, quoted whitespace, escapes, and concatenated quote segments. It checks
decoded components for `..` and checks that the suffix starts at a directory
boundary before replacing the root. Successful output preserves the original
spelling. Incomplete or dynamic quoting/variable syntax remains unnormalized.

The new `recovery-suffixes.json` fixture drives 33 cases through `display_text`,
including traversal negatives and safe-space controls. The corpus expectation
references it. Runtime confinement, execution authority, event schemas, and
historical evidence/reports are unchanged. See the separate review-resolution
verification record for final checks against this correction.
