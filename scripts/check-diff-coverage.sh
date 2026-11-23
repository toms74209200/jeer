#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=$(git merge-base HEAD origin/master 2>/dev/null || git merge-base HEAD master) || exit 2
mkdir -p out || exit 2
scripts/test-small.sh >/dev/null 2>&1
status=$?
[ $status -eq 0 ] || exit $status
untracked=$(git ls-files --others --exclude-standard -- 'src/*.rs') || exit 2
git diff -U0 --no-renames "$base" -- 'src/*.rs' >out/diff-coverage.diff || exit 2
for file in $untracked; do
	git diff -U0 --no-renames --no-index /dev/null "$file" >>out/diff-coverage.diff
	[ $? -le 1 ] || exit 2
done
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
        path = None if target == "/dev/null" else target.split("/", 1)[1]
    elif line.startswith("@@") and path is not None:
        found = re.match(r"@@ -\S+ \+(\d+)(?:,(\d+))? @@", line)
        start, length = int(found.group(1)), int(found.group(2) or "1")
        if length > 0:
            added.setdefault(path, []).extend(range(start, start + length))
uncovered = []
for path, lines in sorted(added.items()):
    if any(path == entry or (entry.endswith("/") and path.startswith(entry)) for entry in side_effects):
        continue
    source = open(path).read().split("\n")
    tests = next((index + 1 for index, text in enumerate(source) if text.strip() == "#[cfg(test)]"), len(source) + 1)
    if path not in counts:
        if any(re.search(r"\bfn\b", text) for text in source[: tests - 1]):
            uncovered.append("%s: no small test runs this file (it is not in out/small.lcov)" % path)
        continue
    for number in lines:
        text = source[number - 1].strip()
        guard = re.fullmatch(r"(_ => )?return Err\(Unformatted::Deferred\)[;,]", text)
        closer = re.fullmatch(r"[)\]}]+\?[;,]?", text)
        if number < tests and counts[path].get(number) == 0 and not (guard or closer):
            uncovered.append("%s:%d: %s" % (path, number, text))
for entry in uncovered:
    print(entry)
if uncovered:
    print("diff-coverage: %d added line(s) of production code no small test runs" % len(uncovered))
    sys.exit(1)
print("diff-coverage: every added line of production code runs in a small test")
PY
