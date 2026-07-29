"""Machine-readable IDs emitted by the Python management layer.

Publisher inventory (2026-07-29):
- bench.py writes the environment-interruption terminal status into uat-meta.

band_aggregate.py and classify_runs.py consume or normalize persisted IDs; they
do not publish new run-terminal protocol IDs. Acceptance-sheet reason maps and
scan diagnostics are presentation/audit vocabulary, not classification input.
"""

PYTHON_PRODUCED_IDS = ("interrupted(environment)",)

INTERRUPTED_ENVIRONMENT = PYTHON_PRODUCED_IDS[0]
