#!/bin/bash
set -u

[ $# -ge 1 ] || { echo "usage: brief.sh <command> [args...]" >&2; exit 2; }
log=$(mktemp) || exit 2
trap 'rm -f "$log"' EXIT
"$@" >"$log" 2>&1
status=$?
limit=30
lines=$(wc -l <"$log")
head -n "$limit" "$log"
[ "$lines" -gt "$limit" ] && echo "... $((lines - limit)) more lines; run $1 for all of them"
exit $status
