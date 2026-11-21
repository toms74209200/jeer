#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
strace -qq -o /dev/null true || { echo "test-small: strace cannot trace processes here" >&2; exit 2; }
host=$(rustc -vV | sed -n 's/^host: //p') || exit 2
runner="CARGO_TARGET_$(printf '%s' "$host" | tr 'a-z-' 'A-Z_')_RUNNER"
mkdir -p out || exit 2
rm -rf out/small.lcov out/traces && mkdir -p out/traces || exit 2
env "$runner=$root/scripts/trace-exec.sh" TRACE_DIR="$root/out/traces" TERM=dumb \
	scripts/cargo-llvm-cov.sh --quiet --bins --lcov --output-path out/small.lcov
status=$?
[ $status -eq 0 ] && exit 0
[ $status -eq 2 ] && exit 2
[ -e out/small.lcov ] && exit 2
exit 1
