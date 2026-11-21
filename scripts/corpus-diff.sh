#!/bin/bash
set -u

usage="usage: corpus-diff.sh [dev|holdout] [--base <rev>]"
role=dev
base_rev=''
while [ $# -gt 0 ]; do
	case "$1" in
	dev | holdout) role=$1 ;;
	--base)
		[ $# -ge 2 ] || { echo "$usage" >&2; exit 2; }
		base_rev=$2
		shift
		;;
	*) echo "$usage" >&2; exit 2 ;;
	esac
	shift
done

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
profile=gjf

scripts/fetch-corpus.sh >/dev/null || exit 2
scripts/oracle.sh "$profile" "out/corpus/$role" "out/oracle/$profile/$role" || exit 2

if [ -z "$base_rev" ]; then
	default=$(git rev-parse --abbrev-ref origin/HEAD 2>/dev/null || echo origin/master)
	base_rev=$(git merge-base HEAD "$default") || exit 2
fi
base_sha=$(git rev-parse "$base_rev^{commit}") || exit 2

bins="$root/out/corpus-bin"
mkdir -p "$bins" || exit 2
cargo build --quiet --release --bin jeer || exit 1
head_bin="$bins/head"
cp target/release/jeer "$head_bin" || exit 2

base_bin="$bins/$base_sha"
if [ ! -e "$base_bin" ]; then
	tree=$(mktemp -d) || exit 2
	git worktree add --quiet --detach "$tree/src" "$base_sha" || exit 2
	if [ ! -e "$tree/src/Cargo.toml" ]; then
		printf '#!/bin/sh\nexit 3\n' >"$base_bin"
	elif ! (cd "$tree/src" && cargo build --quiet --release --bin jeer --target-dir "$root/out/base-target"); then
		git worktree remove --force "$tree/src"
		rm -rf "$tree"
		echo "corpus-diff: the base $base_sha does not build" >&2
		exit 2
	else
		cp "$root/out/base-target/release/jeer" "$base_bin" || exit 2
	fi
	chmod +x "$base_bin"
	git worktree remove --force "$tree/src" || exit 2
	rm -rf "$tree"
fi

run_one() {
	bin=$1 corpus=$2 oracle=$3 rel=$4
	actual=$(mktemp) || exit 2
	timeout 10 "$bin" --stdin --profile gjf <"$corpus/$rel" >"$actual" 2>/dev/null
	status=$?
	if [ -e "$oracle/$rel.error" ]; then
		if [ $status -eq 2 ] && [ ! -s "$actual" ]; then verdict=match; else verdict=mismatch; fi
	elif [ $status -le 1 ] && cmp -s "$actual" "$oracle/$rel"; then
		verdict=match
	elif [ $status -eq 124 ]; then
		verdict=timeout
	elif [ $status -eq 2 ]; then
		verdict=rejected
	elif [ $status -gt 2 ]; then
		verdict=crash
	else
		verdict=mismatch
	fi
	rm -f "$actual"
	printf '%s\t%s\n' "$rel" "$verdict"
}
export -f run_one

results="$root/out/corpus-results"
mkdir -p "$results" || exit 2
for side in base head; do
	bin_var="${side}_bin"
	bin=${!bin_var}
	hash=$(sha256sum <"$bin" | cut -c1-16)
	file="$results/$role-$profile-$hash.tsv"
	if [ ! -s "$file" ]; then
		(cd "out/corpus/$role" && find . -name '*.java' | sed 's#^\./##' | LC_ALL=C sort) |
			xargs -d '\n' -P "$(nproc)" -I{} bash -c 'run_one "$@"' _ "$bin" "$root/out/corpus/$role" "$root/out/oracle/$profile/$role" {} |
			LC_ALL=C sort >"$file.tmp" || exit 2
		mv "$file.tmp" "$file"
	fi
	printf -v "${side}_results" '%s' "$file"
done

cp "$head_results" "$results/latest-$role.tsv"
total=$(wc -l <"$head_results")
head_match=$(grep -c $'\tmatch$' "$head_results")
base_match=$(grep -c $'\tmatch$' "$base_results")
summary=$(cut -f2 "$head_results" | sort | uniq -c | awk '{printf "%s=%s ", $2, $1}')
printf 'corpus %s (%s): %d/%d match (base %s: %d) — %s\n' "$role" "$profile" "$head_match" "$total" "${base_sha:0:7}" "$base_match" "$summary"
printf 'per-file verdicts: %s\n' "out/corpus-results/latest-$role.tsv"

regressions=$(LC_ALL=C join -t $'\t' <(grep $'\tmatch$' "$base_results" | cut -f1) <(LC_ALL=C sort "$head_results") | awk -F'\t' '$2 != "match" {print "- " $1 ": " $2}')
if [ -n "$regressions" ]; then
	echo "regressions (matched at the base, not now):"
	printf '%s\n' "$regressions" | head -50
	exit 1
fi
