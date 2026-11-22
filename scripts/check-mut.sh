#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
work=$(mktemp -d) || exit 2
trap 'rm -rf "$work"' EXIT
scripts/ast-grep.sh scan --rule sg/count/mutable-bindings.yml --json=compact src >"$work/mut.json" || exit 2
scripts/ast-grep.sh scan --rule sg/count/functions.yml --json=compact src >"$work/fn.json" || exit 2
python3 -I - "$work/mut.json" "$work/fn.json" scripts/mut-baseline.txt "${1:-}" <<'PY'
import collections
import json
import re
import sys


def misbehaved(kind, value, trace):
    sys.stderr.write("%s: %s: %s\n" % (sys.argv[0], kind.__name__, value))
    sys.exit(2)


sys.excepthook = misbehaved

mutables = json.load(open(sys.argv[1]))
functions = json.load(open(sys.argv[2]))
limit = 2


def start(match):
    return (match["range"]["start"]["line"], match["range"]["start"]["column"])


def end(match):
    return (match["range"]["end"]["line"], match["range"]["end"]["column"])


names = {}
occurrences = collections.Counter()
for function in sorted(functions, key=lambda f: (f["file"], start(f))):
    found = re.search(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", function["text"])
    name = found.group(1) if found else "?"
    occurrences[(function["file"], name)] += 1
    suffix = "" if occurrences[(function["file"], name)] == 1 else "#%d" % occurrences[(function["file"], name)]
    names[(function["file"], start(function))] = name + suffix

counts = collections.Counter()
for mutable in mutables:
    enclosing = [
        f for f in functions
        if f["file"] == mutable["file"] and start(f) <= start(mutable) <= end(f)
    ]
    if not enclosing:
        counts[(mutable["file"], "<top level>")] += 1
        continue
    innermost = max(enclosing, key=start)
    counts[(innermost["file"], names[(innermost["file"], start(innermost))])] += 1

if sys.argv[4] == "--write-baseline":
    with open(sys.argv[3], "w") as baseline:
        for (path, name), count in sorted(counts.items()):
            if count > limit:
                baseline.write("%s\t%s\t%d\n" % (path, name, count))
    sys.exit(0)

baseline = {}
for line in open(sys.argv[3]):
    path, name, count = line.rstrip("\n").split("\t")
    baseline[(path, name)] = int(count)

violations = []
for key, count in sorted(counts.items()):
    allowed = baseline.get(key, limit)
    if count > allowed:
        violations.append("%s: %s has %d mutable bindings, %d allowed" % (key[0], key[1], count, allowed))
for key, allowed in sorted(baseline.items()):
    count = counts.get(key, 0)
    if count < allowed:
        violations.append(
            "%s: %s now has %d mutable bindings; lower its entry in scripts/mut-baseline.txt from %d"
            % (key[0], key[1], count, allowed)
        )

total = sum(counts.values())
print("mutable bindings in production code: %d, %d of them in the %d functions of scripts/mut-baseline.txt" % (total, sum(baseline.values()), len(baseline)))
for violation in violations:
    print(violation)
sys.exit(1 if violations else 0)
PY
