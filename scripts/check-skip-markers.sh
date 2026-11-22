#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=$(git merge-base HEAD origin/master 2>/dev/null || git merge-base HEAD master) || exit 2
added=$( { git diff --name-only --diff-filter=A "$base" -- 'tests/fixtures/*/*/skip' || exit 2; git ls-files --others --exclude-standard -- 'tests/fixtures/*/*/skip'; } | sort -u) || exit 2
[ -n "$added" ] || { echo "skip-markers: no skip marker added"; exit 0; }
cargo build --quiet --release --bin jeer || exit 1
out=$(mktemp -d) || exit 2
trap 'rm -rf "$out"' EXIT
failures=0
for marker in $added; do
	dir=$(dirname "$marker")
	profile=$(basename "$(dirname "$dir")")
	timeout 10 target/release/jeer --stdin --profile "$profile" <"$dir/input.java" >"$out/stdout" 2>/dev/null
	status=$?
	if [ -e "$dir/expected.error" ]; then
		if [ $status -eq 2 ] && [ ! -s "$out/stdout" ]; then
			echo "FAIL $dir: jeer already rejects it as the oracle does; delete its skip marker"
			failures=$((failures + 1))
		fi
	elif [ $status -le 1 ] && cmp -s "$out/stdout" "$dir/expected.java"; then
		timeout 10 target/release/jeer --stdin --profile "$profile" <"$dir/expected.java" >"$out/again" 2>/dev/null
		again=$?
		if [ $again -eq 0 ] && cmp -s "$out/again" "$dir/expected.java"; then
			echo "FAIL $dir: jeer already gives the oracle's output; delete its skip marker"
			failures=$((failures + 1))
		fi
	fi
done
[ $failures -eq 0 ] && { echo "skip-markers: every added skip marker is on a fixture jeer does not pass yet"; exit 0; }
exit 1
