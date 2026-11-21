#!/bin/bash
set -u

version=0.51.3
root=$(git rev-parse --show-toplevel) || exit 2
bin="$root/out/tools/ast-grep-$version/ast-grep"
if [ ! -x "$bin" ]; then
	case "$(uname -s)-$(uname -m)" in
	Linux-x86_64) asset=app-x86_64-unknown-linux-gnu.zip ;;
	Linux-aarch64) asset=app-aarch64-unknown-linux-gnu.zip ;;
	Darwin-arm64) asset=app-aarch64-apple-darwin.zip ;;
	*) echo "ast-grep.sh: unsupported platform" >&2; exit 2 ;;
	esac
	dir=$(dirname "$bin")
	mkdir -p "$dir" || exit 2
	curl -fsSL -o "$dir/ast-grep.zip" "https://github.com/ast-grep/ast-grep/releases/download/$version/$asset" || exit 2
	python3 -I -m zipfile -e "$dir/ast-grep.zip" "$dir" || exit 2
	chmod +x "$bin" || exit 2
fi
exec "$bin" "$@"
