#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
java_major=$(java -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p')
[ "$java_major" = "25" ] || { echo "generate-identifier-table: needs JDK 25, the oracle's, found ${java_major:-none}" >&2; exit 2; }
java "$root/tools/identifier-table/IdentifierTable.java" >"$root/src/lexer/identifier_table.rs.tmp" || exit 2
mv "$root/src/lexer/identifier_table.rs.tmp" "$root/src/lexer/identifier_table.rs"
