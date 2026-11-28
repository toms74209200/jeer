#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
java_major=$(java -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p')
[ "$java_major" = "25" ] || { echo "generate-entity-table: needs JDK 25, the oracle's, found ${java_major:-none}" >&2; exit 2; }
jar=$(find "${GRADLE_USER_HOME:-$HOME/.gradle}/caches/modules-2/files-2.1/org.commonmark/commonmark/0.29.0" -name commonmark-0.29.0.jar | head -n 1)
[ -n "$jar" ] || { echo "generate-entity-table: commonmark-0.29.0.jar not in the Gradle cache; run the oracle once" >&2; exit 2; }
java -cp "$jar" "$root/tools/entity-table/EntityTable.java" >"$root/src/gjf/javadoc/entity_table.rs.tmp" || exit 2
mv "$root/src/gjf/javadoc/entity_table.rs.tmp" "$root/src/gjf/javadoc/entity_table.rs"
