#!/usr/bin/env bash
# The open-items sections of HANDOFF.md must agree with their own tables.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# On 2026-09-07, "Open items" advertised work that did not exist:
#
#   * **Small, sharp, cheap** opened with "Two open, and one more added on
#     2026-09-01". All **nine** of its rows were struck through. Three items of
#     available work, none of them real.
#   * **Real work, unstarted** opened with "Two items, and neither is actually
#     unblocked". Both were closed — one by deciding against it, one by
#     deletion.
#
# Every row was correctly struck when it closed. What nobody updated was the
# **prose above the table**, which is the part a reader acts on: the tables are
# a reference, and the intro is the summary someone picking this up cold reads
# to decide what to do next. A stale count there sends them looking for work
# that is finished.
#
# This is the same decay the rest of this repository's checkers guard against —
# a summary line written once while its subject was fresh, with nothing forcing
# anyone to look at it again — and it had reached the document whose whole
# purpose is to say what to do next.
#
# The mechanism is a machine-checkable line under each heading:
#
#     **N open, M closed.**
#
# and this script counts the rows and compares. A row is closed when its item
# cell is struck through (`~~…~~`), which is the convention the document
# already uses.
#
# Deliberately not clever: no attempt to parse the prose. A sentence can say
# whatever it needs to; the counted line is the part that has to be true, and
# putting it on its own line is what makes it checkable at all.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

DOC=HANDOFF.md

python - "$DOC" <<'PY'
import io
import re
import sys

doc = sys.argv[1]
lines = io.open(doc, encoding="utf-8", errors="replace").read().split("\n")

# The section to audit, and only that one: `## Open items` up to the next `##`.
start = None
end = len(lines)
for i, l in enumerate(lines):
    if l.strip() == "## Open items":
        start = i
    elif start is not None and l.startswith("## ") and i > start:
        end = i
        break

if start is None:
    print("  x  HANDOFF.md has no `## Open items` section; this check cannot")
    print("     mean anything. Rename it back, or delete this script.")
    sys.exit(1)

# Split into subsections keyed by their `###` heading.
subsections = []
current = None
for l in lines[start:end]:
    if l.startswith("### "):
        current = {"heading": l[4:].strip(), "claim": None, "open": 0, "closed": 0}
        subsections.append(current)
    elif current is not None:
        m = re.match(r"^\*\*(\d+) open, (\d+) closed\.\*\*", l.strip())
        if m:
            current["claim"] = (int(m.group(1)), int(m.group(2)))
        # A table row: `| <id> | <item> | … `
        row = re.match(r"^\|\s*(\d+)\s*\|\s*(.*?)\s*\|", l)
        if row:
            item = row.group(2)
            if item.startswith("~~"):
                current["closed"] += 1
            else:
                current["open"] += 1

failures = []
audited = 0
for s in subsections:
    if s["open"] == 0 and s["closed"] == 0:
        # A prose subsection with no table. Nothing to count, nothing to claim.
        continue
    audited += 1
    if s["claim"] is None:
        failures.append(
            f"`{s['heading']}` has {s['open']} open and {s['closed']} closed "
            f"row(s) and no `**N open, M closed.**` line under its heading"
        )
        continue
    want_open, want_closed = s["claim"]
    if (want_open, want_closed) != (s["open"], s["closed"]):
        failures.append(
            f"`{s['heading']}` says {want_open} open / {want_closed} closed; "
            f"its table has {s['open']} open / {s['closed']} closed"
        )

if audited == 0:
    print("  x  no subsection of `## Open items` has a table of items.")
    print("     Either the document changed shape or this script stopped")
    print("     finding it; a checker that reaches nothing must not pass.")
    sys.exit(1)

if failures:
    print(f"  x  {doc}'s open-item counts disagree with its own tables:")
    for f in failures:
        print(f"       {f}")
    print()
    print("     The tables are the truth. Update the line under the heading —")
    print("     it is what a reader picking this up cold acts on.")
    sys.exit(1)

total_open = sum(s["open"] for s in subsections)
total_closed = sum(s["closed"] for s in subsections)
print(
    f"  ok {audited} open-item table(s) agree with their headings: "
    f"{total_open} open, {total_closed} closed."
)
PY
