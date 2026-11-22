#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
cargo build --quiet --release --bin jeer || exit 1
bin="$root/target/release/jeer"
out=$(mktemp -d) || exit 2
trap 'rm -rf "$out"' EXIT

failures=0
expect() {
	local name=$1 want_status=$2 want_stdout=$3 want_stderr=$4 status=$5
	if [ "$status" -ne "$want_status" ] || [ "$(cat "$out/stdout")" != "$want_stdout" ] || { [ -z "$want_stderr" ] && [ -s "$out/stderr" ]; } || { [ -n "$want_stderr" ] && ! grep -qF -- "$want_stderr" "$out/stderr"; }; then
		echo "FAIL $name: exit $status, stdout $(head -c 200 "$out/stdout" | od -c | head -3), stderr $(head -c 200 "$out/stderr")"
		failures=$((failures + 1))
	fi
}

printf 'class A{int x;}\n' | "$bin" --stdin --profile gjf >"$out/stdout" 2>"$out/stderr"
status=$?
expect "stdin to stdout" 1 "$(printf 'class A {\n  int x;\n}')" "" $status

printf 'class A {\n' | "$bin" --stdin --profile gjf >"$out/stdout" 2>"$out/stderr"
status=$?
expect "rejection to stderr" 2 "" "jeer: 2:1:" $status

printf 'class A {}\n' | "$bin" --stdin --profile gjf 2>"$out/stderr" >/dev/full
status=$?
: >"$out/stdout"
expect "failing stdout write" 3 "" "jeer: cannot write stdout:" $status

[ $failures -eq 0 ] && { echo "io: all cases pass"; exit 0; }
exit 1
