#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
cargo build --quiet --release --bin jeer || exit 1
bin="$root/target/release/jeer"
work=$(mktemp -d) || exit 2
trap 'rm -rf "$work"' EXIT

shapes='nest chain binary blocks lambdas generics'
generate() {
	python3 -I - "$1" "$2" "$3" <<'PY'
import sys
shape, n, path = sys.argv[1], int(sys.argv[2]), sys.argv[3]
body = {
    "nest": "class A { int x = " + "(" * n + "1" + ")" * n + "; }",
    "chain": "class A { void f() { a" + ".b(c)" * n + "; } }",
    "binary": "class A { int x = 1" + " + 1" * n + "; }",
    "blocks": "class A { void f() " + "{ if (a) " * n + "{}" + " }" * n + " }",
    "lambdas": "class A { R r = " + "x -> " * n + "1; }",
    "generics": "class A { " + "List<" * n + "A" + ">" * n + " x; }",
}[shape]
open(path, "w").write(body)
PY
}
fastest() {
	best=''
	for _ in 1 2 3; do
		start=$(date +%s%N)
		"$bin" --stdin --profile cst-roundtrip <"$1" >/dev/null 2>&1 || return 1
		elapsed=$(($(date +%s%N) - start))
		[ -z "$best" ] || [ "$elapsed" -lt "$best" ] && best=$elapsed
	done
	echo "$best"
}

failures=0
for shape in $shapes; do
	generate "$shape" 4000 "$work/small.java" || exit 2
	generate "$shape" 16000 "$work/large.java" || exit 2
	small=$(fastest "$work/small.java") || { echo "FAIL $shape: does not parse"; failures=$((failures + 1)); continue; }
	large=$(fastest "$work/large.java") || { echo "FAIL $shape: does not parse"; failures=$((failures + 1)); continue; }
	ratio=$(awk -v s="$small" -v l="$large" 'BEGIN { printf "%.1f", l / (s > 0 ? s : 1) }')
	echo "$shape: 4x the input takes ${ratio}x the time"
	awk -v r="$ratio" 'BEGIN { exit !(r > 8) }' && { echo "FAIL $shape: superlinear"; failures=$((failures + 1)); }
done
[ $failures -eq 0 ]
