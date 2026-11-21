#!/bin/bash
set -u

version=0.9.2
root=$(git rev-parse --show-toplevel) || exit 2
dir="$root/out/tools/cargo-llvm-cov-$version"
if [ ! -x "$dir/bin/cargo-llvm-cov" ]; then
	cargo install --quiet --locked --version "$version" --root "$dir" cargo-llvm-cov >&2 || exit 2
fi
PATH="$dir/bin:$PATH" exec cargo llvm-cov "$@"
