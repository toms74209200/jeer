#!/bin/bash
set -u

workflow_failure() {
	echo "fetch-corpus: $1" >&2
	exit 2
}

root=$(git rev-parse --show-toplevel) || workflow_failure "not in a git repository"
sources="$root/corpus/sources.tsv"
repos="$root/out/corpus-repos"
corpus="$root/out/corpus"

grep -v '^#' "$sources" | while IFS=$'\t' read -r name role url commit include; do
	repo="$repos/$name"
	if [ "$(git -C "$repo" rev-parse HEAD 2>/dev/null)" != "$commit" ]; then
		rm -rf "$repo" && mkdir -p "$repo" || workflow_failure "cannot create $repo"
		git -C "$repo" init -q || workflow_failure "git init failed for $name"
		git -C "$repo" fetch -q --depth 1 --filter=blob:none "$url" "$commit" || workflow_failure "cannot fetch $name at $commit"
		git -C "$repo" sparse-checkout set --no-cone $include || workflow_failure "sparse checkout failed for $name"
		git -C "$repo" checkout -q FETCH_HEAD || workflow_failure "checkout failed for $name"
	fi
	dest="$corpus/$role/$name"
	stamp="$dest/.commit"
	if [ "$(cat "$stamp" 2>/dev/null)" != "$commit $include" ]; then
		rm -rf "$dest" && mkdir -p "$dest" || workflow_failure "cannot create $dest"
		(cd "$repo" && find . -path ./.git -prune -o -type f -print | sed 's#^\./##') | while IFS= read -r rel; do
			case "$rel" in
			*.java) target="$rel" ;;
			*.input) target="${rel%.input}.java" ;;
			*) continue ;;
			esac
			mkdir -p "$dest/$(dirname "$target")"
			cp "$repo/$rel" "$dest/$target" || exit 2
		done || workflow_failure "cannot copy the files of $name"
		printf '%s %s\n' "$commit" "$include" >"$stamp"
	fi
	echo "$role/$name: $(find "$dest" -name '*.java' | wc -l) files"
done || exit 2
