#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
result=0
for check in "scripts/lint.sh" "scripts/check-dependencies.sh" "scripts/check-sg.sh comments" "scripts/check-sg.sh functions" "scripts/check-sg.sh test-names" "scripts/check-sg.sh test-helpers" "scripts/check-sg.sh exceptions" "scripts/check-branching-results.sh" "scripts/test-small.sh"; do
	$check
	status=$?
	[ $status -eq 0 ] && continue
	echo "check-small: $check exited $status" >&2
	[ $status -gt $result ] && result=$status
done
exit $result
