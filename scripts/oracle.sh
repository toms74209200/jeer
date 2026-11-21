#!/bin/bash
set -u

usage="usage: oracle.sh <profile> <input-root> <output-root>"

workflow_failure() {
	echo "oracle: $1" >&2
	exit 2
}

[ $# -eq 3 ] || workflow_failure "$usage"
profile=$1
input_root=$(cd "$2" && pwd) || workflow_failure "no input root: $2"
mkdir -p "$3" || workflow_failure "cannot create output root: $3"
output_root=$(cd "$3" && pwd)

root=$(git rev-parse --show-toplevel) || workflow_failure "not in a git repository"
oracle="$root/oracle"

java_major=$(java -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p')
[ "$java_major" = "25" ] || workflow_failure "the oracle runs on JDK 25 (latest LTS), found: ${java_major:-none}"

config_hash=$(cat "$oracle/build.gradle.kts" "$oracle/settings.gradle.kts" "$oracle/gradle/wrapper/gradle-wrapper.properties" | sha256sum | cut -c1-16)
cache="$root/out/oracle-cache/$profile-$config_hash"
mkdir -p "$cache" || workflow_failure "cannot create cache: $cache"

work="$oracle/work"
rm -rf "$work" && mkdir -p "$work" || workflow_failure "cannot create $work"

list=$(mktemp) || workflow_failure "mktemp failed"
trap 'rm -f "$list" "$list.log"' EXIT
(cd "$input_root" && find . -type f -name '*.java' | sed 's#^\./##' | LC_ALL=C sort) >"$list"

pending=0
while IFS= read -r rel; do
	hash=$(sha256sum <"$input_root/$rel" | cut -c1-64)
	if [ ! -e "$cache/$hash.java" ] && [ ! -e "$cache/$hash.error" ] && [ ! -e "$work/$hash.java" ]; then
		cp "$input_root/$rel" "$work/$hash.java" || workflow_failure "cannot copy $rel"
		pending=$((pending + 1))
	fi
done <"$list"

if [ "$pending" -gt 0 ]; then
	(cd "$oracle" && ./gradlew -q --console=plain spotlessApply -Pprofile="$profile") >"$list.log" 2>&1
	status=$?
	if [ $status -ne 0 ]; then
		reported=$(sed -n 's/.*There were \([0-9]*\) lint error(s).*/\1/p' "$list.log" | head -1)
		[ -n "$reported" ] || { cat "$list.log" >&2; workflow_failure "gradle failed without lint errors"; }
		parsed=$(grep -cE '^ *work/[0-9a-f]{64}\.java:' "$list.log")
		[ "$parsed" -eq "$reported" ] || { cat "$list.log" >&2; workflow_failure "spotless reported $reported lint errors but listed $parsed"; }
		grep -E '^ *work/[0-9a-f]{64}\.java:' "$list.log" | sed 's/^ *work\///' | while IFS= read -r line; do
			hash=${line%%.java:*}
			printf '%s\n' "${line#*.java:}" >>"$work/$hash.error"
		done
	fi
	for f in "$work"/*.java; do
		hash=$(basename "$f" .java)
		if [ -e "$work/$hash.error" ]; then
			mv "$work/$hash.error" "$cache/$hash.error"
		else
			mv "$f" "$cache/$hash.java"
		fi
	done
fi
rm -rf "$work"

while IFS= read -r rel; do
	hash=$(sha256sum <"$input_root/$rel" | cut -c1-64)
	mkdir -p "$(dirname "$output_root/$rel")"
	rm -f "$output_root/$rel" "$output_root/$rel.error"
	if [ -e "$cache/$hash.error" ]; then
		cp "$cache/$hash.error" "$output_root/$rel.error"
	elif [ -e "$cache/$hash.java" ]; then
		cp "$cache/$hash.java" "$output_root/$rel"
	else
		workflow_failure "no oracle result for $rel"
	fi
done <"$list"
