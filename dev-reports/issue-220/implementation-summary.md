# Issue #220 implementation summary

- Preserved the existing tab-separated pack table when at least one compatible
  admitted or local pack exists.
- Suppressed the heading-only stdout result when no compatible row exists and
  emitted `no compatible packs for <profile> × <intent>` on stderr while
  retaining a successful exit status.
- Added process-level regression coverage for the `nextjs × fix` empty result.
  Existing valid-list, invalid-local-candidate, verification, and pinning tests
  remain green.
