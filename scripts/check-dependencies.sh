#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
command -v jq >/dev/null || { echo "check-dependencies: jq not found" >&2; exit 2; }
metadata=$(cargo metadata --quiet --format-version 1 --no-deps --offline 2>/dev/null || cargo metadata --quiet --format-version 1 --no-deps) || exit 2
allowed=$(grep -vE '^\s*(#|$)' scripts/allowed-crates.txt | awk '{print $1 " " $2}')

violations=$(printf '%s' "$metadata" | jq -r '.packages[] | .dependencies[] | "\(.kind // "normal") \(.name)"' | sort -u | while read -r kind name; do
	printf '%s\n' "$allowed" | grep -qxF "$kind $name" || echo "- $kind dependency \`$name\` is not in scripts/allowed-crates.txt"
done)
if [ -n "$violations" ]; then
	echo "dependencies not allowed (adding one goes through human review):" >&2
	echo "$violations" >&2
	exit 1
fi
