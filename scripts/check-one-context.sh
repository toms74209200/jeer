#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
base=$(git merge-base HEAD origin/master 2>/dev/null || git merge-base HEAD master) || exit 2
git diff -U0 "$base" -- 'src/*.rs' >out/one-context.diff || exit 2
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
for line in open(diff):
    if line.startswith("--- "):
        source = line[4:].strip()
        old_path = None if source == "/dev/null" else source[2:]
    elif line.startswith("+++ "):
        target = line[4:].strip()
        new_path = None if target == "/dev/null" else target[2:]
    elif line.startswith("@@"):
        found = re.match(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@", line)
        old_start, old_length = int(found.group(1)), int(found.group(2) or "1")
        new_start, new_length = int(found.group(3)), int(found.group(4) or "1")
        production = False
        if new_path is not None and new_length > 0:
            production |= new_start < test_start(open(new_path).read())
        if old_path is not None and old_length > 0:
            production |= old_start < test_start(old_text(old_path))
        path = new_path or old_path
        if production and path != "src/main.rs":
            context = path[len("src/"):].split("/")[0]
            context = context[:-3] if context.endswith(".rs") else context
            contexts.setdefault(context, set()).add(path)
for context, paths in sorted(contexts.items()):
    print("%s: %s" % (context, ", ".join(sorted(paths))))
if len(contexts) > 1:
    print("one-context: production code changes %d contexts; a task changes one (src/main.rs and tests excepted)" % len(contexts))
    sys.exit(1)
print("one-context: production code changes at most one context")
PY
