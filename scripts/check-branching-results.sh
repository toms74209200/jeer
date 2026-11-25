#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
if [ -n "${BRIEF:-}" ]; then
	scripts/check-compiles.sh >/dev/null
	compiled=$?
	[ $compiled -eq 1 ] && { echo "branching-results: fails, the code does not compile (scripts/check-compiles.sh reports why)"; exit 1; }
	[ $compiled -eq 0 ] || exit $compiled
fi
log=$(mktemp) || exit 2
trap 'rm -f "$log"' EXIT
status=0
for manifest in Cargo.toml tools/branching-results/Cargo.toml; do
	DYLINT_RUSTFLAGS="-D branching_results" scripts/dylint.sh --path tools/branching-results -- --manifest-path "$manifest" --all-targets --quiet >>"$log" 2>&1 || status=$?
done
[ $status -eq 0 ] && exit 0
cat "$log" >&2
grep -q 'branching_results' "$log" && exit 1
exit 2
