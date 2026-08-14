# `stdlib/` — a design sketch, not a standard library

**These 25 `.mg` files are Rust.** `pub trait Read`, `&mut self`,
`let total = 0usize;`, `pub mod agent;`, `use std::io::{Read, Write};`. None of
them parse as MAGE, and none ever have.

They are kept because the design intent is worth keeping. They are labelled
because, unlabelled, they were worse than nothing: an agent reading
`stdlib/std/io.mg` to learn MAGE idiom learns Rust instead.

## How this happened

Nothing consumed them. The compiler never opens this directory; no script
checked it; and `u std.io` resolves *without* it, because `use` brings nothing
into scope. A file with no consumer has no error message, so 4,402 lines could
sit here for months claiming to be the standard library of a language they were
not written in.

`scripts/check-mg-sources.sh` is now that consumer, and runs in CI. Every `.mg`
file in the repository typechecks or is listed there with a reason. This
directory is listed.

## Why it is not simply rewritten

Because there is nothing to rewrite it *into*, and nothing that would keep it
honest afterwards.

MAGE has no module system. `resolve_use` parses a path and discards it, so a
library cannot be imported. The library surface is instead **global**: the
standard vocabulary (`map`, `filter`, `fold`, `join`, … — 31 combinators in
`resolve::VOCABULARY`) and the capability namespaces (`io`, `fs`, `net`, `llm`,
`gpu`, `agent`, … — 20, in `hir::CAPABILITY_NAMESPACES`) are in scope
everywhere, with no import and no tokens spent on one.

For a language optimising for token efficiency that is arguably the right
design, not a missing feature. It also means a hand-written `std.io` module has
no way to be reached, and translating 4,402 lines into MAGE would produce code
that still nothing imports — which is exactly how this directory got into its
current state. Rewriting without a consumer reproduces the bug.

## What would make this real

In order, each a prerequisite for the next:

1. **A module system** — `resolve_use` that walks a module tree, or a decision
   that MAGE deliberately has no imports and this directory should be deleted.
   That decision is the actual blocker; everything below is downstream of it.
2. **A consumer** — examples or tests that import from here, so a break in the
   library is a break in CI. `check-examples.sh` is the model: it pins the
   printed answer, not just the exit status.
3. **Then** port modules one at a time, removing each from the sketch list in
   `scripts/check-mg-sources.sh` as it starts typechecking. The list is designed
   to only shrink.

Until step 1 is decided, adding MAGE files here would be adding to the problem.

## What the files are useful for today

Reading, with the understanding that they are Rust. They record an intended
shape for `io`, `fs`, `net`, `col`, `json`, `math`, `time`, `agent`, `llm`,
`kb`, `evolve`, `neural`, `rl`, `tensor` and the rest — which surfaces someone
thought a MAGE standard library should have, and roughly what each should
expose. That is a real design artifact. It is just not source code.
