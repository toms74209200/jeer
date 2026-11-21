#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
command -v cargo >/dev/null || { echo "lint: cargo not found" >&2; exit 2; }

mkdir -p out/exceptions || exit 2
dirs=()
for d in src tests benches tools; do
	[ -d "$d" ] && dirs+=("$d")
done
grep -rnE '#!?\[expect\(' --include='*.rs' "${dirs[@]}" >out/exceptions/lint.txt
[ $? -le 1 ] || exit 2

result=0
cargo fmt --all --check || result=1
cargo clippy --quiet --all-targets --all-features -- -D warnings || result=1
dylint_bin=$(scripts/dylint.sh --bin-dir) || exit 2
for tool in tools/*/Cargo.toml; do
	[ -e "$tool" ] || continue
	(cd "$(dirname "$tool")" && PATH="$dylint_bin:$PATH" cargo fmt --check && PATH="$dylint_bin:$PATH" cargo clippy --quiet -- -D warnings) || result=1
done
exit $result
