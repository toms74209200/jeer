#!/bin/bash
set -u

[ $# -eq 1 ] || { echo "usage: coverage-summary.sh <lcov>" >&2; exit 2; }
[ -f "$1" ] || { echo "coverage-summary: no lcov at $1; run scripts/test-small.sh" >&2; exit 2; }
root=$(git rev-parse --show-toplevel) || exit 2
threshold=$(sed -n 's/^threshold=//p' "$root/scripts/judge-small-test-coverage.sh")

awk -v root="$root/" -v threshold="$threshold" 'FILENAME == ARGV[1] {
		if ($0 != "" && $0 !~ /^#/) prefixes[++n] = $0
		next
	}
	/^SF:/ {
		file = substr($0, 4)
		sub("^" root, "", file)
		module = file
		sub(/^src\//, "", module)
		sub(/\/.*/, "", module)
		sub(/\.rs$/, "", module)
		side = 0
		for (i = 1; i <= n; i++) if (index(file, prefixes[i]) == 1) side = 1
		kind[module] = side ? "side effects" : "pure"
		next
	}
	/^DA:/ {
		split(substr($0, 4), f, ",")
		total[module]++
		if (f[2] > 0) hit[module]++
	}
	END {
		print "## Coverage"
		print ""
		print "| Module | Kind | Lines | Small-test coverage |"
		print "|---|---|---:|---:|"
		for (m in total) {
			if (kind[m] == "pure") {
				printf "| %s | pure | %d | %.1f%% (%d/%d) |\n", m, total[m], 100 * hit[m] / total[m], hit[m], total[m] | "sort"
			} else {
				printf "| %s | side effects | %d | |\n", m, total[m] | "sort"
			}
			all += total[m]
			covered_all += hit[m]
			if (kind[m] == "pure") { pure += total[m]; covered += hit[m] }
		}
		close("sort")
		printf "\nShare of pure code: %.1f%% (%d/%d lines)\n\n", all ? 100 * covered_all / all : 100, covered_all, all
		printf "Small-test coverage of pure code: %.1f%% (%d/%d lines), at least %d%% required\n", pure ? 100 * covered / pure : 100, covered, pure, threshold
	}' "$root/scripts/side-effect-modules.txt" "$1"
