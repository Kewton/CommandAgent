# Implementation Summary: Issue #232

Implemented offline run inspection and opt-in provider tracing as part of the combined #255/#229/#232 contract.

- Extended `--runs` with an optional run ID. Detail output summarizes status, phases, stop reason, event path, trace count, and the stored summary without creating provider clients.
- Added chronological `--events` output, closed `--filter phase|tool|provider` selection, and versioned JSON projections for list, detail, and events views.
- Kept event parsing honest: an invalid run selector or malformed selected JSONL event produces an error instead of silently dropping evidence.
- Added the `run_trace` leaf and wired `--trace` once at the shared provider-call boundary. Each exchange is persisted as a separate versioned JSON file under the active run, using the existing secret/home-path scrubbing before writing.
- Kept trace opt-in and isolated per execution thread. Trace-write failures warn without changing the provider call result, and existing event names and fields remain backward compatible.
- Updated bilingual CLI documentation and added focused run/event/JSON/filter/trace tests plus a corpus fixture for the shared observability contract.
