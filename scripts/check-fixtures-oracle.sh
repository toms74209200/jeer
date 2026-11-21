#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
work=$(mktemp -d) || exit 2
trap 'rm -rf "$work"' EXIT

failures=0
for profile_dir in tests/fixtures/*/; do
	profile=$(basename "$profile_dir")
	for dir in "$profile_dir"*/; do
		dir=${dir%/}
		[ -e "$dir/input.java" ] || continue
		mkdir -p "$work/in/$profile"
		cp "$dir/input.java" "$work/in/$profile/$(basename "$dir").java"
	done
	[ -d "$work/in/$profile" ] || continue
	scripts/oracle.sh "$profile" "$work/in/$profile" "$work/out/$profile" || exit 2
	for dir in "$profile_dir"*/; do
		dir=${dir%/}
		[ -e "$dir/input.java" ] || continue
		name=$(basename "$dir")
		if [ -e "$work/out/$profile/$name.java.error" ]; then
			if [ ! -e "$dir/expected.error" ] || [ -e "$dir/expected.java" ]; then
				echo "FAIL $dir: the oracle rejects the input; expected.error is the only expectation"
				failures=$((failures + 1))
			fi
		elif [ -e "$dir/expected.error" ] || ! cmp -s "$work/out/$profile/$name.java" "$dir/expected.java"; then
			echo "FAIL $dir: expected.java is not the oracle's output (regenerate it with scripts/generate-fixture.sh)"
			failures=$((failures + 1))
		fi
	done
done
[ $failures -eq 0 ]
