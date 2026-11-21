#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
cargo build --quiet --release --bin jeer || exit 1
bin="$root/target/release/jeer"
out=$(mktemp -d) || exit 2
trap 'rm -rf "$out"' EXIT

failures=0
passed=0
skipped=0
for dir in tests/fixtures/*/*/; do
	dir=${dir%/}
	[ -e "$dir/input.java" ] || continue
	if [ -e "$dir/skip" ]; then
		skipped=$((skipped + 1))
		continue
	fi
	profile=$(basename "$(dirname "$dir")")
	timeout 10 "$bin" --stdin --profile "$profile" <"$dir/input.java" >"$out/stdout" 2>"$out/stderr"
	status=$?
	if [ -e "$dir/expected.error" ]; then
		if [ $status -ne 2 ] || [ -s "$out/stdout" ]; then
			echo "FAIL $dir: expected rejection (exit 2, no output), got exit $status"
			failures=$((failures + 1))
			continue
		fi
	else
		if [ $status -gt 1 ] || ! cmp -s "$out/stdout" "$dir/expected.java"; then
			echo "FAIL $dir: exit $status"
			diff -u "$dir/expected.java" "$out/stdout" | head -40
			failures=$((failures + 1))
			continue
		fi
		timeout 10 "$bin" --stdin --profile "$profile" <"$dir/expected.java" >"$out/again" 2>/dev/null
		again=$?
		if [ $again -ne 0 ] || ! cmp -s "$out/again" "$dir/expected.java"; then
			echo "FAIL $dir: not idempotent (exit $again on its expected output)"
			failures=$((failures + 1))
			continue
		fi
	fi
	passed=$((passed + 1))
done
echo "fixtures: $passed passed, $failures failed, $skipped skipped"
[ $failures -eq 0 ]
