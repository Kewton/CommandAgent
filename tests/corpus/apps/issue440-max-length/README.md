# Issue #440 fixtures

`i1-recovery.jsonl` is a sanitized field projection of the read-only campaign
`20260907-1317-standard10`, I1 session `01a07a2f-0d28-7120-a0a2-eae3e2f88b37`,
events 584–643. Response bodies, arguments, local paths, and credentials are
omitted. Original timing, token usage, tool names/status, and source line
numbers are retained. No original evidence is modified.

The default limit consumes two matching replies (584, 607), then stops before
requesting another reply. An internal limit of three consumes 584, 607, 629
and stops before 631. The intervening successful Reads never reset the guard.
Matching duration totals are 262217 ms and 385947 ms, respectively, both below
the unchanged 900000 ms step cap. Tests replay these timings without sleeping.

`long-page.txt` is a synthetic 16 KiB Write payload, tested through native and
normalized text tools. Successful Write/Edit resets count and duration; a
failed mutation, a short reply, or an unknown token count does not.
