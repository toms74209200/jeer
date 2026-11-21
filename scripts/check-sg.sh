#!/bin/bash
set -u

[ $# -eq 1 ] || { echo "usage: check-sg.sh <rule-id>" >&2; exit 2; }
root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
scripts/ast-grep.sh scan --filter "^$1\$" --report-style short
case $? in
0) exit 0 ;;
1) exit 1 ;;
*) exit 2 ;;
esac
