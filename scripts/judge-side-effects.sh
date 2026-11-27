#!/bin/bash
set -u

[ $# -eq 1 ] || { echo "usage: judge-side-effects.sh <traces>" >&2; exit 2; }
[ -d "$1" ] || { echo "judge-side-effects: no traces in $1; run scripts/test-small.sh" >&2; exit 2; }
temp=${TMPDIR:-/tmp}
temp=${temp%/}/
target=${CARGO_TARGET_DIR:-$(git rev-parse --show-toplevel 2>/dev/null)/target}
target=${target%/}/

found=$(for trace in "$1"/*.trace; do
	[ -e "$trace" ] || continue
	awk -v test="$(basename "$trace" | sed 's/\.[0-9]*\.trace$//')" -v temp="$temp" -v root="$target" '
	function allowed(path, call) {
		if (path == "" || index(path, root) == 1 || index(path, temp) == 1) return 1
		if (call ~ /^mkdir(at)?$/ && index(root, path) == 1) return 1
		if (path ~ /terminfo/) return 1
		if (path == "/etc/ld.so.cache" || path == "/etc/ld.so.preload" || path ~ /^\/(usr\/)?lib(64|32|x32)?\//) return 1
		if (path ~ /\/toolchains\/[^\/]+\/lib\//) return 1
		if (index(path, "/proc/self/") == 1 || index(path, "/sys/fs/cgroup/") == 1 || index(path, "/sys/devices/system/cpu/") == 1 || path == "/proc/sys/vm/overcommit_memory") return 1
		return 0
	}
	{
		line = $0
		sub(/^[0-9]+ +/, "", line)
		call = line
		sub(/\(.*/, "", call)
	}
	call == "execve" && !started {
		started = 1
		next
	}
	call ~ /^(execve|execveat|fork|vfork)$/ || (call ~ /^clone3?$/ && line !~ /CLONE_THREAD/) {
		print test ": " line
		next
	}
	call ~ /^(socket|socketpair|connect|bind|listen|accept|accept4|sendto|recvfrom|sendmsg|recvmsg|sendmmsg|recvmmsg|shutdown|getsockopt|setsockopt|getsockname|getpeername)$/ {
		print test ": " line
		next
	}
	{
		rest = line
		while (match(rest, /"[^"]*"/)) {
			path = substr(rest, RSTART + 1, RLENGTH - 2)
			rest = substr(rest, RSTART + RLENGTH)
			if (!allowed(path, call)) {
				print test ": " line
				break
			}
		}
	}' "$trace"
done)

if [ -n "$found" ]; then
	echo "small tests reached the file system, the network or other processes (.shampoo/rules/side-effects.toml):" >&2
	printf '%s\n' "$found" >&2
	exit 1
fi
