#!/bin/bash
set -u

[ $# -eq 1 ] || { echo "usage: generate-fixture.sh tests/fixtures/<profile>/<name>" >&2; exit 2; }
dir=${1%/}
[ -e "$dir/input.java" ] || { echo "generate-fixture: no $dir/input.java" >&2; exit 2; }
root=$(git rev-parse --show-toplevel) || exit 2
profile=$(basename "$(dirname "$dir")")
name=$(basename "$dir")
work=$(mktemp -d) || exit 2
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/in"
cp "$dir/input.java" "$work/in/$name.java"
"$root/scripts/oracle.sh" "$profile" "$work/in" "$work/out" || exit 2
rm -f "$dir/expected.java" "$dir/expected.error"
if [ -e "$work/out/$name.java.error" ]; then
	cp "$work/out/$name.java.error" "$dir/expected.error"
else
	cp "$work/out/$name.java" "$dir/expected.java"
fi
