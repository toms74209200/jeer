#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
mkdir -p out || exit 2
rm -f out/small.lcov
scripts/cargo-llvm-cov.sh --quiet --bins --lcov --output-path out/small.lcov
status=$?
[ $status -eq 0 ] && exit 0
[ $status -eq 2 ] && exit 2
[ -e out/small.lcov ] && exit 2
exit 1
