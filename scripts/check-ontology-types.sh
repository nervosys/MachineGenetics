#!/usr/bin/env bash
# `MAGE_ONTOLOGY.md` counts the compiler's own types. This checks the counts.
#
# ─────────────────────────────────────────────────────────────────────────────
# WHY THIS EXISTS
# ─────────────────────────────────────────────────────────────────────────────
#
# `DOCS.md` tells readers to prefer the ontology over the prose, and two of its
# tables are pure arithmetic about the source: a **type index** giving each
# enum's variant count (`Expr (ExprKind) | Syntactic | enum (31) | §13.1`) and a
# **Counts Summary** of the same kind of figure. Nothing compared either against
# the code.
#
# On 2026-09-07, first run, in a document an agent is told to trust first:
#
#     type index      Effect 16→18, Expr 31→35, ItemKind 18→20, Stmt 3→5,
#                     SemanticOp 18→17, TokenKind 168→182
#     Counts Summary  the same six, plus "Source modules 42" against 65
#
# `Stmt` is the sharp one. An agent reading this to learn what a statement can
# be is told there are three kinds — and `Guard` and `Defer`, two constructs
# with their own spec sections, are not among them.
#
# And **`RAP endpoints 24`**, in a document whose §5 was corrected on 2026-09-02
# from "24 endpoints" to 38, *with a note recording the correction*. One copy
# was fixed and the other, eighty lines further down in the same file, was not.
# The same shape as the four copies of the RAP method list, inside a single
# document.
#
# ── The counting is deliberately paranoid ─────────────────────────────────────
#
# Three earlier versions of this scan returned 179, 180 and 182 for
# `TokenKind`, because doc comments, attributes and nested delimiters each break
# a naive line count differently. A checker that cannot reproduce its own number
# would have replaced six wrong figures with six differently wrong ones.
#
# So: the body is taken by brace matching, comments and attributes are stripped,
# and variants are split on *top-level* commas — depth-tracked, so
# `Foo(Bar, Baz)` counts once. One algorithm, used everywhere here.
#
# ── Ambiguity is named, not guessed ───────────────────────────────────────────
#
# Ten type names are defined more than once across the five crates — `Severity`
# five times. An earlier pass of mine guessed, by taking the same-named enum
# with the most variants, and reported eleven disagreements of which four were
# it comparing `ContractKind` in `forge.rs` against a row about the one in
# `verify.rs`. **A checker that resolves an ambiguity by picking is a checker
# that invents findings.**
#
# So the document says which one it means, in the cell itself:
#
#     | Expr (ExprKind) | Syntactic | enum (35 @ prototype/src/ast.rs) | §13.1 |
#
# An ambiguous name with no `@ path` is a **failure**, not a skip: the fix is
# one annotation, and leaving it unchecked forever is how `Expr` and `ItemKind`
# — the two rows an agent is most likely to act on — would have stayed wrong.
#
# Rows that are not arithmetic about the source (`Ontological concepts`,
# `System invariants`) are listed as NOT CHECKED on every run, never
# suppressed. A field that disappears when empty teaches a reader to stop
# looking for it.
set -o errexit
set -o nounset
set -o pipefail

cd "$(dirname "$0")/.."

python - <<'PY'
import io
import json
import os
import re
import sys

DOC = "MAGE_ONTOLOGY.md"
ROOTS = ["prototype/src", "RecursiveMachineIntelligence/src", "ribosome/src",
         "germline/src", "forge/src"]


def variant_names(src, name):
    """Variants of `pub enum <name>` in `src`, or None if it is not there."""
    marker = f"pub enum {name} {{"
    start = src.find(marker)
    if start < 0:
        return None
    body = src[start + len(marker):]
    depth, i = 1, 0
    while depth > 0 and i < len(body):
        if body[i] == "{":
            depth += 1
        elif body[i] == "}":
            depth -= 1
        i += 1
    inner = body[:i - 1]
    inner = re.sub(r"/\*.*?\*/", "", inner, flags=re.S)
    inner = "\n".join(re.sub(r"//.*$", "", l) for l in inner.split("\n"))
    inner = "\n".join(l for l in inner.split("\n") if not l.strip().startswith("#["))
    parts, depth, cur = [], 0, ""
    for ch in inner:
        if ch in "({[":
            depth += 1
        elif ch in ")}]":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    if cur.strip():
        parts.append(cur)
    out = []
    for p in parts:
        m = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)", p.strip())
        if m:
            out.append(m.group(1))
    return out


# Every `pub enum` in the tree: name -> {path: variant count}.
defs = {}
for root in ROOTS:
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d != "target"]
        for fn in sorted(filenames):
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(dirpath, fn).replace(os.sep, "/")
            src = io.open(path, encoding="utf-8", errors="replace").read()
            for m in re.finditer(r"pub enum ([A-Za-z][A-Za-z0-9_]*) \{", src):
                name = m.group(1)
                vs = variant_names(src, name)
                if vs is not None:
                    defs.setdefault(name, {})[path] = len(vs)

doc = io.open(DOC, encoding="utf-8", errors="replace").read()
wrong, absent, ambiguous, unchecked = [], [], [], []
ok = 0

# ── The type index: `| Name | Domain | enum (N[ @ path]) | §x |` ─────────────
for m in re.finditer(
        r"^\|\s*([A-Za-z][A-Za-z0-9_]*)[^|]*\|[^|]*\|\s*enum \((\d+)(?:\s*@\s*([^)]+))?\)\s*\|",
        doc, re.M):
    name, want, hint = m.group(1), int(m.group(2)), m.group(3)
    places = defs.get(name)
    if hint:
        hint = hint.strip()
        got = (places or {}).get(hint)
        if got is None:
            absent.append(f"{name} — the row points at {hint}, which defines no `pub enum {name}`")
            continue
        if got != want:
            wrong.append((f"{name} @ {hint}", want, got))
        else:
            ok += 1
        continue
    if not places:
        absent.append(f"{name} — no crate defines `pub enum {name}`")
    elif len(places) > 1:
        where = ", ".join(f"{p} ({c})" for p, c in sorted(places.items()))
        ambiguous.append(f"{name} — defined in {len(places)} places; add `@ path`. Candidates: {where}")
    else:
        (path, got), = places.items()
        if got != want:
            wrong.append((f"{name} @ {path}", want, got))
        else:
            ok += 1

# ── The Counts Summary: rows that are arithmetic about the source ────────────
def builtin_effect_kinds():
    """The effect kinds a `/ …` annotation may name: every `Effect`
    variant except `Custom`, which represents a declared effect block.

    Asserts `Custom` is actually there rather than subtracting one on
    faith: if the variant is ever renamed, this must fail loudly rather
    than quietly return a number one short.
    """
    src = io.open("prototype/src/hir.rs", encoding="utf-8", errors="replace").read()
    vs = variant_names(src, "Effect")
    if vs is None or "Custom" not in vs:
        return None
    return len([v for v in vs if v != "Custom"])


def enum_count(path, name):
    src = io.open(path, encoding="utf-8", errors="replace").read()
    vs = variant_names(src, name)
    return None if vs is None else len(vs)


derived = {
    "Source modules": len([f for f in os.listdir("prototype/src") if f.endswith(".rs")]),
    "TokenKind variants": enum_count("prototype/src/lexer.rs", "TokenKind"),
    "AST ItemKind variants": enum_count("prototype/src/ast.rs", "ItemKind"),
    "AST ExprKind variants": enum_count("prototype/src/ast.rs", "Expr"),
    "AST Type variants": enum_count("prototype/src/ast.rs", "Type"),
    "HIR Ty variants": enum_count("prototype/src/hir.rs", "Ty"),
    # NOT the variant count. `Effect` has 18 variants and MAGE has
    # **17** built-in effect kinds: the eighteenth is `Custom(String)`,
    # which is how a *user-declared* `effect` block is represented, not a
    # name anyone can write in a `/ …` annotation. MAGE_SPEC.md §11.2 says
    # "these seventeen names", its table has seventeen rows, and the
    # ontology's own `effects` section lists seventeen kinds beside five
    # annotations.
    #
    # This row said 16, which is neither. I corrected it to 18 — the enum
    # count — and that was the wrong one of the two candidate meanings,
    # caught before it merged by asking what the *other* three documents
    # said. **A figure with two plausible readings needs the reading
    # written down, not just the number fixed.**
    "Effect kinds": builtin_effect_kinds(),
    "Semantic VCS operations": enum_count("prototype/src/semantic_vcs.rs", "SemanticOp"),
    "GradOp (autograd)": enum_count("prototype/src/autograd.rs", "GradOp"),
    "Consensus phases": enum_count("prototype/src/consensus.rs", "Phase"),
    "CRDT operations": enum_count("prototype/src/crdt.rs", "CrdtOp"),
}
try:
    onto = json.load(io.open("MAGE_ONTOLOGY.json", encoding="utf-8"))
    derived["RAP endpoints"] = onto["counts"]["rap_methods"]
    # `SKB rules total` is deliberately absent: the ontology JSON has no
    # skb key, and `check-skb-tree.sh` already regenerates the whole tree
    # and diffs it, which is a stronger check than a count. It stays in the
    # NOT CHECKED list here so its absence is visible rather than assumed.
except Exception:
    pass

summary_rows = dict(
    (m.group(1).strip(), int(m.group(2)))
    for m in re.finditer(r"^\|\s*([A-Za-z][^|]*?)\s*\|\s*(\d+)\s*\|\s*$", doc, re.M)
)
for label, real in sorted(derived.items()):
    if real is None:
        absent.append(f"Counts Summary `{label}` — the source it derives from was not found")
        continue
    if label not in summary_rows:
        absent.append(f"Counts Summary has no `{label}` row, and this script derives one")
        continue
    if summary_rows[label] != real:
        wrong.append((f"Counts Summary: {label}", summary_rows[label], real))
    else:
        ok += 1

for label in sorted(summary_rows):
    if label not in derived and re.match(r"^[A-Z]", label):
        unchecked.append(label)

print(f"{ok} claim(s) agree, {len(wrong)} disagree, {len(ambiguous)} ambiguous, "
      f"{len(absent)} name nothing.")

if unchecked:
    print()
    print("  NOT CHECKED — Counts Summary rows this script cannot derive from")
    print("  the source. Not a pass; nothing looked at them:")
    print("    " + ", ".join(unchecked))

fail = False
for group, rows in (("disagree with the source", wrong),
                    ("are ambiguous", ambiguous),
                    ("name nothing", absent)):
    if not rows:
        continue
    fail = True
    print()
    print(f"  x  {len(rows)} claim(s) {group}:")
    for r in rows:
        if isinstance(r, tuple):
            print(f"       {r[0]:<44} doc says {r[1]:>4}, source has {r[2]:>4}")
        else:
            print(f"       {r}")

if fail:
    print()
    print("     The source is the truth. Update the figure, and while you are in")
    print("     there, check whether the prose around it says the same number.")
    sys.exit(1)

print("  ok every derivable ontology count matches the source.")
PY
