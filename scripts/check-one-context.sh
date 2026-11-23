#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=$(git merge-base HEAD origin/master 2>/dev/null || git merge-base HEAD master) || exit 2
mkdir -p out || exit 2
untracked=$(git ls-files --others --exclude-standard -- 'src/*.rs') || exit 2
git diff -U0 --no-renames "$base" -- 'src/*.rs' >out/one-context.diff || exit 2
for file in $untracked; do
	git diff -U0 --no-renames --no-index /dev/null "$file" >>out/one-context.diff
	[ $? -le 1 ] || exit 2
done
python3 -I - out/one-context.diff "$base" <<'PY'
import re
import subprocess
import sys


def misbehaved(kind, value, trace):
    sys.stderr.write("check-one-context: %s: %s\n" % (kind.__name__, value))
    sys.exit(2)


sys.excepthook = misbehaved
diff, base = sys.argv[1:3]


def test_start(text):
    lines = text.split("\n")
    return next((index + 1 for index, line in enumerate(lines) if line.strip() == "#[cfg(test)]"), len(lines) + 1)


def old_text(path):
    shown = subprocess.run(["git", "show", "%s:%s" % (base, path)], capture_output=True, text=True)
    return shown.stdout if shown.returncode == 0 else ""


contexts = {}
old_path = new_path = None
old_line = new_line = 0
old_tests = new_tests = 0


def record(path):
    if path != "src/main.rs":
        context = path[len("src/"):].split("/")[0]
        context = context[:-3] if context.endswith(".rs") else context
        contexts.setdefault(context, set()).add(path)


for line in open(diff):
    if line.startswith("--- "):
        source = line[4:].strip()
        old_path = None if source == "/dev/null" else source.split("/", 1)[1]
        old_tests = test_start(old_text(old_path)) if old_path else 0
    elif line.startswith("+++ "):
        target = line[4:].strip()
        new_path = None if target == "/dev/null" else target.split("/", 1)[1]
        new_tests = test_start(open(new_path).read()) if new_path else 0
    elif line.startswith("@@"):
        found = re.match(r"@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@", line)
        old_line, new_line = int(found.group(1)), int(found.group(2))
    elif line.startswith("-") and old_path:
        if line[1:].strip() and old_line < old_tests:
            record(old_path)
        old_line += 1
    elif line.startswith("+") and new_path:
        if line[1:].strip() and new_line < new_tests:
            record(new_path)
        new_line += 1
for context, paths in sorted(contexts.items()):
    print("%s: %s" % (context, ", ".join(sorted(paths))))
if len(contexts) > 1:
    print("one-context: production code changes %d contexts; a task changes one (src/main.rs and tests excepted)" % len(contexts))
    sys.exit(1)
print("one-context: production code changes at most one context")
PY
