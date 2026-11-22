#!/bin/bash
set -u

root=$(git rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
branch=${1:-$(git rev-parse --abbrev-ref HEAD)} || exit 2
transcripts="$HOME/.claude/projects/$(printf '%s' "$root" | sed 's#[^A-Za-z0-9]#-#g')"
if [ ! -d "$transcripts" ]; then
	echo "token-usage: no session transcripts at $transcripts; nothing to observe"
	exit 0
fi
base=$(git merge-base origin/master "$branch" 2>/dev/null || git merge-base master "$branch") || exit 2
read -r added deleted < <(git diff --numstat "$base" "$branch" -- src | awk '{a += $1; d += $2} END {print a + 0, d + 0}')
mkdir -p out/token-usage || exit 2
python3 -I - "$transcripts" "$branch" "$added" "$deleted" "out/token-usage/$branch.json" <<'PY'
import glob
import json
import os
import sys


def misbehaved(kind, value, trace):
    sys.stderr.write("%s: %s: %s\n" % (sys.argv[0], kind.__name__, value))
    sys.exit(2)


sys.excepthook = misbehaved

transcripts, branch, added, deleted, output = sys.argv[1:6]
files = glob.glob(os.path.join(transcripts, "*.jsonl")) + glob.glob(
    os.path.join(transcripts, "*", "subagents", "*.jsonl")
)
usages = {}
for path in files:
    for line in open(path, encoding="utf-8", errors="replace"):
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        message = record.get("message")
        if record.get("type") != "assistant" or record.get("gitBranch") != branch:
            continue
        if not isinstance(message, dict) or "usage" not in message or "id" not in message:
            continue
        usages[message["id"]] = message["usage"]
kinds = ["input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
totals = {kind: sum(usage.get(kind, 0) for usage in usages.values()) for kind in kinds}
total = sum(totals.values())
lines = int(added) + int(deleted)
report = {
    "branch": branch,
    "messages": len(usages),
    "tokens": totals,
    "total_tokens": total,
    "src_lines_added": int(added),
    "src_lines_deleted": int(deleted),
    "total_tokens_per_src_line": round(total / lines) if lines else None,
    "output_tokens_per_src_line": round(totals["output_tokens"] / lines) if lines else None,
}
with open(output, "w") as out:
    json.dump(report, out, indent=1)
print(
    "token-usage %s: %d tokens over %d messages (output %d, cache read %d, cache creation %d, input %d) "
    "for +%s/-%s lines in src: %s tokens per line, %s output tokens per line"
    % (
        branch, total, len(usages), totals["output_tokens"], totals["cache_read_input_tokens"],
        totals["cache_creation_input_tokens"], totals["input_tokens"], added, deleted,
        report["total_tokens_per_src_line"], report["output_tokens_per_src_line"],
    )
)
PY
