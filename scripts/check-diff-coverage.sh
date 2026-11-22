#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=$(git merge-base HEAD origin/master 2>/dev/null || git merge-base HEAD master) || exit 2
newest=$(find src -name '*.rs' -newer out/small.lcov 2>/dev/null | head -1)
if [ ! -e out/small.lcov ] || [ -n "$newest" ]; then
	scripts/test-small.sh >/dev/null 2>&1
	status=$?
	[ $status -eq 0 ] || exit $status
fi
git diff -U0 "$base" -- 'src/*.rs' >out/diff-coverage.diff || exit 2
python3 -I - out/diff-coverage.diff out/small.lcov scripts/side-effect-modules.txt "$root" <<'PY'
import re
import sys


def misbehaved(kind, value, trace):
    sys.stderr.write("check-diff-coverage: %s: %s\n" % (kind.__name__, value))
    sys.exit(2)


sys.excepthook = misbehaved
diff, lcov, effects, root = sys.argv[1:5]
side_effects = [line.strip() for line in open(effects) if line.strip()]
counts = {}
current = None
for line in open(lcov):
    line = line.strip()
    if line.startswith("SF:"):
        current = line[3:].replace(root + "/", "")
        counts.setdefault(current, {})
    elif line.startswith("DA:") and current is not None:
        number, hits = line[3:].split(",")[:2]
        counts[current][int(number)] = int(hits)
added = {}
path = None
for line in open(diff):
    if line.startswith("+++ "):
        target = line[4:].strip()
        path = None if target == "/dev/null" else target[2:]
    elif line.startswith("@@") and path is not None:
        found = re.match(r"@@ -\S+ \+(\d+)(?:,(\d+))? @@", line)
        start, length = int(found.group(1)), int(found.group(2) or "1")
        added.setdefault(path, []).extend(range(start, start + length))
uncovered = []
for path, lines in sorted(added.items()):
    if any(path == entry or (entry.endswith("/") and path.startswith(entry)) for entry in side_effects):
        continue
    source = open(path).read().split("\n")
    tests = next((index + 1 for index, text in enumerate(source) if text.strip() == "#[cfg(test)]"), len(source) + 1)
    for number in lines:
        if number < tests and counts.get(path, {}).get(number) == 0:
            uncovered.append("%s:%d: %s" % (path, number, source[number - 1].strip()))
for entry in uncovered:
    print(entry)
if uncovered:
    print("diff-coverage: %d added line(s) of production code no small test runs" % len(uncovered))
    sys.exit(1)
print("diff-coverage: every added line of production code runs in a small test")
PY
