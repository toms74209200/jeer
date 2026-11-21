#!/bin/bash
set -u

version=6.4.0
root=$(git rev-parse --show-toplevel) || exit 2
dir="$root/out/tools/dylint-$version"
if [ ! -x "$dir/bin/cargo-dylint" ] || [ ! -x "$dir/bin/dylint-link" ]; then
	cargo install --quiet --locked --version "$version" --root "$dir" cargo-dylint dylint-link >&2 || exit 2
fi
[ "${1:-}" = "--bin-dir" ] && { echo "$dir/bin"; exit 0; }
PATH="$dir/bin:$PATH" exec cargo dylint "$@"
