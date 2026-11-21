#!/bin/sh
exec strace -f -qq -e trace=%file,%network,%process -o "$TRACE_DIR/$(basename "$1").$$.trace" "$@"
