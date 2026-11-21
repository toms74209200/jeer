#!/bin/bash
set -u

[ $# -eq 2 ] && [ "$1" = "--rev" ] || { echo "usage: check-fixture-commit.sh --rev <commit>" >&2; exit 2; }
rev=$2
changes=$(git diff-tree --no-commit-id --root -r --name-status -M "$rev") || exit 2

violations=''
touches_src=$(printf '%s\n' "$changes" | awk -F'\t' '$NF ~ /^src\// {print "y"; exit}')
while IFS=$'\t' read -r status path rest; do
	[ -n "$status" ] || continue
	[ -n "$rest" ] && path=$rest
	case "$path" in
	tests/fixtures/*/*/skip)
		dir=${path%/skip}
		case "$status" in
		A)
			printf '%s\n' "$changes" | grep -qP "^A\t\Q$dir/input.java\E$" ||
				violations="$violations
- $path: a skip marker is added only with its new fixture; an existing fixture is never skipped again"
			;;
		D) ;;
		*) ;;
		esac
		;;
	tests/fixtures/*)
		[ -n "$touches_src" ] && violations="$violations
- $path: a commit that changes fixtures changes no production code (src/); only deleting skip markers goes with an implementation"
		;;
	esac
done <<<"$changes"

if [ -n "$violations" ]; then
	echo "fixture rule violations in $(git log -1 --format='%h %s' "$rev"):$violations" >&2
	exit 1
fi
