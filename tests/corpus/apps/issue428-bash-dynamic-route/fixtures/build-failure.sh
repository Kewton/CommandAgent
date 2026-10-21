#!/bin/sh
# Deterministic failed-build result for testing the real Recovery promotion gate.
printf '%s\n' 'Synthetic build failure: expense route type check failed' >&2
exit 1
