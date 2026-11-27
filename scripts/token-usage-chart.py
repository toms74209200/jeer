"""Chart the tokens each task spent, from the token usage records.

Reads the body of the issue labelled token-usage-records, whose JSONL block
holds one record per branch (scripts/token-usage.sh), and a JSON list of
pull requests (`gh pr list --json number,title,headRefName`), which names
each branch's PR. Writes one self-contained HTML page: the tokens of each
PR stacked by kind, the same without cache reads (which are most of every
total and hide the rest), and a table.
Usage: token-usage-chart.py <issue body> <prs.json> [highlight PR]
"""

import html
import json
import sys


def misbehaved(kind, value, trace):
    sys.stderr.write("%s: %s: %s\n" % (sys.argv[0], kind.__name__, value))
    sys.exit(2)


sys.excepthook = misbehaved

KINDS = [
    ("cache_read_input_tokens", "cache read", "--series-1"),
    ("cache_creation_input_tokens", "cache write", "--series-2"),
    ("output_tokens", "output", "--series-3"),
    ("input_tokens", "input", "--series-4"),
]
WIDTH, HEIGHT, LEFT, RIGHT, TOP, BOTTOM = 960, 260, 64, 16, 16, 64

_, fence, rest = open(sys.argv[1]).read().replace("\r\n", "\n").partition("```jsonl\n")
block, close, _ = rest.partition("```")
if not fence or not close:
    sys.stderr.write("token-usage-chart: the records issue has no jsonl block\n")
    sys.exit(2)
records = {found["branch"]: found for found in (json.loads(line) for line in block.splitlines() if line)}
prs = json.load(open(sys.argv[2]))
highlight = int(sys.argv[3]) if len(sys.argv) > 3 else None
parsed = sorted(((pr["number"], pr["title"], records.get(pr["headRefName"])) for pr in prs), key=lambda row: row[0])
rows = [(number, title, found) for number, title, found in parsed if found]
first = min((number for number, _, _ in rows), default=None)
skipped = [number for number, _, found in parsed if not found and first is not None and number > first]
if highlight in skipped:
    sys.stderr.write("#%d is not charted: no token-usage record\n" % highlight)


def nice(value):
    steps = [m * 10**e for e in range(0, 12) for m in (1, 2, 5)]
    return next(step for step in steps if step * 4 >= value)


def amount(value):
    return next(
        "%.3g%s" % (value / scale, suffix)
        for scale, suffix in ((1e6, "M"), (1e3, "k"), (1, ""))
        if value >= scale or scale == 1
    )


def panel(title, kinds):
    totals = [sum(record["tokens"][key] for key, _, _ in kinds) for _, _, record in rows]
    step = nice(max(totals + [1]))
    top = step * 4
    plot_w, plot_h = WIDTH - LEFT - RIGHT, HEIGHT - TOP - BOTTOM
    slot = plot_w / max(len(rows), 1)
    bar = max(min(slot * 0.7, 40), 2)
    y = lambda v: TOP + plot_h - v / top * plot_h
    grid = "".join(
        '<line class="grid" x1="%d" x2="%d" y1="%.1f" y2="%.1f"/>'
        '<text class="tick" x="%d" y="%.1f" text-anchor="end">%s</text>'
        % (LEFT, WIDTH - RIGHT, y(i * step), y(i * step), LEFT - 6, y(i * step) + 4, amount(i * step))
        for i in range(5)
    )
    columns = "".join(
        '<g class="col%s"><title>%s</title>%s%s'
        '<text class="tick" x="%.1f" y="%d" text-anchor="end" transform="rotate(-45 %.1f %d)">#%d</text></g>'
        % (
            " highlight" if number == highlight else "",
            html.escape(
                "#%d %s\n%s\n" % (number, title, record["branch"])
                + "\n".join("%s: %s" % (label, format(record["tokens"][key], ",")) for key, label, _ in KINDS)
                + "\ntotal: %s" % format(record["total_tokens"], ",")
            ),
            '<rect class="hit" x="%.1f" y="%d" width="%.1f" height="%d"/>'
            % (LEFT + i * slot, TOP, slot, plot_h),
            "".join(
                '<rect x="%.1f" y="%.1f" width="%.1f" height="%.1f" style="fill:var(%s)"/>'
                % (LEFT + i * slot + (slot - bar) / 2, y(base + value), bar, max(y(base) - y(base + value) - 2, 0), color)
                for (key, _, color), base, value in (
                    (kind, sum(record["tokens"][k] for k, _, _ in kinds[:j]), record["tokens"][kind[0]])
                    for j, kind in enumerate(kinds)
                )
                if value
            ),
            LEFT + i * slot + slot / 2, HEIGHT - BOTTOM + 14, LEFT + i * slot + slot / 2, HEIGHT - BOTTOM + 14,
            number,
        )
        for i, (number, title, record) in enumerate(rows)
    )
    legend = "".join(
        '<span><i style="background:var(%s)"></i>%s</span>' % (color, label) for _, label, color in kinds
    )
    return '<h2>%s</h2><div class="legend">%s</div><svg viewBox="0 0 %d %d" role="img" aria-label="%s">%s%s</svg>' % (
        title, legend, WIDTH, HEIGHT, title, grid, columns,
    )


total = sum(record["total_tokens"] for _, _, record in rows)
cache_read = sum(record["tokens"]["cache_read_input_tokens"] for _, _, record in rows)
table = "".join(
    "<tr%s><td>#%d</td><td>%s</td>%s<td>%s</td><td>%s</td></tr>"
    % (
        ' class="highlight"' if number == highlight else "",
        number,
        html.escape(title),
        "".join("<td>%s</td>" % format(record["tokens"][key], ",") for key, _, _ in KINDS),
        format(record["total_tokens"], ","),
        "" if record["total_tokens_per_src_line"] is None else format(record["total_tokens_per_src_line"], ","),
    )
    for number, title, record in rows
)
print(
    """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Token usage per task</title>
<style>
:root { color-scheme: light; --surface-1: #ffffff; --text-primary: #191919; --text-secondary: #666666;
  --grid: #e5e5e5; --series-1: #257de0; --series-2: #f47230; --series-3: #00b894; --series-4: #f2a600; }
@media (prefers-color-scheme: dark) { :root { color-scheme: dark; --surface-1: #191919; --text-primary: #efefef;
  --text-secondary: #9c9c9c; --grid: #333333; --series-1: #338ced; --series-2: #e76624; --series-3: #00a888;
  --series-4: #d28e05; } }
body { margin: 24px auto; max-width: 1000px; padding: 0 16px; background: var(--surface-1); color: var(--text-primary);
  font: 14px/1.5 system-ui, sans-serif; }
h1 { font-size: 20px; margin: 0 0 4px; } h2 { font-size: 15px; margin: 28px 0 4px; }
.lede { color: var(--text-secondary); margin: 0; }
.legend { display: flex; gap: 16px; color: var(--text-secondary); font-size: 12px; }
.legend i { display: inline-block; width: 10px; height: 10px; border-radius: 2px; margin-right: 6px; }
svg { width: 100%%; height: auto; display: block; }
.grid { stroke: var(--grid); stroke-width: 1; }
.tick { fill: var(--text-secondary); font-size: 11px; font-variant-numeric: tabular-nums; }
.hit { fill: transparent; } .col:hover .hit, .col.highlight .hit { fill: var(--grid); opacity: .5; }
.col.highlight .tick { fill: var(--text-primary); font-weight: 600; }
table { border-collapse: collapse; width: 100%%; font-size: 12px; font-variant-numeric: tabular-nums; }
th, td { padding: 4px 8px; border-bottom: 1px solid var(--grid); text-align: right; }
th:nth-child(2), td:nth-child(2) { text-align: left; }
tr.highlight td { font-weight: 600; }
</style></head><body>
<h1>Token usage per task</h1>
<p class="lede">%d PRs, %s tokens in all; cache reads are %.1f%% of them. Hover a column for its numbers.</p>
%s
%s
%s
<h2>Table</h2>
<table><tr><th>PR</th><th>Title</th>%s<th>total</th><th>per src line</th></tr>%s</table>
</body></html>"""
    % (
        len(rows),
        amount(total),
        100 * cache_read / total if total else 0,
        panel("All tokens, by kind", KINDS),
        panel("Without cache reads", KINDS[1:]),
        '<p class="lede">Not charted, without a token-usage record: %s</p>' % ", ".join("#%d" % n for n in skipped)
        if skipped
        else "",
        "".join("<th>%s</th>" % label for _, label, _ in KINDS),
        table,
    )
)
