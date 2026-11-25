#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
command -v cargo >/dev/null || { echo "compiles: cargo not found" >&2; exit 2; }
log=$(mktemp) || exit 2
trap 'rm -f "$log"' EXIT
cargo check --quiet --all-targets --message-format=short >"$log" 2>&1
status=$?
[ $status -eq 0 ] && exit 0
grep -E '^[^ :]+:[0-9]+:[0-9]+: error' "$log" || { cat "$log" >&2; exit 2; }
exit 1
