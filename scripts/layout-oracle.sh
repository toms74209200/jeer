#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
java_major=$(java -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p')
[ "$java_major" = "25" ] || { echo "layout-oracle: needs JDK 25, found ${java_major:-none}" >&2; exit 2; }
(cd "$root/oracle" && ./gradlew -q --console=plain spotlessDiagnose >/dev/null 2>&1) || true
gjf=$(find "${GRADLE_USER_HOME:-$HOME/.gradle}/caches" -name 'google-java-format-2.0.0.jar' | head -1)
guava=$(find "${GRADLE_USER_HOME:-$HOME/.gradle}/caches" -name 'guava-*-jre.jar' | grep -v sources | sort | tail -1)
[ -n "$gjf" ] && [ -n "$guava" ] || { echo "layout-oracle: run scripts/oracle.sh once so Gradle fetches google-java-format" >&2; exit 2; }
exec java -cp "$gjf:$guava" "$root/tools/layout-oracle/LayoutCases.java"
