#!/bin/bash
set -u

threshold=95
[ $# -eq 1 ] || { echo "usage: judge-small-test-coverage.sh <lcov>" >&2; exit 2; }
root=$(git rev-parse --show-toplevel) || exit 2
[ -r "$1" ] || { echo "judge-small-test-coverage: cannot read $1" >&2; exit 2; }

awk -v root="$root/" -v threshold="$threshold" -v excluded="$root/scripts/side-effect-modules.txt" '
BEGIN {
	while ((getline line < excluded) > 0) {
		if (line != "" && line !~ /^#/) prefixes[++n] = line
	}
}
/^SF:/ {
	file = substr($0, 4)
	sub("^" root, "", file)
	skip = 0
	for (i = 1; i <= n; i++) if (index(file, prefixes[i]) == 1) skip = 1
	next
}
/^DA:/ {
	if (skip) next
	split(substr($0, 4), f, ",")
	total[file]++
	if (f[2] > 0) hit[file]++
	next
}
END {
	all = 0; covered = 0
	for (file in total) {
		all += total[file]; covered += hit[file]
		if (hit[file] < total[file]) printf "%s: %d/%d lines\n", file, hit[file], total[file]
	}
	if (all == 0) { print "small-test coverage: no pure code yet"; exit 0 }
	pct = 100 * covered / all
	printf "small-test coverage of pure code: %.2f%% (%d/%d lines, threshold %d%%)\n", pct, covered, all, threshold
	exit (pct + 1e-9 < threshold) ? 1 : 0
}' "$1"
