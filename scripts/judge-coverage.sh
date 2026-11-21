#!/bin/bash
set -u

usage='usage: judge-coverage.sh <base> [--update] <lcov>'
[ $# -ge 2 ] || { echo "$usage" >&2; exit 2; }
base=$1
shift
mode=''
if [ "$1" = "--update" ]; then
	mode=--update
	shift
fi
[ $# -eq 1 ] || { echo "$usage" >&2; exit 2; }
[ -f "$1" ] || { echo "judge-coverage: no lcov at $1; run the tests first" >&2; exit 2; }
root=$(git rev-parse --show-toplevel) || exit 2
baseline=scripts/coverage-baseline.txt

head=$(awk '/^DA:/ { split(substr($0, 4), f, ","); total++; if (f[2] > 0) covered++ } END { print covered + 0, total + 0 }' "$1")
previous=$(git -C "$root" show "$base:$baseline" 2>/dev/null || echo "0 0")

set -- $head $previous
covered=$1 total=$2 base_covered=$3 base_total=$4

set -- $(awk -v c="$covered" -v t="$total" -v bc="$base_covered" -v bt="$base_total" 'BEGIN {
	p = (bc + 1) / (bt + 2)
	q = t > 0 ? c / t : 1
	bound = t > 0 ? p - sqrt(p * (1 - p) / t) : 0
	printf "%.1f %.1f %d\n", q * 100, bound * 100, q < bound
}')
share=$1 bound=$2 dropped=$3

report="share of pure code: $covered/$total lines ($share%), base $base_covered/$base_total, one standard deviation below the base at $bound%"
echo "$report"
[ -n "${GITHUB_STEP_SUMMARY:-}" ] && echo "$report" >> "$GITHUB_STEP_SUMMARY"

if [ "$mode" = "--update" ]; then
	echo "$covered $total" > "$root/$baseline"
fi

if [ "$dropped" -eq 1 ]; then
	echo "the share of pure code dropped more than one standard deviation, which charges the budget (.shampoo/rules/coverage.toml)" >&2
	exit 3
fi
