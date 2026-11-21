#!/bin/bash
set -u
set -o pipefail

[ $# -eq 1 ] || { echo "usage: check-roundtrip.sh <dev|holdout>" >&2; exit 2; }
role=$1
root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
scripts/fetch-corpus.sh >/dev/null || exit 2
cargo build --quiet --release --bin jeer || exit 1
bin="$root/target/release/jeer"

roundtrip_one() {
	bin=$1 file=$2
	actual=$(mktemp) || { printf 'ERROR %s: mktemp failed\n' "$file"; return; }
	timeout 10 "$bin" --stdin --profile lexer-roundtrip <"$file" >"$actual" 2>"$actual.err"
	status=$?
	if [ $status -gt 1 ]; then
		printf 'FAIL %s: exit %s %s\n' "$file" "$status" "$(head -c 200 "$actual.err")"
	elif ! cmp -s "$actual" "$file"; then
		printf 'FAIL %s: tokens do not reproduce the input\n' "$file"
	fi
	rm -f "$actual" "$actual.err"
}
export -f roundtrip_one

failures=$(find "out/corpus/$role" -name '*.java' -print0 |
	xargs -0 -P "$(nproc)" -I{} bash -c 'roundtrip_one "$@"' _ "$bin" {}) || {
	echo "check-roundtrip: the parallel run failed" >&2
	exit 2
}
if printf '%s' "$failures" | grep -q '^ERROR'; then
	printf '%s\n' "$failures" | grep '^ERROR' >&2
	exit 2
fi
total=$(find "out/corpus/$role" -name '*.java' | wc -l)
count=$(printf '%s' "$failures" | grep -c '^FAIL')
echo "roundtrip $role: $((total - count))/$total files reproduced"
if [ "$count" -gt 0 ]; then
	printf '%s\n' "$failures" | head -30
	exit 1
fi
