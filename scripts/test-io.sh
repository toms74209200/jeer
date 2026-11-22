#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
cargo build --quiet --release --bin jeer || exit 1
bin="$root/target/release/jeer"
out=$(mktemp -d) || exit 2
trap 'rm -rf "$out"' EXIT
failures=0

printf 'class A{int x;}\n' | "$bin" --stdin --profile gjf >"$out/stdout" 2>"$out/stderr"
status=$?
if [ $status -ne 1 ] || [ "$(cat "$out/stdout")" != "$(printf 'class A {\n  int x;\n}')" ] || [ -s "$out/stderr" ]; then
	echo "FAIL stdin to stdout: exit $status, stderr $(head -c 200 "$out/stderr")"
	failures=$((failures + 1))
fi

printf 'class A {\n' | "$bin" --stdin --profile gjf >"$out/stdout" 2>"$out/stderr"
status=$?
if [ $status -ne 2 ] || [ -s "$out/stdout" ] || ! grep -qF 'jeer: 1:10:' "$out/stderr"; then
	echo "FAIL rejection to stderr: exit $status, stderr $(head -c 200 "$out/stderr")"
	failures=$((failures + 1))
fi

printf 'class A {}\n' | "$bin" --stdin --profile gjf 2>"$out/stderr" >/dev/full
status=$?
if [ $status -ne 3 ] || ! grep -qF 'jeer: cannot write stdout:' "$out/stderr"; then
	echo "FAIL failing stdout write: exit $status, stderr $(head -c 200 "$out/stderr")"
	failures=$((failures + 1))
fi

printf 'class A {\n' | "$bin" --stdin --profile gjf >"$out/stdout" 2>/dev/full
status=$?
if [ $status -ne 3 ] || [ -s "$out/stdout" ]; then
	echo "FAIL failing stderr write: exit $status"
	failures=$((failures + 1))
fi

[ $failures -eq 0 ] && { echo "io: all cases pass"; exit 0; }
exit 1
