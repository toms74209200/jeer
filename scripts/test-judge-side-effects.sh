#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
data="$root/scripts/testdata/side-effects"
work=$(mktemp -d) || exit 2
trap 'rm -rf "$work"' EXIT
failures=0
expect() {
	expected=$1 dir=$2
	CARGO_TARGET_DIR=/repo/target TMPDIR=/tmp "$root/scripts/judge-side-effects.sh" "$dir" >/dev/null 2>"$work/err"
	actual=$?
	[ "$actual" -eq "$expected" ] || { echo "FAIL $dir: exit $actual, expected $expected"; cat "$work/err"; failures=$((failures + 1)); }
}
expect 0 "$data/accepted"
for trace in "$data"/flagged/*.trace; do
	mkdir -p "$work/$(basename "$trace")"
	cp "$trace" "$work/$(basename "$trace")/"
	expect 1 "$work/$(basename "$trace")"
done
expect 2 "$work/missing"
[ $failures -eq 0 ] && echo "judge-side-effects: all cases pass"
[ $failures -eq 0 ]
