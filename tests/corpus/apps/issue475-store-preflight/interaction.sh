#!/bin/sh
set -eu
# Product invokes node-program <generated-script> <url> <evidence> <options>.
exec node observer.mjs interaction "$2" "$3"
