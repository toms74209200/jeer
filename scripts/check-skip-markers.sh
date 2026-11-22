#!/bin/bash
set -u
set -o pipefail

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=$(git merge-base HEAD origin/master 2>/dev/null || git merge-base HEAD master) || exit 2
committed=$(git diff --name-only --diff-filter=A "$base" -- 'tests/fixtures/*/*/skip') || exit 2
untracked=$(git ls-files --others --exclude-standard -- 'tests/fixtures/*/*/skip') || exit 2
added=$(printf '%s\n%s\n' "$committed" "$untracked" | sed '/^$/d' | sort -u) || exit 2
[ -n "$added" ] || { echo "skip-markers: no skip marker added"; exit 0; }
cargo build --quiet --release --bin jeer || exit 2
failures=0
for marker in $added; do
	dir=$(dirname "$marker")
	[ -e "$dir/input.java" ] || { echo "skip-markers: $dir is not a fixture" >&2; exit 2; }
	scripts/test-fixtures.sh "$dir" >/dev/null
	status=$?
	case $status in
	0)
		echo "FAIL $dir: jeer already passes it as scripts/test-fixtures.sh judges; delete its skip marker"
		failures=$((failures + 1))
		;;
	1) ;;
	*) exit 2 ;;
	esac
done
[ $failures -eq 0 ] && { echo "skip-markers: every added skip marker is on a fixture jeer does not pass yet"; exit 0; }
exit 1
