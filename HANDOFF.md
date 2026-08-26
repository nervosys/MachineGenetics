# Handoff — through 2026-08-25

What this repository is, what state it is in, and what to do next. Written for
someone picking it up cold.

---

## Where things stand

`master` is green and released. Everything below is verified, not asserted —
each claim has a command beside it.

| | |
|---|---|
| Tests | **2,915** — rmi 1,384 · prototype 1,202 · ribosome 164 · germline 112 · forge 53 |
| CUDA | **1,229 passing** on dual RTX 3090 Ti, driver 610.88 |
| Warnings | 0 compiler, 0 clippy in the four owned crates (`rmi` keeps 2 — vendored) |
| Vulnerabilities | 0 Rust across five lockfiles, 0 npm — and the four *committed* lockfiles report 0 warnings too. Re-run 2026-08-25, and **no longer only a claim with a date on it**: `scripts/check-security-register.sh` now re-derives it in CI and compares the result against `SECURITY_AUDIT.md` §1's accepted-risk register in both directions. `master` still carries the `nanoid` npm advisory (Dependabot #18) — the fix exists only on `handoff` |
| CI | 10 jobs defined, **9 of which ever run** — green on `master` and on `handoff`. The tenth, `prototype — cuda kernels (self-hosted GPU)`, has no runner and has never executed (open item 2) |
| Reliability floors | file-oracle parse 99/100, perturbed pattern-heal 42, native-lexer ratio 0.997 |
| Examples | 12 of 12 typecheck, run, and print their recorded answer |
| `.mg` sources | **101 checked, 0 sketches** — every `.mg` file in the repository typechecks |
| Documentation | 205 MAGE blocks typecheck; 57 documentation entry points run; 275 `rmi/docs` API items all exist |
| Release | `v0.3.0`, with the promo video attached as a release asset |

Reproduce all of it:

```sh
scripts/test-all.sh --check-docs          # everything + documentation check
scripts/check-examples.sh                 # the 12 shipped examples, end to end
scripts/check-mg-sources.sh               # every .mg file in the repo
scripts/check-security-register.sh        # §1's accepted risks vs. what cargo audit reports
scripts/test-all.sh --cuda --bench --check-docs   # + GPU and the benchmark harnesses
```

**`handoff` is pushed and PR #6 is open against `master`, with CI green on
every commit.** It was not pushed when this document was first written; that was
open item 0, and it is closed.

It has **not** been merged, deliberately — `gh pr merge --auto` merges
*immediately* in this repository, because there are no required status checks,
which is how PR #4 once landed with CI still pending. So the merge is a human
action, and until someone takes it `master` keeps the `nanoid` advisory that
`handoff` fixes. That is the only item here with an outside clock on it.

---

## What changed on 2026-08-19 → 24, and what it cost

Eight commits. Every open item that could be closed from inside this repository
is closed; what remains is listed under [Open items](#open-items) with a reason
attached to each.

| Item | Outcome |
|---|---|
| 0 — unpushed branch | Pushed. PR #6 open, CI green, **not merged** (see above) |
| 1 — module system | **Declined.** One flat namespace, `MAGE_SPEC.md` §2.3 normative. `use` is now an error; `stdlib/` deleted, 26 files and 4,402 lines |
| 4 — RAP error shape | **Fixed.** Protocol failures use the JSON-RPC `error` member; program failures stay in `result` |
| 10 — multi-shot resumption | **Declined.** §11.6 normative. No `resume` keyword, no CPS rewrite |
| 14 — `token-bench` gating | **Fixed.** Shrink-only baseline; the status reports movement, not the size of a known set |
| 15 — `rmi` `cuda` feature | **Feature builds now.** `cuda = []`; cudarc was mandatory for a feature gating nothing that could run |
| 16 — `Relaxed` refcount | **Fixed, and it was unsound** — not "not demonstrated unsound", as recorded |
| 17 — `rmi` `wasm` feature | **Fixed** the day it was filed. Two lines |
| 18 — fabricated `DeviceInfo` | **New, and open.** The ontology half is fixed; the behaviour half is upstream's |

**Two decisions, not two implementations.** Items 1 and 10 were both filed as
real work, unstarted. Neither was: each was a design question nobody had
answered, and an undecided design is indistinguishable from an unimplemented one
from outside. Both closed by writing the answer down normatively and pinning it
with a test that fails when someone starts building the thing that was declined.
For item 10 that pin is `resume` remaining a usable identifier — reserving the
word is the first edit multi-shot needs, so it trips before the 39 expression
forms get touched.

**Two more were mis-framed the other way.** Items 15 and 17 were filed as
"vendored — owner's call", and that framing was repeated three times in this
session before anyone checked it. The real blockers were two lines and one
deleted dependency. Only `cuda_full.rs` genuinely belongs upstream. **Five of
the eight closures came from re-reading how an item was filed, not from writing
much code** — which is worth knowing before trusting any remaining row's
triage, including the ones below.

**Two new instruments**, both of which found something on their first run:

- `check-orphan-sources.sh` found `compute/wasm.rs` — 208 lines nothing
  compiled, whose backend the crate advertised to callers.
- `check-ignored-tests.sh` found `perf_report` — the harness behind the ABL
  scaling figures in `MEASUREMENTS.md`, compiled by every `cargo test` and
  executed by nothing.

Together they cover the two ways code sits in the tree without ever running: a
file no `mod` reaches, and a test no schedule names.

**Left undone, and known** — *done 2026-08-25, see below.* `SECURITY_AUDIT.md`
§1's accepted-risk register was still not compared against what `cargo audit`
actually reports, the gap that document names itself. It now is:
`scripts/check-security-register.sh`. And §1 said "four committed lockfiles"
while the CI job is named "all five" — the fifth is git-ignored and resolved
fresh, so the two describe different surfaces and neither said so. §1 now says
so.

---

## What changed on 2026-08-25

One commit, closing the item the section above left open.

`scripts/check-security-register.sh` compares `SECURITY_AUDIT.md` §1's
accepted-risk register against what `cargo audit` reports across all five
lockfile surfaces, plus `npm audit` on `video/`. It fails in both directions —
an advisory reported by some surface with no row in the table, **and** a row
marked **Accepted** that no surface reports any more — which is the shape the
register's own history demands, because it had drifted both ways at once.

**The register agrees today, and that is the finding.** All five surfaces were
re-run: four committed lockfiles report zero findings of any kind, and `paste`
(RUSTSEC-2024-0436, unmaintained) is reported only by `rmi`'s git-ignored
lockfile, exactly as §1 records. `npm audit` on `video/` is 0. Nothing had
changed since 2026-08-18 — but nothing had *established* that either, which is
the difference the script makes. The `Vulnerabilities` row above says to treat
that line as a claim with a date on it; it now has a mechanism instead.

Three things are worth carrying out of building it:

- **The "Accepted" direction only works over the union of all five surfaces.**
  `paste` is reported by exactly one — the git-ignored one — so a check that
  audited only the four committed lockfiles would have declared a correct row
  stale. The narrower scope is the more defensible-sounding one, and it would
  have been wrong.
- **A row the check cannot classify is a failure, not a pass.** If a Status
  column says none of FIXED / Accepted / "No longer applies", the script says
  so and exits 1. That is rule 8 applied at row granularity: a row outside the
  assertion's reach must not look like a row that passed.
- **The npm total is summed from the severity buckets, not read from
  `"total"`.** `npm audit --json` has two keys named `total` —
  `vulnerabilities.total` and `dependencies.total`, which is 293 here — and
  telling them apart by document order is a parse that reads the package count
  as a vulnerability count on the day that order changes. Verified against
  synthetic reports with the blocks in both orders.

Verified by breaking it, five ways, each failing for its own reason: a reported
advisory deleted from the table, an Accepted row flipped to FIXED, an invented
Accepted row nothing reports, an unclassifiable Status column, and the `## 1.`
heading renamed. The npm arm reads 0 against the live surface no matter what
the table says, so no break test reaches it; its parser was tested separately
against synthetic reports, including the shape-changed case, which must yield
"cannot parse" rather than 0.

**The first version told a correct document it was wrong.** Testing the path
CI actually depends on — the git-ignored lockfile absent, because CI generates
it in a prior step — the check reported the missing surface *and then* reported
`paste`'s Accepted row as an acceptance that had stopped applying, advising
that it be struck through. Both messages were produced by one cause, and the
second sends someone to edit a row that was right. That is the trap already
recorded under Traps — **a checker must distinguish "wrong" from "not
checked", because the two have opposite remedies.** The "accepted but reported
nowhere" direction now runs only when every surface was audited, and says so
when it does not. The other direction — "recorded as fixed and reported again"
— still runs on a partial set, because a finding that is there is there.

**And running it in CI corrected §1.** The row saying the `paste` warning is "a
property of a local resolve rather than of this repository" was the right
phrasing for `crossbeam-epoch 0.9.18`, a stale artifact in one working copy.
`paste` is not that: CI resolves `rmi`'s lockfile from scratch and reports the
identical single advisory, so a fresh consumer gets it too. It is git-ignored,
not local. The two look the same in a report, and only the fresh resolve tells
them apart — which is the argument for the CI job auditing five surfaces while
§1 counts four.

**A trap, paid for in this session.** The break-test harness restored the
document between cases with `git checkout -- SECURITY_AUDIT.md`, which also
reverted the *uncommitted documentation edits* made earlier in the same
session. `git checkout --` does not distinguish your experiment from your work.
Commit before running a harness that reverts, or point it at a copy.

### Then §2–§4, which nothing had ever checked

§1 now has an instrument. The rest of `SECURITY_AUDIT.md` does not, and its
claims are specific enough to check — which the taxonomy says is the one thread
worth pulling through code. Four results, three of them defects:

**§3's `unsafe` count was still wrong, after being corrected once.** It said
"the real surface is **9** `unsafe` blocks in `memory_pool.rs`". There are
**13**: 9 blocks plus **4 `unsafe impl Send`/`Sync`** on `Slab` and
`TensorBuffer`. The omitted four are the higher-risk half — a block's soundness
is local, an `unsafe impl` is an unchecked global assertion — and they are
exactly where the fifth defect was. This is the same row that previously
asserted the allocator did not exist at all. **Corrected twice now, understated
both times, and both times the omitted part was where a real bug lived.**

**§5 described a fixed data race as an open question.** It called the four
`unsafe impl` "defensible under the refcount discipline", said reading the
counter `Relaxed` while gating mutation was "a synchronisation decision rather
than a counter — worth a second opinion from someone who owns this code", and
left it there. That hedge is how it survived: item 16 wrote the interleaving
down and it *is* a data race on the ordinary sharing path, fixed 2026-08-19 by
making the load `Acquire` (`memory_pool.rs:516`, verified). A security document
saying "possibly fine, someone should look" about a defect that was found and
fixed six days earlier is decay in the direction that reads as caution.

**§2's "no TLS" claim is true, and worth recording as checked.** `--rap` really
is plaintext JSON-RPC over TCP; the `rustls` implementation behind
`--features tls` is `ribosome`'s *worker transport* (`ribosome/src/tls.rs`),
not RAP, so it does not contradict §2 — which it looks like it might, given
open item 3. A negative result, recorded so nobody re-derives it.

**The inventory is now pinned.** `test-all.sh` emits `unsafe_memory_pool`,
`unsafe_cuda_full` and `unsafe_cuda_backend` from the same expression §3 tells
a reader to run, and four `CHECKS` rows compare them against the two documents
that state them — pinning both of `cuda_full.rs`'s mentions to one key, as the
rule above requires. Verified in both directions: a measured value that
disagrees is reported as a mismatch, and a reworded claim is reported as a
pattern that no longer matches. Documented-count pins are now **84**.

Confirmed by the same sweep, so it is measured rather than assumed: **the only
Rust `unsafe` anywhere in the owned crates** is 3 blocks in
`prototype/src/cuda_backend.rs`, 13 in `memory_pool.rs` and 16 in the
never-compiled `cuda_full.rs`. `forge`, `ribosome` and `germline` have none,
and neither do rmi's benches or examples. Everything else matching `unsafe` in
`prototype/src` — 74 hits across 16 files — is the *MAGE keyword*, not Rust.
§3 named three files as keyword-only hits; there are sixteen.

### And §2–§4, where the guards worked and nobody could tell

Five more results, one of them a fix.

**Both ABL container decoders exited 0 on every corrupt container.**
`--from=abl-bytes` and `--run=abl-bytes` hand-roll the same `take()`-guarded
decoder, and §3 cites it as evidence that decode is bounds-validated. It is:
eight malformed containers — truncated header, bad magic, bad version, a
four-billion item count, and `name_len`/`expr_len` at the `u32` ceiling — each
produce a clean diagnostic and **no panic, abort or hang**, through both entry
points. That claim is now measured rather than read.

But every one of those eight **exited 0**, while the same `match` arm exits 1
when the file merely cannot be read. A caller driving the tool by exit status —
which is how an agent drives it — could not distinguish a decoded container
from a rejected one. **This is "detected, recorded, and never surfaced"**, the
same shape as the unknown net layer lowered to `Op::IDENTITY`: the guard fires,
prints, and returns success. Fixed, and pinned by
`prototype/tests/abl_container_exit_status.rs`, the repository's first
integration test that spawns the binary — because the failure *is* a process
exit status and `std::process::exit` cannot be observed from inside the
binary's own harness. Verified by reverting the fix and watching the corrupt
case fail. The test asserts both directions: a valid container must still exit
0, which is what catches an over-eager fix that makes the decoder always fail.

**A near-finding, recorded as a non-finding.** `take`'s bound is
`*pos + n > buf.len()`, an unchecked add, and the identical shape had already
been a real defect in rmi (`TensorBuffer::slice`, fixed with `checked_add`).
Here it is not reachable: `n` comes from a `u32` field and `pos` from the
buffer, so it cannot overflow a 64-bit `usize`, and on a 32-bit target it would
wrap to a panicking slice range rather than an out-of-bounds read. **Written
down as a parenthetical rather than reported as a defect** — the taxonomy's
"reproduce before believing" applies to one's own pattern-matching too.

**§4's RA row still recommended what had been done twenty days earlier** —
"Recommend wiring `cargo audit`/`cargo deny` into CI as a gate (the one
concrete CI action item)", wired 2026-08-05 — and the "Open recommendations"
section on the same page already said so. Two parts of one document
disagreeing, neither noticed.

**§4's CM row claimed `Cargo.lock` committed → reproducible builds ✅**, where
`git ls-files '*Cargo.lock'` returns **four**. The fifth is git-ignored by
design. Same four-versus-five distinction as §1, unqualified in both places
until now.

**"Confirmed zero secret/credential material (leak scan)" named no tool, no
corpus and no date.** Re-run over 559 tracked files against high-signal issuer
patterns — still zero — and the line now says what it does *not* cover: no
entropy analysis, no history scan, and no `gitleaks`/`trufflehog` installed
here. A vague claim is unfalsifiable and therefore safe to ignore, which is the
worst thing a security assertion can be.

**Two §3 claims verified true and left alone**: the RAP non-loopback refusal
(rc=2, and the `MAGE_RAP_ALLOW_REMOTE=1` override warns then binds — both
halves run, where the row had only ever exercised one), and the subprocess
backend passing argv rather than a shell string (`Command::new(prog).args(…)`,
the only two `Command::new` sites in `prototype/src`). Negative results are
worth recording so the next person does not re-derive them.

### §2 said there were no signatures. There are two signature schemes.

The largest single find of the session, and the one that best explains why the
rest of this document exists.

`SECURITY_AUDIT.md` §2 — the FIPS/cryptographic-posture section — inventoried
three primitives (SHA-256 for content-addressing, xxHash, an LCG) and concluded
**"No secret keying, no signatures, no KDF"**, and from that, that the FIPS gap
"affects compliance posture, not present-day confidentiality."

What the repository actually contains:

| | Where |
|---|---|
| **HMAC-SHA256 under a fleet secret key**, hand-rolled (RFC 2104) | `ribosome/src/mac.rs`, used by **three** call sites |
| **Ed25519 signatures** over build provenance, per-worker private seeds, `TrustStore` with revocation | `ribosome/src/provenance.rs` |
| **Constant-time comparison** | `ribosome/src/mac.rs` |
| **Challenge-response worker authentication** | `ribosome/src/remote.rs` |
| **TLS 1.3** (`rustls` + `ring`) behind `--features tls` | `ribosome/src/tls.rs` |

There is secret key material in this system, and the section whose job is to
inventory cryptography listed none of it.

**The code is not the problem, and saying so matters.** The hand-rolled HMAC is
verified against **RFC 4231 cases 1–3**, long-key case included — a claim its
own module comment makes, which I checked because it is specific enough to
check. Comparison is constant-time. `provenance.rs` states its trust model at
the top, including the limitation that HMAC is symmetric so any verifier can
also mint, and names asymmetric keys as the fix — then implements them. This is
careful work. **The defect is entirely in the document.**

**Why it happened is the mechanism, not the oversight.** The **Scope** line at
the top of `SECURITY_AUDIT.md` named three surfaces: `rmi`, `prototype`,
`agentic-eval`. `ribosome` and `germline` were extracted from `forge` later
(§1's own note, steps 148–149). §1 was widened to audit five lockfiles. §2's
scope never moved. Every cryptographic mechanism in the repository arrived
inside crates that the crypto section did not consider itself to cover.

**An absence claim cannot fail loudly.** "There is no X" stays green by doing
nothing, and a scope change expires it silently — with no diff, no failing
test, and no reader able to tell. That is the worst property a security finding
can have, and it is the reason this one survived while numeric claims in the
same document were caught by pins.

`scripts/check-crypto-inventory.sh` now fails when a crypto dependency in any
`Cargo.toml` is not named in §2, when §2's inventory **table** names a crate no
lockfile contains, and when a **hand-rolled** primitive exists that §2 never
mentions — the last because `mac.rs`'s HMAC has no manifest entry and is
invisible to every dependency scan, which is exactly how it went unlisted.
Verified by breaking all five cases, plus a sixth to isolate the second
direction, which the obvious break did not reach.

**Its first draft cried wolf four times on a correct document**, and fixing
that is the more useful lesson. Comparing every crypto name appearing anywhere
in §2 against the *direct* dependencies reported `ring` (real, reached through
`rustls`'s feature rather than declared), `ed25519` (real, pulled in by
`ed25519-dalek`), `signature` (a real crate *and* an ordinary English word in a
section about signatures), and `aws-lc-rs` (the FIPS migration target §2
recommends — a dependency the repository deliberately does **not** have). Four
failures, all wrong. Per rule 10 that is worse than no check: an instrument
that cries wolf converts "unknown" into "passing" as soon as people learn to
skip it. The fix was to narrow what the check treats as a *claim* — the table's
Crate column is where the document asserts a dependency; prose is where it
discusses one — and to compare against the **lockfiles** rather than the
manifests, so a transitive or feature-gated crate is not called missing.

Two smaller things fell out of the same reading: §2's "no transport encryption"
was true of RAP and false of the repository, and `mac.rs`'s own doc comment said
"two subsystems" where there are three — the third being the worker handshake,
a different *use* of the key, which is precisely what an auditor tracing key
material needs the list for.

### Then the keys, which §2 had never mentioned because it thought there were none

Having found the cryptography, the next question is the one a posture section
exists to answer: **where do the keys come from?** Four results, and the
verification story is the interesting part — the code that handles keys is
better than the code that accepts them.

**There is no production key source.** Every `Signer::new`, every
`AsymmetricSigner::from_seed`, every `auth_key` in the repository is **in a
test**. No environment variable, config field or CLI flag provisions a key.
Defensible for a library — the fleet key is the embedder's — but it means there
is no reference path and no guidance, which is why none of the below was
written down anywhere.

**An empty key is accepted and verifies.** Measured with a throwaway probe
rather than inferred from the constructor: `Signer::new("w", Vec::new())` signs,
the record verifies, and an independent signer holding the same empty key forges
successfully. A one-byte key likewise. **The reason this matters is that an
empty `Vec` is what an unset environment variable becomes** — a fleet whose key
was never provisioned would authenticate every claim and report success while
providing nothing, and no verifier could tell, because a MAC over an empty key
is a well-formed MAC. This is the `guard`-fall-through shape in a security
primitive: the system says yes and means nothing.

**Left as an owner decision, deliberately.** Refusing a weak key means
`Signer::new` returns a `Result`, a breaking change to a public API of the
build engine. Having already changed one CLI contract this session, making a
second unilateral API change on a security primitive is not mine to make. The
constructor's rustdoc now warns with the measured behaviour, `SECURITY_AUDIT.md`
§2 records it, and the recommendation names it first because the failure mode is
silent success. **This is the "waiting on a decision, not on work" category, and
it is worth stating why rather than filing it blank** — the last time an item
sat here framed as a design question, it turned out to be a defect nobody had
run the check on.

**The two schemes have opposite rotation stories, and only one was written
down.** Ed25519 rotates properly — per-worker keys, a `TrustStore`, and `revoke`
that *records* a revocation rather than deleting the key, so a rejoining
compromised node cannot re-trust itself by re-announcing. `verify` fails closed
on revoked, unknown worker, substituted key, wrong subject and bad signature; I
read every branch. The **HMAC path has no rotation at all**: one key per
`Signer`, one `auth_key` per server, no key id, no overlapping acceptance
window. Changing the fleet secret requires every worker and verifier to change
at once — the hardest operation to perform safely is the one with no support.

**And one thing recorded as an assumption rather than a defect.**
`remote::next_nonce` is `HMAC(b"ribosome-nonce", clock ‖ counter)` under a
**constant, public** key, with a comment arguing unpredictability is not
required "because the secret is the key". That holds against replay; it is
weaker against a pre-play attacker who predicts the next nonce and induces a
legitimate client to answer it first, which needs a rogue endpoint and a guess
at the issuing nanosecond, and is moot under the `tls` transport. Whether that
matters is a threat-model question, and **the reasoning in the code is explicit
rather than accidental** — which is the difference between an assumption to
confirm and a bug to fix. Recorded as the former. Six times this document has
warned that a comment agreeing with its author is not evidence; the honest
answer here is that the comment states a real trade-off and the owner is the one
who knows the deployment.

### The lesson had been learned, written down, and half-applied

Found by accident, which is how most of these are found: after the full suite
ran, `git status` showed `benchmarks/RELIABILITY_REPORT.md` **modified**, and
nothing had edited it.

`check-ci-floors.sh` runs `reliability-bench` twice to measure the parse and
heal floors, and that binary **writes `RELIABILITY_REPORT.md` every time**. The
same script already save-and-restores `TOKEN_REPORT.md` around the token bench,
under a comment explaining exactly why — *a check that rewrites a tracked file
is not a check either*, rule 6 above. **The rule was applied to one artifact of
that script and not to the other.**

The consequence is worse for this one, because the file embeds p50/p95/p99
**latencies**. Running the suite on a machine already busy with other runs
rewrote them 30/247/348 → **51/551/1089 µs**, and a `git add -A` would have
committed load-dependent timing noise into the repository as though it were a
measurement. Which is very nearly what happened here.

Now restored on every exit path via a `trap`, including a floor breach — a
check that fails must not leave the tree rewritten either. **Restored, not
compared**: `TOKEN_REPORT.md` earns a staleness check because it is
byte-stable, and this one deliberately does not, because a latency table would
be permanently red. That is the same distinction rule 10 draws — whether the
artifact records a measurement or a timing — and it is why the two artifacts
need *different* treatment rather than the same treatment.

Worth stating plainly, because it generalises: **an instrument with a side
effect on tracked state is a defect even when its verdict is correct**, and the
side effect is invisible in the instrument's own output. The only thing that
surfaced it was `git status` on an unrelated commit.

**A trap, and a near miss.** Inserting the `CHECKS` rows with
`awk -v new="$ROWS"` silently stripped every backslash, turning
`\*\*[0-9,]+` into `**[0-9,]+` — a different and invalid regex, in a
tab-separated table where a wrong field also fails silently. **`awk -v`
processes escape sequences in the value.** Reverted and done in Python, which
also had to take the row terminator from the anchor line rather than assume
`\n`, because the working copy is CRLF.

**What the register check deliberately does not check** is the *rationale* in
the Status column. "Only under the non-default `gpu` feature" is a claim about a
dependency graph, and re-deriving it per row is a `cargo tree` reachability
argument with a false-positive class — rule 10 says a weak instrument is worse
than a documented gap, so the gap is documented in the script's header and in
§1.

---

## The one thing to understand

Five sessions have now found well over a hundred defects in this repository,
and **almost none was found by reading code**. Nearly every one came from
running something and comparing the result against what a document, a comment,
or a test claimed.

The code has consistently been in better shape than the claims about it. The
claims are what keep turning out to be wrong — and because this is a language
built to be *generated and consumed by agents*, a wrong claim is not a
documentation defect. It is a supply of confidently incorrect programs.

The one exception is worth its own note, because it inverts the rule.
Taxonomy §7 collects four **memory-safety defects** in `rmi`'s allocator, two
of them undefined behaviour on an ordinary code path. Those were found by
reading code — but only because a security document claimed that code had been
reviewed, and claimed it specifically enough to check. **The claim was the
thread; the reading was pulling it.**

There is a second pattern under the first, visible only in aggregate. A single
mechanical edit — a global rename of `Rmi` to `RecursiveMachineIntelligence` —
produced *four* separate defects found in four different places over two days:
the `ABL1` magic bytes in the container's published format, the decoder's own
error message, twelve `framewerx::` module paths, and the `RmiError` type name
in an API reference. Each looked like an isolated typo. **A rename that touches
literals is one defect wearing many costumes, and finding one is a reason to go
looking for the rest.** By the end of the sweep it had produced **five**: the
magic bytes, the decoder's error message, twelve module paths, a type name, and
a shell variable that turned into a command. Four were documentation; the fifth
was a script that had been broken and exiting 0 since the edit. The same held for name-versus-id confusion in the
`rmi` docs, which turned up in six places across three files.

[Failure taxonomy](#failure-taxonomy) is the useful part of this document. It
groups what has gone wrong by *shape*, because the shapes repeat, and knowing
them tells you where to look next.

---

## The instruments now in place

These exist because the same failure kept recurring: a claim written once, never
revisited, and quietly wrong. Each fails in **both** directions — a claim that
breaks, *and* a recorded state that silently starts passing.

Four of them carry a **baseline** that can only shrink, which is the right shape
when the backlog is real and the fix is incremental:

| Baseline | Holds | State |
|---|---|---|
| `scripts/doc-blocks-baseline.txt` | MAGE blocks that fail `--check` | **0** — fails on one new violation |
| `scripts/rmi-api-doc-baseline.txt` | documented API items that do not exist | **0** — same |
| `scripts/orphan-sources-baseline.txt` | `.rs` files no `mod` reaches | **1** — `cuda_full.rs`, needs a CUDA toolkit to wire up |
| `benchmarks/token-claims-baseline.txt` | corpus `token_count` claims that disagree with measurement | **150** — the subject of `benchmarks/FINDINGS.md` §1, a finding rather than a regression |

Every one of them fails in both directions: a new entry fails, and so does an
entry that has silently stopped applying and should have been deleted. A
baseline nobody is required to shrink is just a list.

| Command | Checks |
|---|---|
| `scripts/test-all.sh --check-docs` | every documented test count against the run that just produced them; `--cuda --bench` adds the GPU and benchmark figures |
| `scripts/check-examples.sh` | that all 12 examples typecheck, evaluate, **and print the recorded answer** |
| `scripts/check-mg-sources.sh` | every `.mg` file in the repo typechecks, or is a listed sketch with a stated reason |
| `scripts/check-doc-blocks.sh` | that **every** MAGE block in every markdown file passes `--check` — the baseline is empty, so any new failing block fails the check |
| `scripts/check-doc-evals.sh` | that every documentation block defining `main` or a `@test` **runs** — 57 entry points, the second oracle |
| `scripts/check-vocabulary.sh` | that all 31 words of §8 check, run, and return what the ontology publishes — and that its own case list *is* the published vocabulary |
| `scripts/check-ci-floors.sh` | the three measurable published floors, measured fresh; also fails if the committed `TOKEN_REPORT.md` differs from a new run |
| `scripts/check-rmi-api-doc.sh` | every item `rmi/docs/*.md` documents exists in the crate, and every `**Module:**` path resolves — baseline **0**, so any invented name fails |
| `scripts/check-orphan-sources.sh` | that every `.rs` file is reachable by some `mod` declaration — i.e. that it compiles at all. Every other row asks a question *about* compiled code; this one asks whether there is any. Found two on its first run. `wasm.rs` turned out to compile clean once a `mod` declaration was added, so it is fixed and the baseline shrank to one: `cuda_full.rs` (1,812 lines, 16 `unsafe`, 6 tests that have never run), which cannot be wired up without a CUDA toolkit. |
| `scripts/check-ignored-tests.sh` | that every `#[ignore]`d test is named by some CI job — i.e. that something, somewhere, runs it. The sibling of the row above: that one finds files nothing compiles, this one finds tests nothing schedules. `eval_bench` was red for 76 commits behind three hiding places at once (`#[ignore]`, a `--bench` flag nobody passed, an unpushed branch) while the figure it certifies appeared in three documents. Its first run found `perf_report`, which produces the ABL scaling numbers and had never been executed by CI. No baseline: the live set is 2 and both are covered, so green is the honest state. |
| `scripts/check-crypto-inventory.sh` | that `SECURITY_AUDIT.md` §2 names every crypto dependency in every `Cargo.toml`, that its inventory *table* names only crates some lockfile contains, and that every **hand-rolled** primitive is mentioned. §2 concluded "no secret keying, no signatures, no KDF" while the repository held HMAC-SHA256 under a fleet key, Ed25519 provenance signatures and a TLS transport — all of it arriving with crates added after §2's scope was written. **An absence claim stays green by doing nothing**, which is why it needed a check that does something. |
| `scripts/check-security-register.sh` | that `SECURITY_AUDIT.md` §1's accepted-risk register agrees with what `cargo audit` reports across all five surfaces, and that `video/` is at zero. Both directions: a reported advisory with no row, and an **Accepted** row nothing reports any more. The rows are read *out of* §1 rather than copied into the script, so adding a row is what tells the check the row exists. The `audit` job below finds advisories; this reads what was *decided* about the ones that stay — which nothing did, which is how §1 came to list `RUSTSEC-2026-0097` after it stopped firing while missing `RUSTSEC-2026-0190` for two months. No baseline: every row currently agrees, so green is the honest state. |
| CI `audit` job | `cargo audit` over all five lockfiles separately, plus `npm audit` on `video/` — which nothing covered until a high-severity advisory was sitting in it |
| CI ontology step | `MAGE_ONTOLOGY.json` matches a fresh `--emit-ontology` |
| CI version step | `mage-parse --version` matches the tool id Ribosome keys on |
| CI dependency guard | `ribosome` depends on no MAGE crate, no `germline`, and no TLS stack by default |

Plus, in the suite: every keyword introduces the token it claims, every
published type can be written in a signature, every published control sigil
parses, every published path exists, every capability namespace performs its
effect, every layer surface name compiles, every published RAP method has a
working call, every heal pattern matches an example and returns a fix, every
opcode the compute dispatcher handles is in its doc table, every sigil the
quick reference teaches is discoverable from the ontology, and a decoder
following the published container format consumes every byte of a real
container, and all five descriptions of that format agree on the magic, the
version and the symbol section, and a **corrupt** container exits non-zero
through both container entry points while a valid one still exits zero
(`prototype/tests/abl_container_exit_status.rs` — a process-level test, because
the thing that was wrong was a process exit status).

**If you add a measured claim to a document, add it to `CHECKS` in
`scripts/check-doc-counts.sh` in the same commit.** And if two documents state
the same measured figure, pin **both** rows to the one measured key — that is
what caught `ribosome`'s dependency count, where `ARCHITECTURE.md` and
`RIBOSOME.md` had agreed on 39 for as long as either had existed. The `.mg` source counts
(`101 checked, 25 listed sketches` at the time) were wired in this way, and the pin caught
its own extraction bug on the first run — the measured value came out as
`files;` because the awk field index was off by one. The one figure that
stayed stale after the checker existed was one nobody had listed.

### What the instruments taught, the hard way

Ten rules, each one paid for. The heading here said "four" for a while, which
was true when it was written and wrong for a long time after — the same defect
this section is about, in the section about it. The first three are one family:
a check can agree with its subject, agree with its own generator, or agree with
a weaker property than the one that matters, and all three read as green.

#### 1. A pin guarantees agreement, not truth

CI checked `MAGE_ONTOLOGY.json` byte-for-byte against a fresh
`--emit-ontology` — and thereby guaranteed that a *wrong* answer stayed
identical. Four of the ten effect names it published were rejected by the
compiler. The check was working perfectly and proving nothing. What was missing
crosses the boundary: not "does the file match its generator" but "does every
name the file publishes actually work".

#### 2. A pin over a generated list tends toward tautology

The same failure one level down. The follow-up test for layer names iterated
`layer_map`, which is *filtered* by the same function the validation uses — so
it could not fail by construction. **The escape is inputs the generator has
never seen**: catching `Lienar` requires a name no list contains. Rule 1 is
about a file agreeing with its generator; this is about a *test* drawing its
subjects from the thing under test.

#### 3. A weaker criterion reports success early

`check-doc-blocks.sh` counted parse errors, because that was the failure that
prompted it. Blocks that parsed and then failed the *checker* scored as passes
— 43 of them, including some in files the script had been used to certify as
fixed. **The criterion a ratchet enforces is the definition of done it hands to
whoever comes next.**

#### 4. A fixed list under a universal doc comment

The commonest failure of all. Six separate tests claimed a cross-boundary
property and asserted a hardcoded subset:

| The test said | What it checked |
|---|---|
| "every CLI flag the binary accepts must be in the ontology" | eight named flags were present — 12 were missing, including `--eval` |
| "every published RAP method dispatches" | each method called with `{}` and not "unknown" — the *contract* was wrong for a third of them |
| `examples_all_parse` | that the published examples parse, not that they check |
| `every_framewerx_module_path_exists` | 256 paths exist, not that the 243 names they publish are in them |
| `ci_floors` (published as "read from ci.yml") | nothing at all: the workflow had none of them |
| `heal_patterns_section_nonempty` | that a list of 34 is not empty |

Each passed for years. **If the doc comment says "every" and the body has a
literal list, the test is weaker than its own claim.** Every replacement
compares both directions and was verified by breaking it.

Then I wrote a seventh. The replacement for row two —
`every_published_rap_key_is_real`, whose whole point was that the old test
checked the wrong thing — was a hand-written list of working calls under a doc
comment saying "every published parameter is read". It exercised **31 of 37**
methods and asserted nothing about the other six. **Knowing the pattern by name
is not enough: the coverage assertion has to live in the same function**, or
the next person to add a method gets no signal. Fixing that immediately found
five methods whose contracts had never been checked, and two — `sandbox/policy`
and `manifest/generate` — that read a `source` parameter the ontology did not
publish, so *every* call an agent could construct returned "agent `X` not
found" for agents that were right there in the program it never received.

A sweep for the shape (grep `#[test]` whose doc comment says "every"/"all" over
a body with a literal list) turned up 13 candidates and three more holes:

- `every_typed_vocabulary_name_checks_its_arity` listed 29 of 31 published
  words, and the two omitted — `scan`, `group` — were the only two with **no
  typed arm at all**, so `scan(1, 2, 3, 4, 5)` typechecked clean.
- `every_effect_documented_in_the_spec_parses_and_checks` kept its own copy of
  §11.2's rows, which is the one boundary it exists to guard. It now reads the
  table out of `MAGE_SPEC.md`.
- `the_builtin_names_attributed_on_call_are_the_documented_ones` covered 36 of
  §11.2's 47 domain names; the eleven omitted included `read`, `listen`,
  `send`, `open`, `alloc` and `sleep`, all of which attribute an effect.

**A test that names its own subjects cannot report the subject it never names.**

#### 5. A both-directions test can still be vacuous

The guide↔ontology check was written first as "every token
`syntax-quick-ref.md` teaches appears in *some* ontology section". It passed
with `xd` deleted from the sigil table — because `xd` is also a lexer keyword,
and the union of every section made the check nearly unfalsifiable. It now
tests the guide's Declarations fence against `sigils` specifically, which is
the narrow claim that can fail, and keeps the union for the broad one.
**Comparing both directions is not enough if one side is wide enough to swallow
anything.**

#### 6. When a tool both measures and writes, comparing its output to its own artifact proves nothing

`check-ci-floors.sh` enforced the published token-ratio ceiling by reading the
`**Total**` row of the *committed* `TOKEN_REPORT.md`. That file is generated by
`token-bench`, **nothing ran `token-bench`**, and it had gone stale. My first
fix ran the bench and compared the file to the bench's stdout — but the bench
*writes* that file, so the two agreed by construction and it passed on a
deliberately stale report. The working version saves the committed copy, runs
the bench, compares the files, and puts the original back, because a check that
rewrites a tracked file is not a check either.

#### 7. A harness can change meaning under a table it produced

`MEASUREMENTS.md` gave ABL artifact scaling as 78 / 234 / 858 / 3354 bytes at
2 / 8 / 32 / 128 layers. Re-running `perf_report` prints **67 bytes four
times**. Neither is wrong: the harness builds *identical* layers, v3's REPEAT
fold collapses them, and the table was measured before the fold existed — it is
still exact for layers that do not repeat. **The natural response to that
disagreement is to "correct" a true table into a misleading one**, because the
document looks stale and the tool looks authoritative. The harness now measures
both cases and labels them.

#### 8. A file the instrument cannot see reads exactly like a file with nothing wrong

`check-rmi-api-doc.sh` reported "0 documented items" for `rmi/docs/ontology.md`
and passed. The file has no `pub` items — it is prose, tables and ASCII
taxonomies. Checked by hand, all three of its enumerations were wrong: six of
eleven documented relations do not exist, the domain set is ten variants rather
than the five headings it uses, and its property table named `name` as the
unique identifier when it is `id: Uuid`. **A passing result on a subject
outside the assertion's reach says nothing either way**, and looks identical to
success.

The extreme case of that is a file no compiler reads, and on 2026-08-19 it got
an instrument: `check-orphan-sources.sh` asks whether each `.rs` file is
reachable by any `mod` declaration. Every other check in `scripts/` asks a
question *about* code that compiles; none asked whether it compiles. It reports
`cuda_full.rs` — 1,812 lines, 16 `unsafe` blocks, 6 tests that have never run
anywhere — and, on its first run, a second file nobody had mentioned:
`compute/wasm.rs`, whose backend `core/discoverability.rs` **advertises to
callers**. That one compiled clean the moment a `mod` declaration was added:
the file was never broken, only unreferenced, which is exactly why nothing had
ever reported it. It is fixed, and the baseline is down to one. The instrument is trustworthy here for a measured reason rather than
an assumed one: across six crates its first run reported exactly those two and
no false positives, and the `#[path = "..."]` attribute that would defeat it
appears nowhere in the repository. Both conditions are written into the script,
because the day either stops holding is the day it starts lying. (It reports one
now — `wasm.rs` was fixed the same day, and the baseline shrank with it.)

Its sibling is `check-ignored-tests.sh`, added 2026-08-24, and the pair covers
the two ways code can sit in the tree without ever running: a file no `mod`
reaches, and a test no schedule names. The second found `perf_report`
immediately — the harness behind the ABL scaling figures in `MEASUREMENTS.md`,
compiled by every `cargo test` and executed by nothing. `cargo test` prints such
a test as "ignored", which reads like a decision somebody is making on each run.
Nobody is deciding anything.

#### 9. Agreement between two documents is not corroboration when one is a copy

`ARCHITECTURE.md` and `RIBOSOME.md` both cite `ribosome`'s transitive
dependency count as evidence that the build engine privileges no language. Both
said **39**; it is **28** unique crate names on normal edges (34 counting six
that resolve at two versions), and neither said which it meant. Both rows now
pin to one measured key that `test-all.sh` emits via `cargo tree`.

#### 10. A weak instrument is worse than a documented gap

The name-level API check cannot see a wrong signature on a function that
exists, so I measured that gap: of 162 documented functions, the 66 defined
exactly once are arity-checkable, and four disagreed. I fixed those and
**deliberately did not build an arity checker** — it would cover 41% and has a
false-positive class that tripped twice on my own placeholders while I was
writing the fixes. A check that is right 41% of the time and cries wolf
**converts "unknown" into "passing"**, which is the state every rule above
describes. The measurement is recorded instead, to be repeated by hand.

The same judgement applies in reverse: `benchmarks/RELIABILITY_REPORT.md`
deliberately has *no* freshness check, because it embeds p50/p95/p99 latencies
and would be permanently red. `TOKEN_REPORT.md` is byte-stable and therefore
checkable. **The distinction is whether the artifact records a measurement or a
timing.**

---

### Documents decay in both directions

Under-claims are defects too, and they are harder to notice because nobody is
disappointed by them.

`SECURITY_AUDIT.md` §3 said the non-loopback `--rap` refusal was "proposed, not
yet enforced". It has been enforced for some time — `rap::is_non_loopback`
exits 2 unless `MAGE_RAP_ALLOW_REMOTE=1`. A reader planning a deployment would
have built a guard that already exists, or avoided the tool over a gap it does
not have.

The same row's memory-safety claim was wrong in the other direction, and that
one mattered: "rmi has 1 audited `unsafe` in lib.rs, 3 in the CUDA FFI shim —
all reviewed, FFI-boundary only". `lib.rs` has none; the real surface is **9
`unsafe` blocks in `runtime/memory_pool.rs`**, an arena allocator compiled
unconditionally — not an FFI boundary, and the highest-risk category of
`unsafe` there is, asserted not to exist. Reviewing those nine found **four
defects**, two of them needing no unusual configuration (see taxonomy §7).
**The claim "reviewed" is not free**: this document made it on the code's
behalf, and the review it stood in for found undefined behaviour on a default
path.

An accepted-risk register decays the same way and worse. `SECURITY_AUDIT.md`
§1 named two accepted `cargo audit` warnings; re-running the five surfaces,
one had **stopped firing** and a different one had **started** — with a patched
version available, so accepting it was never right. CI's `audit` job correctly
does not fail on warnings, which means **nothing compares the accepted-risk
table against the warnings actually reported.** And "0 npm" was a claim about a
surface nothing checked at all: `video/` had one high-severity advisory.

The measurement documents had drifted furthest. `benchmarks/FINDINGS.md` §2
still says in the present tense that "three quarters of the corpus's own
reference solutions don't parse", with seven of ten categories at 0.0%;
measured today it is **parse 99/100, effective 100/100**. `AGENT_PROTOCOL.md`
carried nine stale figures including "~50×" in a document whose own worked
example three sections down measures 4.4×. In both cases **the outputs pasted
from a real run survived and the prose written by hand around them did not.**

---

## Open items

Grouped by what would actually unblock them, because "open" has meant three
different things.

### Decided on 2026-08-19

Three of the five rows below this heading were decisions rather than work, and
they are made. Recorded here rather than deleted, because "why is there no
module system" is a question that will be asked again.

| # | Was | Decision | What it cost |
|---|---|---|---|
| 0 | Unpushed commits on `handoff` | **Pushed**, not auto-merged. `gh pr merge --auto` merges *immediately* here — the repo has no required status checks — so the branch is up and CI runs against a PR that a human closes. | — |
| 1 | Module system | **There is none, and now the spec says so** (`MAGE_SPEC.md` §2.3). An import costs tokens and buys nothing when the library is small, fixed and global. `use` is an **error**, not a warning: a construct that can never mean anything should not typecheck. | `stdlib/` deleted — 26 files, 4,402 lines, of which 36 files never typechecked and nothing ever read. The corpus cost of the error, which was the stated reason to keep it a warning, was **one line in one example**. Every `.mg` file in the repository now typechecks. |
| 4 | RAP error shape | **Protocol failures moved to the JSON-RPC `error` member**; program failures stayed in `result`. Unknown method is -32601, a non-JSON frame -32700, a frame with no `method` -32600. | The wire change is real but small: the only consumer in the repo was the example client in `internals/07`, which reached straight for `["result"]`. A malformed frame is now *answered* rather than closing the connection, which it used to do silently. |

The distinction in that last row is the one worth keeping: **a MAGE program
that fails to check is a successful call.** `language/parse` returning
`{"ok": false, "error": {...}}` is the server answering the question it was
asked. Only "there was no call" belongs in `error`.

### Waiting on a decision, not on work

| # | Item | The decision |
|---|---|---|
| 2 | GPU CI runner | Correctness **is** verified on the hardware here and recorded. What is missing is a self-hosted runner so `cuda-gpu` runs unattended — an account action, declined once already. |
| 3 | TLS trust posture | The transport seam and a `rustls` implementation exist behind `--features tls`. Pinned self-signed / mutual TLS / public PKI is deliberately the operator's; `acceptor`/`connector` take your config. |
| 19 | **`Signer::new` accepts an empty HMAC key** | Measured 2026-08-25: it signs, the record verifies, and an unset environment variable is exactly how a caller gets there — so an unprovisioned fleet authenticates everything and reports success. Refusing it means returning a `Result` from a public API of the build engine, which is a breaking change and the owner's call. Rustdoc warns; `SECURITY_AUDIT.md` §2 records it and ranks it first of three key-management actions. **Not a "design question" filed against a defect** — the check has been run, the behaviour is measured, and only the API decision is open. |
| 20 | **The HMAC path cannot be rotated** | One key per `Signer`, one `auth_key` per `WorkerServer`, no key id, no overlapping acceptance window — so changing the fleet secret needs a simultaneous fleet-wide change. The Ed25519 path beside it rotates and revokes properly, which is what makes the gap visible. Adding a key id is a wire-format change; hence a decision, not a patch. |

### Deliberate, and not defects

| # | Item | Why it stays |
|---|---|---|
| 5 | `rmi`'s 2 clippy warnings | Vendored; must stay syncable against its own upstream. |
| 6 | Ab-initio migration steps 2c & 3 | Declined as negative-sum (ROADMAP step 99). Revisit only with new measurements. |
| 7 | Single-workspace build | The crates are separate workspaces by design; `rmi` must stay independent. |
| 8 | External dependency resolution | Fetching third-party code is a distinct trust problem — provenance, pinning, revocation. Folding it into the planner is how build systems become unauditable. |
| 9 | `json`, `kb`, `db` attribute no effect | No built-in kind names a store, and inventing a `Custom` would infer an effect that §11.4 then refuses in an annotation — leaving no way to declare what you perform. Declare `effect Db { … }`; `examples/effects-showcase` does. |

### The documentation, finished

**Every MAGE code block in every markdown file in this repository now parses
and typechecks.** `scripts/check-doc-blocks.sh` reads `0 (baseline 0)`, and
its baseline file is empty. It started at 177 of 258 failing.

Two kinds of block are skipped, both visible in the source: fragments
(containing `...`), and blocks whose label says they are broken or invalid.
`MAGE_SPEC.md` uses the second deliberately, to record five designs that do
not compile — `grad(…)`, `rl` blocks, the compile-time SKB query API, SIMD
types and the module system — each labelled as invalid where it appears.
That is the honest state: the spec still describes more language than exists,
but it now says which parts.

**Midway, the count went *up* because the criterion did.**
`check-doc-blocks.sh` counted *parse* errors only, and 43 blocks parsed and
then failed the checker — unresolved names, undeclared effects, type
mismatches — several of them in files the script had already been used to
certify as fixed. It now requires `Errors: 0`, which turned 82 remaining into
114. **An instrument that measures the weaker property reports success early.**

**The spec was the hardest, and the most valuable.** A disagreement between
`MAGE_SPEC.md` and the compiler has to be *resolved* rather than patched,
because either side may be the one that is wrong, and the answer differs per
construct. What the pass found, in the sections nobody had run:

| §  | The spec said | The compiler says |
|---|---|---|
| 5.1 | `layer dense(784, 256, relu)` | `layer name: Linear(784, 256)` — every layer is named, the kind comes from the layer map, and activations are layers |
| 5.4 | `model:`, `data:`, `fn on_epoch(…)` callbacks | `net:`, `dataset:`, 25 fields, no callbacks |
| 6.1 | `Tensor<f32, [3, 224, 224]>` | `tensor[f32, 3, 224, 224]` — lowercase, one bracket |
| 7.1 | `rule integer(T) :- numeric(T), !floating(T);` | `rule integer(t: str) { numeric(t) }` — no Prolog, no `query` item |
| 8.1 | `select tournament(k: 8), target fitness > 0.98` | `select { 8 }` — strategies are blocks, no `target`, no callbacks |
| 9.1 | `agent` with `brain:`, `memory:` and methods | two fields, both lists of identifiers, and no code at all |
| 9.2 | `dispatch` / `aggregate` / `on_failure` blocks | four fields; fan-out is `map`, fan-in is `fold` |
| 12.1 | `@req(…)` above the signature | a `spec` block sharing the function's name |

And five constructs the spec documented that do not exist at all: `grad(…)`,
`rl` blocks, `Capability`/`Region`, SIMD types, and the module system. Each is
now labelled **Invalid MAGE today** with the parse error it produces, rather
than deleted — the design intent is worth keeping, the false impression is
not.

Seven of those are worth knowing about, because the defect was not just syntax:

- **`effects.md` taught agents to under-declare.** Four places asserted an
  effect hierarchy — "CORRECT — net implies io", "CORRECT — agent implies
  async", "apply the hierarchy rule — don't list implied effects" — and marked
  the *under-declared* version correct. There is no hierarchy: `/ net` does not
  cover an inferred `io`, verified by making one function call another and
  watching the checker object. For a capability system, advice to omit an
  annotation is the worst direction to be wrong in.
- **Every "CORRECT:" block in `anti-patterns.md` was also wrong.** The file
  pairs a WRONG block with a CORRECT one, ten times; the WRONG halves are
  supposed to fail, and all ten corrections failed too. Its Anti-Pattern 10,
  "Mixing Rust Crate Paths with MAGE Stdlib", offered `use std::fs;` as the fix
  for `use tokio::fs;` — both Rust, and moot besides, since `use` brings
  nothing into scope.
- **Both system prompts stated false rules in prose.** The agent-guide one
  claimed the same effect hierarchy and listed `process` as a built-in effect
  (the kind is `proc`). **Nothing checks prose**, and in a system prompt a
  false rule outranks a broken example — a model follows the rule when
  generating code the examples do not cover.

- **`migration-guide/` documented a capability system that does not exist.**
  `[capabilities] grants = […]` in `Forge.toml`, a `Capability` type,
  `cap.require("fs.read")?`, `Capability.request(…, Lease.new(Duration…))` and
  a table of 14 capability strings (`net.http.get`, `mem.deref`, `ffi.call`).
  None of it exists — and it is not missing, because the real design puts the
  grant somewhere stronger: reaching the namespace *is* the request, the
  annotation is the grant, and the check happens before the program starts.
  A document describing a weaker runtime mechanism teaches an agent to look
  for a permission call that is not there, and to assume the annotation is
  documentation.
- **The whole `cookbook/` was written against a standard library that does
  not exist.** Eight files, 60 blocks: `u std.io.{File, BufReader, Read}`,
  `s.new()`, `[T]~.new()`, `TempFile`, `fs.watch`, `Command.new("git")`,
  `Mutex`, `channel`, `join_all`, `I Agent ~ Greeter`, `AgentRuntime`, `Bus`,
  `Capability.request(…, Lease.new(…))`. It is the most detailed
  documentation in the repository and none of it ran. Rewriting it against
  the capability namespaces found the `contains` divergence, the `async`
  namespace that never existed, and the fact that **no capability call could
  be evaluated at all**.
- **`quick-start/03-syntax-tour.md` taught the opposite of the central rule.**
  "The compiler tracks effects automatically — you don't need to annotate them
  unless you want to document intent." It requires them, at every public
  boundary, and that requirement *is* the capability gate. The same page taught
  `I Area ~ Shape`, `Point @{ x, y }`, `@ item ~ items`, `[1, 2, 3]~`,
  `{1, 2, 3}`, `+v PI` and `c f` — none of which parse. It is page three of the
  quick start.
- **`patterns.md` was twelve Rust patterns.** Ten did not parse; the two that
  did were still Rust. Two patterns had no MAGE referent at all —
  "Effect-Bounded Generics" (there is no effect polymorphism; `fn(str) -> T / io`
  does not parse) and "Module Organization" (there is no module system). They
  are now default arguments and one flat namespace. Rewriting it found the
  capability hole below.

- **`syntax-quick-ref.md` and `migration.md` were Rust cheat sheets.** These
  two are the densest documents an agent reads, and almost every row was
  false: `let`, `async fn`, `mod`, `use path::to::Item`, `foo::<i32>()`,
  `#[derive(Debug)]`, `println!`, `true`/`false`, `Foo { x: 1 }` as a struct
  literal, four `std::` module tables, and the effect hierarchy again.
  `migration.md` answered six of its eight worked migrations with "no changes
  needed — identical Rust and MAGE". **Both are now measured**: every row was
  run through `--check` before it was written down, and the ones that fail are
  listed as things that do not carry over.
- **`agent-guide/examples/` answered 26 prompts in Rust.** These are
  prompt → response pairs — training data, where the response *is* the answer a
  model learns to give. Only 19 of the 26 failed to parse; the rest were Rust
  the lenient parser happened to accept, which is the worse half. The advanced
  file taught an entire fictional agent API: `impl Agent for X`,
  `self.cap.request("net.http.get", …)`, `Swarm::new()`. Rewriting the three
  files found four compiler defects — the escaped-quote bug, `range`'s missing
  arity check, `len` committing an open type, and the method effect hole — all
  four by running the examples and checking the answers.

This was the shipped examples' story a third time, after `examples/` and
`prototype/examples/`, and by volume the largest instance — and the rate of
compiler bugs per document never dropped, across all three.
`scripts/check-doc-blocks.sh` and `check-doc-evals.sh` now hold the result.

### The vendored crate's documentation, and what it cost to find

`RecursiveMachineIntelligence/docs/` is four files and 3,246 lines. Nothing had
ever compiled a signature or resolved a path in any of them. All four are now
checked at name level by `scripts/check-rmi-api-doc.sh`, baseline **0**.

It began as one follow-up question — *does the vendored crate repeat
`SECURITY_AUDIT.md`'s "all reviewed" claim?* It does not, but answering it
opened `api.md`, and `api.md` was 12% fiction: **27 of 228 documented items
existed nowhere in the crate.**

| File | What was wrong |
|---|---|
| `api.md` (1,709 ln) | 27 invented items; an FFI section documenting `FfiValue`/`FfiFuncPtr` and an `unsafe call_unchecked` — none of which exist, and the real registry passes RMIL values to *safe* Rust closures; all twelve `**Module:**` paths named `framewerx::` for a crate called `rmi` |
| `protocol.md` (399 ln) | A wire format the code does not speak: magic `FRWX` where it is **`FWRX`**, `flags`/`message_type` at the wrong widths and in the wrong order, `MsgId`/`Timestamp` fields that are not in the header, and a CRC32 trailer where the real checksum is **XXH64 inside the header** over different bytes in a different order |
| `ontology.md` (477 ln) | Six of eleven relations invented, six real ones missing; ten domain variants presented as five; `name` called the unique identifier when it is `id: Uuid`. Its taxonomies hold **166 nodes against 28 shipped concepts, 12 in common** — a survey of the field, now labelled as one |
| `architecture.md` (661 ln) | One stale type name, caught by the checker on its first multi-file run |

Three things are worth carrying out of that:

**The same mistake, six times, in three files.** Every knowledge-facing type
was documented with **name-keyed lookups where the real API takes a `Uuid`** —
`Ontology`, `AIConceptsOntology`, `AIHistoryKB`, `CheckpointManager`, the hash
ring. That shape survives review because the call *reads* correctly:
`get_concept("attention")` is exactly what one would expect to write, and only
the type checker objects.

**Two entries would have produced working, wrong code** rather than a compile
error — `KeyValueStore` documented `with_compression` when the real builder is
`without_compression` (compression is on by default, so the reader enables what
is already enabled), and `ConsistentHashRing::new` documented a virtual-node
count where the parameter is a replication factor. Those are the ones a type
checker cannot catch.

**A wrong wire-format diagram is worse than a missing one.** Every error in
`protocol.md` is a silent interop failure at a byte offset, not a compile
error, and an implementer has no way to tell which of five discrepancies is
responsible for the checksum mismatch they are looking at.

Left undone, deliberately: the concept taxonomies are labelled aspirational
rather than regenerated from `populate_core_concepts`, and signatures are
checked only where a function is defined exactly once (66 of 162 — all now in
exact arity agreement). See rule 10 above for why no arity checker was built.

### Real work, unstarted

Two items, and **neither is actually unblocked** — which is the finding, not the
framing this section had. Both are downstream of a decision, and only one said
so.

| # | Item | Size |
|---|---|---|
| 10 | ~~**Multi-shot resumption for effect handlers**~~ | **Closed 2026-08-19 by deciding against it**, the same way item 1 closed. The row below said the implementation was downstream of a decision nobody had made; the decision is made. Single-shot is the language's answer, and `MAGE_SPEC.md` §11.6 now says so normatively instead of listing multi-shot under "what is missing". No evaluator rework, no `resume` keyword, no sigil. |
| 13 | ~~`stdlib/`~~ | **Closed 2026-08-19, by deletion.** Item 1 resolved as *no module system*, and a standard library reached by imports has no meaning without one. The 25 sketches — 4,402 lines of Rust behind a `.mg` extension, read by nothing — are gone. `scripts/check-mg-sources.sh` now reports **101 checked, 0 skipped**: every `.mg` file in the repository typechecks, and the sketch list it was built to shrink is empty. |

**Item 10, scoped.** Single-shot resumption is verified working, exactly as
§11.6 documents it — `handle { work() } with A { ask() => 7 }` gives 107, the
`ret` form gives 1007, and two operations in sequence each resume (5 + 5 = 10).
Measured, not assumed.

What is left has two parts, and the first is a **surface decision, not an
implementation task**:

- **Multi-shot needs the continuation reified as a value**, which means a
  `resume` keyword — and MAGE is dual-surface, so that is a keyword *and* a
  sigil, plus spec sections, ontology entries, and a rule for what effects a
  resumed continuation performs. The spec currently says "There is no `resume`
  keyword, because with single-shot tail resumption there is nothing for it to
  do", which is the correct reason today and stops being one the moment
  multi-shot lands. Someone has to design that.
- **Then the evaluator rework.** `eval.rs` is 2,987 lines, 39 expression forms
  and 53 recursive `self.eval(` sites, all of which keep the continuation in the
  Rust call stack — where it cannot be captured. Multi-shot needs CPS or an
  explicit CEK-style machine, which touches every form, with 1,202 tests riding
  on current behaviour.

**This item was filed under "real work, unstarted" with no blocker marked**,
beside `stdlib/` which is explicitly blocked on item 1. It had the same shape as
item 1, and it closed the same way.

**Decided 2026-08-19: single-shot is final.** Not deferred — declined. The two
costs above are the reason: a permanent per-arm token cost in a language whose
premise is that tokens are scarce, and a CPS/CEK rewrite of every expression
form to move a continuation out of the Rust call stack, in exchange for
generators and backtracking that no MAGE program has asked for. Handlers that
do the work MAGE exists for — state, reader, logging, tracing, retry,
capability interception, test mocking — need none of it and all work today.

Re-measured before deciding rather than trusting the 2026-08-18 numbers: the
three published results still hold exactly (`107`, `1007`, `5 + 5 = 10`), and
`resume` still evaluates as an ordinary identifier, which is what "there is no
`resume` keyword" has to mean to be falsifiable.

`MAGE_SPEC.md` §11.6 is now normative, and
`eval::tests::single_shot_resumption_is_a_decision_not_a_gap` pins both halves —
that `resume` remains a usable identifier, and that the spec still states the
decision. Reserving the word is the first edit multi-shot would require, so it
fails before anyone touches the 39 expression forms. If a real program ever
needs multi-shot, that section is the thing to change first, with the program in
hand.

### Small, sharp, cheap

Three, all left by this session and all recorded where they were found rather
than acted on:

| # | Item | Why it is here and not done |
|---|---|---|
| 14 | ~~**`token-bench`'s exit status cannot gate anything**~~ | **Closed 2026-08-19.** The row said correcting the 150 disagreeing claims "is a decision about what the field means, not a fix" — still true, and still undecided. But the exit status did not need that decision; it needed to stop reporting the *size* of a known set and start reporting *movement* against it. The known set is now `benchmarks/token-claims-baseline.txt`, shrink-only, the same pattern as `scripts/doc-blocks-baseline.txt`. A 151st disagreement fails; so does a baseline entry that has stopped disagreeing and should have been deleted. `check-ci-floors.sh` honours the status again — and dropping its `|| true` turned out to matter twice, because `set -o errexit` means a bare command substitution that fails kills the script *at the assignment*: my first version gated correctly and printed no reason, which is a worse instrument than the one it replaced. Exit 2 (the bench could not run at all) was being swallowed by the same `|| true` and now fails. |
| 15 | ~~**`rmi`'s `cuda` feature cannot build, and gates a stub if it could**~~ | **Partly closed 2026-08-19: the feature builds now.** `cuda = ["dep:cudarc"]` made cudarc 0.10 mandatory, and its build script wants `include/cuda.h` from an installed toolkit — so `cargo build --features cuda` failed before compiling a line of the crate. The only file using cudarc is `cuda_full.rs`, which no `mod` reaches, so the dependency was mandatory for a feature that gated nothing that could run. Verified no compiled file mentions cudarc, then changed it to `cuda = []`: the feature now builds, cudarc is out of the graph, and `--features cudarc` still reaches it for anyone wiring `cuda_full.rs` up. **What remains is genuinely the owner's call**: `cuda_full.rs` is still unreachable, with its 16 `unsafe` blocks and 6 `#[ignore]`d tests that have never run on any hardware, and deleting it or porting it to cudarc 0.19 is an upstream decision. It stays baselined in `scripts/orphan-sources-baseline.txt`. The **1,229 CUDA tests** verified on this hardware are all `prototype --features cuda`, the real path, and are unaffected. |
| 16 | ~~**`rmi`'s `Send`/`Sync` on refcounted buffers read the counter `Relaxed`**~~ | **Closed 2026-08-19, and it was unsound after all** — the row previously said "not demonstrated unsound", which was true only because nobody had written the interleaving down. `as_bytes_mut` is `Arc::get_mut` by hand, and it loaded the count `Relaxed`. Thread A reads through `as_bytes`, then drops its handle (`fetch_sub`, `Release`). Thread B's `Relaxed` load observes `1` **without** synchronizing-with that `Release`, so B's writes through the returned `&mut [u8]` are unordered against A's reads of the same bytes: a data race on the ordinary sharing path. Now `Acquire`, which is the ordering `Arc::get_mut` uses and for exactly this reason. The four `unsafe impl` are sound under that ordering. `refcount()` stays `Relaxed` and is now documented as advisory, since a statistic confers nothing. **No test here verifies this**, and none can: the fix is invisible to a single-threaded suite, and on x86 — where loads are acquire in hardware — the broken version would never have manifested either. `loom` would verify it and is not a dependency this vendored crate should gain on my say-so. Recorded rather than instrumented, per rule 9. |
| 17 | ~~**`rmi`'s `wasm` feature builds, pulls a dependency, and enables no code**~~ | **Closed 2026-08-19, the day it was filed.** `compute/wasm.rs` was 208 lines no `mod` declaration reached, so `--features wasm` succeeded, pulled `wasm-bindgen v0.2.126` into the graph, and enabled nothing — while `core/discoverability.rs` advertised a "WASM Backend" unconditionally. The fix was two lines: `#[cfg(feature = "wasm")] pub mod wasm;`. It compiles clean, `--features wasm` and the default build are both warning-free, and all 1,230 lib tests still pass. The file was never broken — only unreferenced, which is the whole reason it survived. Removed from the orphan baseline in the same commit, as that ratchet requires. |
| 18 | **`rmi` publishes five hardware backends that are CPU code, and they report invented device properties** | Found 2026-08-19 while checking whether the ontology should gate backends by feature — the answer turned out to matter less than what it was gating. `compute/metal.rs`, `vulkan.rs`, `webgpu.rs`, `apple_ane.rs` and `qualcomm.rs` carry no binding for the API they are named after, and `Cargo.toml` declares none: the only GPU crate in the manifest is optional `wgpu`. They are CPU implementations under hardware names, and each constructor comments what a real one *would* do while returning fabricated `DeviceInfo` — `metal.rs` announces itself as `"Metal (Apple GPU)"` with 16 GB of unified memory and 10 compute units; `vulkan.rs` reports 32. **The ontology half is fixed** (`add_extra_compute_backends` now publishes what the code does, plus an `implementation` field naming each as a cpu-scaffold, and the false `feature_flag: "gpu"` on Metal and Vulkan is gone — neither is gated at all). **What is left is a behaviour change in a vendored crate**: a caller that reads `DeviceInfo` to size a workload is told about memory and compute units that do not exist, and correcting that changes what every consumer sees. Options are to report the real CPU device, or to make construction fail when the API is absent — both are upstream's call. Worth ranking above items 15 and 17, because those cost a dependency and this one returns wrong numbers to anyone who asks. |

The five items previously here — `guard` as a reference, the `+=` diagnostic,
`scan`'s seed, `pub` on a `data` field, and the array-literal/slice mismatch —
are all done.

Three were worse than recorded. `+=` failed `--check` *always*, not only with
untyped parameters. The array-literal item was filed as "a design question,
not obviously a bug", and turned out to be `lower_type` discarding the
declared length — which made **every** fixed-size array parameter uncallable
with a literal. And `pub` on a `data` field was already in the spec's grammar
(§4.3, `visibility? IDENT ':' type`), so it was a straight parser omission
rather than a decision anyone needed to make.

**A "design question" filed against a defect stops anyone from running the
check that would settle it.** Both of the last two sat here for a session
because the note framed them as needing judgment.

---

## Failure taxonomy

Every bug found so far falls into one of seven shapes, ordered worst first. The
worst are the ones that produce a clean `--check` — except §7, which produces a
clean everything, because no test or checker in the repository can reach it.

### 1. Silently wrong answer

Compiles, runs, returns the wrong number. No error anywhere.

- **A bare unit-variant pattern bound instead of testing.**
  `?= s { Circle => 0, Square => 4, Triangle => 3 }` bound `s` to a fresh
  variable named `Circle` and took the first arm — every time, for every input.
  `sides(Square)` returned `0`.
- **An unknown net layer lowered to `Op::IDENTITY`.** `layer b: Lienar(128, 64)`
  — a typo of `Linear` — became a pass-through, and the net compiled, lowered
  and ran with that layer doing nothing. The translator had recorded it in
  `unknown_layers` all along; the only readers were the ABL-lowering path and a
  `train` warning. **Detected, recorded, and never surfaced is
  indistinguishable from not detected.**
- **An enum variant pattern matched nothing**, so a `match` quietly took the
  wildcard arm.
- **`guard cond else { … }` fell through**, running the body with the
  precondition false: `a(-5)` answered `-10`.
- **No capability call could be evaluated.** `fs.read_to_string(p)`,
  `env.get_env(k)`, `time.now()` and every other namespace call typechecked
  and then died with `unknown function` — the receiver is not a value, so the
  call fell through to the ordinary builtin table. **Every documented way for
  an agent to reach a resource was checkable and unrunnable**, which is why
  no cookbook recipe had ever been run. `io`, `fs`, `env` and `time` are now
  implemented; the rest report that the checker tracks the capability and the
  interpreter cannot perform it, rather than answering with a unit value.
- **`p"…"` printed nothing and interpolated nothing.** The print-string and
  eprint-string forms — the ones `cookbook/` uses throughout, and the shortest
  print in the language — were folded into a plain string literal by the
  parser. The statement was a no-op whose value was the raw text, braces
  included. They now desugar to `println` / `eprintln` over a format string.
- **A format-string hole is invisible to `--check`.** `p"value {nope}"` and
  `f"value {nope}"` typecheck clean and print `value <fn nope>` — the holes
  are parsed and evaluated only at run time, so an unresolved name inside one
  is neither a resolve error nor a type error, and renders as a function
  value. Not fixed: making holes visible to the checker means desugaring the
  interpolation at parse time rather than in the evaluator.
- **`println!("hi")` printed nothing and returned a bool.** MAGE has no
  macros, and this was the worst way not to have them: `!` parsed as logical
  *not* applied to the call, so `format!("hi {x}")` evaluated to `true` and
  `println!(…)` did nothing at all. No error at any stage. It is the single
  most likely line for a model carrying Rust habits to write. A `name!(` with
  no space between is now a parse error that names the macro and the
  replacement.
- **A string ending in an escaped quote lost it.** Literal decoding used
  `trim_matches('"')`, which strips *every* delimiter at each end, so `"x\""`
  came out as `x\` — the escaped quote became a backslash. Nothing errored:
  `contains(html, "\"")` was simply false and `split(html, "href=\"")` never
  split, so a link extractor returned `[]` and read as a logic bug in the
  program. Found by running a documentation example and checking its answer.
- **`range(1, 101)` ran as `0..1`.** `range` was the one typed vocabulary arm
  with no arity check, and the evaluator read argument 0 and discarded the
  rest. A FizzBuzz over `range(1, 101)` printed one line and exited 0. Both
  oracles now reject it and point at `a..b`.
- **`contains` on a string failed the checker.** The evaluator has always
  done three things — substring for a string, key membership for a map,
  element membership for a list — and the checker knew only the third. So
  `contains(email, "@")` evaluated correctly and reported `expected a
  collection, found str`.
- **The vocabulary enforced nothing.** All 15 argument unifications in
  `infer_vocab_call` were `let _ = unify(…)`, discarding the failure, so
  `join(xs, 7)` and `upper(5)` checked clean. The one thing a fixed, closed
  set of 31 combinators is *for* is catching misuse. They now report, and the
  stricter pass immediately caught three blocks written earlier in this
  session.
- **`len` decided an open type.** `len` accepts a string or a collection, and
  on an unresolved type variable it committed to a collection — so
  `v body = net.connect(url)` / `len(body)` / `body: str` was a type error,
  while the same three statements in the other order checked clean. **Statement
  order decided whether the program compiled.**
- **The suggested fix for an undeclared effect named the function.**
  `heal.rs` built the annotation from the first backticked word in the
  diagnostic, which in that message is the function — so the repair for
  `P.leak` reads "Add `/ P.leak` effect annotation". Applied, it writes an
  annotation that fails the unknown-effect check on the next pass. The
  diagnostic states the effects in a bracketed list; the fix now reads that.
  **A repair loop is an agent-facing answer too** — wrong advice there is the
  same defect class as a wrong number.

*How to find more:* run programs and check the **answer**, not the exit status.
That is why `check-examples.sh` pins printed output.

### 2. Typechecks, then does not evaluate

`--check` clean; `--eval` dies. `--check` cannot find these.

- `data` sum-type variants — resolution registered the type name and no variants
- bare variant constructors (`Rect(3.0, 4.0)` vs `Shape.Rect(3.0, 4.0)`)
- **`println`, `print`, `eprint`, `eprintln`** — registered as builtins, typed,
  attributed `IO`, and the evaluator had no arm for any of them. The most common
  function in the language. It survived because *no shipped example calls it*:
  the pin records each example's returned value, and all twelve return theirs
  rather than printing. **A pin covers only what it exercises.**
- `Ok`/`Err`, effect operations, and `impl`/`extend`/trait methods

### 2b. Evaluates, then does not typecheck

The inverse, and rarer — the checker rejects a working program.

- **`x |> f(a)` did not typecheck.** The pipeline operator is in the spec four
  times: prose, a token definition (`PIPE = '|>'`), a grammar rule
  (`pipe_expr = expression '|>' expression`), and a worked example. The lexer
  emits the token, the parser builds an `Expr::Pipeline`, and the *evaluator*
  desugars it correctly to `f(x, a)` — but the checker inferred the two sides
  independently, so `10 |> add(5)` was checked as the standalone call `add(5)`
  and reported `expected 2 argument(s), found 1` while evaluating to `15`.
  Every program using the documented operator ran correctly and failed
  `--check`.

- **Generic functions were declarable and uncallable.** `f id[T](v: T) -> T`
  lowered `T` to a nominal `Ty::Named` — a distinct type unifying with nothing —
  because signature collection ran `lower_type` with no generic binding.
  `check_function` bound the generics as fresh variables, but only for the
  *body*. So `id(1)` evaluated to `1` and reported `type mismatch: I32 vs
  sym1`. Each call site now instantiates its own copy of the quantified
  variables, which is what lets `id(1)` and `id("ab")` coexist.

The lesson is the same as its mirror image, from the other side: **`--check`
and `--eval` are two different oracles, and agreeing with one says nothing
about the other.** Run both.

### 3. Accepted and silently discarded

The parser takes it; nothing downstream honours it. The failure surfaces
elsewhere, pointing at the wrong line.

- **A swarm's `dispatch`, `aggregate` and `on_failure` blocks.** They parsed,
  were stored on `SwarmDef`, and reached exactly one consumer: the MLIR text
  dump. No resolve pass walked them, no typechecker entered them, the evaluator
  had never seen them — and `--fmt` printed them back as the literal string
  `dispatch { ... }`, so a format round-trip **deleted the body**. An agent
  could write a swarm's coordination logic, get `Status: OK`, format the file,
  and be left with nothing. §14 and §15 both say these three do not exist, so
  the parser now agrees and the diagnostic says what to write instead
  (`map`/`fold` over ordinary functions, §9.2). The `ast`/`mlir`/`fmt` paths
  stay — they are reachable from the bridge, and the damage was on the surface.
- **`use` brings nothing into scope.** `u totally.made.up.path` was accepted;
  `u std.io.read_to_string` then `read_to_string("x")` gave `unresolved name` at
  the *call site* — the one line the author wrote correctly. Now warns.
- **Default arguments were parsed and discarded.** `Param::default` was in the
  AST, with a parser test asserting it was *stored*, and read by nothing. Now
  honoured.
- **`--backend=<name>` selected nothing.** Documented as "Select hardware
  accelerator for dispatch", it reaches exactly one of six dispatch paths
  (`--run=abl-bytes`); every `--target=abl-*` builds a `CpuBackend` directly.
  So `--target=abl-compute --backend=cuda` printed "CpuBackend dispatch" and
  ran on the CPU without a word. For a repository whose headline numbers
  include GPU results, the failure mode is someone believing they measured a
  GPU. It now says when it is inert; wiring the other paths means threading
  `SelectedBackend` through their dispatch loops, which is item 14.
- **`nl/explain` and `nl/refactor` could never succeed.** Both take a `source`
  parameter, interpolate it bare into a prompt, and hand it to an engine whose
  `extract_code_block` reads source *only* from a ``` fence. So `intent.source`
  was always `None` and both answered "No source code provided" for every
  input, including well-formed ones. They are the natural-language surface —
  the methods an agent reaches for first — and the only 4 of 37 RAP methods
  with no test.
- **Capability handles performed no effect.** `resolve.rs` registered twenty
  namespaces and said their "use is tracked by the effect system". It was not: a
  `pub` function declared pure could call `net.connect(…)` or `llm.generate(…)`
  and check clean, while the bare `println(…)` beside it was caught. **The gate
  was open at exactly the seam the language documents as the way through it** —
  the safe-looking code was the unchecked code.
- **Method bodies were never effect-checked.** `infer_module` walked
  `ItemKind::Function` only, so nothing inside an `impl` or `extend` block was
  collected: `--check` on a module of methods printed **"Functions analyzed:
  0"** and returned OK, and a `pub fn` inside one could `fs.read_to_string(…)`
  while declaring nothing. Calling a method propagated nothing either, so a
  pure-looking `main` could reach any capability through one `.`. This is the
  same shape as the row above it, one call form over: **the two ways to reach a
  capability were `namespace.op(…)` and `receiver.method(…)`, and neither was
  checked.** Methods are now keyed `Type.method` and checked like any other
  body; a call is charged to a method when the name is unambiguous in the
  module. The count in the summary line is the tell — it was the one number
  that said the checker had done nothing.

### 4. Documented but unimplemented

- **`|x| expr` closures** — in the spec's formal grammar *and* its feature list,
  and a parse error. The vocabulary is built on higher-order functions, so the
  documented way to pass one to `map`/`filter`/`fold` was the one that failed.
- **`!` as break** — the spec names it, and the lexer's own comment reads
  `KwBreak, // break (legacy — canonical is !)`. The canonical spelling parsed
  nowhere; the "legacy" one was the only one that worked.
- **`/ agent`** — a documented effect that was a parse error *and* not a
  built-in kind: two independent failures on one row of one table.
- **§11.2's operations column** — of 41 operations named, 22 perform nothing.
- **The five swarm orchestration patterns.** `swarm_map_reduce`,
  `swarm_pipeline`, `swarm_saga`, `swarm_fan_out`, `swarm_race` and
  `grammar_extension` are reserved in the lexer and consumed by **nothing** —
  no parser arm, no evaluator, no mention in MAGE_SPEC.md — while
  `agent-guide/rap-agentic.md` documented three of them with worked calls. A
  reservation costs the name twice: the call is a parse error, *and* no user
  function can fill the gap. They now say so when written. Implementing them,
  or un-reserving them, is a design decision.
- **The published container format omitted its symbol table.** The `.abl`
  container has carried one since v2 — the section that makes a `kb` artifact
  self-describing, because without it a decoder recovers predicate arities and
  not their names. It appeared in **none** of the four places the format is
  described: `MAGE_ONTOLOGY.json`'s `abl.format`, `AGENT_PROTOCOL.md`, and the
  module docs in `abl.rs` and `main.rs`. An agent implementing a decoder from
  any of them parsed the items and stopped, leaving **100 of `unified.mg`'s 420
  bytes** unread. `decode_container` was fine throughout, which is why nothing
  noticed: the code reads the real format, and only the description was wrong.
  A test now walks a real container using *only* the published field list and
  asserts it lands exactly on the last byte.
- **A global rename put the language's name where four magic bytes belong.**
  `ABL` → "Agentic Binary Language" rewrote every literal spelling of the
  container magic: the ontology's format line and its fallback, the
  `--manifest` entry for `--target=abl-bytes`, two module docs, and — worst —
  the decoder's own error message, so the one thing a caller sees when a file
  fails to open said it expected a 23-character string instead of `ABL1`. The
  `"magic"` *field* was right the whole time, because it is read from the
  constant, which is exactly why the section looked healthy.
- **`abl_compute`'s "Supported ops" table listed 6 of the 23 it dispatches.**
  It said `LINEAR`, `MATMUL`, `CONV2D`, `ATTN` and the normalisations "are
  reported as **unsupported** because they require parameter tensors that the
  bridge does not yet thread through" — while the parameter store described
  fifty lines below threads every one of them, and the capstone benchmark had
  been dispatching 97 ops with an empty unsupported list. Anyone deciding
  whether the CPU backend could run their net was reading a description of a
  compiler that no longer existed. The table is now the dispatcher's arms, in
  both directions, checked by scraping this file — an opcode listed but not
  dispatched promises what the backend cannot do, and an arm missing from the
  table hides what it can.
- **`type-mismatch` healed nothing for the mismatch the language reports.**
  The pattern is published as the mechanical repair for type errors, and its
  generator only fired when the message named `Option` or `Result`. The
  checker's own commonest mismatch names neither — `type mismatch: I32 vs
  Usize`, which is what `f m() -> i32 { len(xs) }` produces, the first error
  most programs hit. So the repair an agent asked for came back empty, on the
  error it was most likely to be asking about. It now offers a real cast edit
  between two numeric types and names both sides otherwise. Found by giving
  every pattern an `example` message and running it.
- **The ontology published 17 of the binary's 29 flags.** The twelve missing
  included **`--eval`** — the only way to run a program, and half of the
  two-oracle discipline this repository depends on — plus `--version`,
  `--json`, `--fix`, `--manifest`, `--token-report` and the whole `--build=`
  / `--describe=` / `--spine=` family. An agent grounding in
  `MAGE_ONTOLOGY.json` could not learn that MAGE programs can be executed.

  The test that should have caught it asserted **eight named flags were
  present**, under a doc comment reading "every CLI flag the binary actually
  accepts must be in the cli_flags ontology section". It now scrapes the flag
  literals out of `main.rs` and compares the two sets in both directions —
  verified by removing `--eval` and watching it fail. 36 flags are published.
- **The tool manifest omitted `--eval`, calling it "superseded by
  `--run=abl`".** It is not superseded: `--run=abl` reads an Agentic Binary
  Language *container* and answers `bad magic` on a `.mg` file. So
  `--manifest` — which describes itself as "read this first" and promises an
  agent will never need the prose docs — listed no way to run a MAGE program.
  Now listed, with the two-oracle point in its detail text.
- **A test kept its own copy of the thing it tested.** `positional()` in
  `main.rs`'s test module duplicated the production modifier-flag filter, so
  deleting `--token-report` from the real filter left the test green. One
  function now, called by both — verified by deleting the flag again and
  watching it fail. **A test that mirrors the implementation cannot catch the
  implementation drifting.**
- **`MAGE_SPEC.md` Appendix B — the dual-syntax mapping table — had seven
  wrong rows**, under the heading "Every Human-mode construct has an
  Agent-mode equivalent. Both parse to the same AST." `const` → `c`
  (lowercase is an identifier; the sigil is `C`), `else if` → `:?` (the space
  matters: `: ?`), `for x in y` → `@ x ~ y` (the separator is `in`), `while` →
  `loop ?` (it is `@w`), `Foo { x }` → `Foo @{ x }` (the `@` goes before the
  name), plus `const fn` → `c f`, `pub(crate)` → `~` and `crate::` → `~.`,
  none of which parse in either mode. B.8's "shared syntax" listed `f16`,
  `bf16` and `tensor!` literals, none of which exist.
- **`?:` was published as a sigil by two documents that disagreed about what
  it meant** — the ontology called it "human-mode if (sugar for ?)",
  MAGE_SPEC.md B.2 called it the KB-query sigil — and the lexer has no such
  token at all. Removed, with its absence pinned the way `^`'s is.
- **`nl/explain` answered with Rust `Debug` output.** Asked to explain
  `+f add(a: i32, b: i32) -> i32`, it said
  `(a: Path { segments: ["i32"], type_args: [] }, …)` — the AST node
  `{:?}`-formatted into a natural-language answer, from the method an agent
  reaches for to *understand* code. The formatter has rendered types correctly
  all along; `fmt::type_to_string` is now public and the explanation uses it.
- **`nl/refactor` could not refactor, defeated by its own name.** The intent
  classifier matched keywords as bare substrings, and the knowledge-base
  branch lists `fact` — which is inside "re**fact**or". Every refactor request
  classified as "generate a knowledge base" and answered `kb Generated { }`,
  with `ok: true`. The previous session had "fixed" this method by fencing its
  source, which only got it far enough to answer confidently and wrongly.

  Keyword matching is word-bounded now (`add` no longer matches
  "addressable", `run` no longer matches "runtime"), multi-word phrases still
  match as phrases, and `nl/refactor` returns refactored source.
- **The published RAP contract was wrong for a third of its methods.** RAP is
  the protocol an agent drives the compiler through, and `rap_methods`
  publishes each method's parameters and return keys. **Eight methods read
  parameters the ontology did not name, and seventeen returned keys it did
  not name.** `skb/query` publishes `{query}` and reads `by`/`value` —
  calling it as documented returns `matches: []`, because the missing keys
  default to the empty string. `sandbox/policy` publishes `name` and reads
  `agent`. `verify/contracts` publishes `source` and reads five other keys.
  `build/check` publishes `diagnostics` and returns `errors`.

  The test that should have caught it called each method with `{}` and
  asserted only that the method was not "unknown" — which a method that
  always errors still passes. The new one calls each with inputs that
  *succeed* and checks the answer's shape, and it caught a mistake in its own
  first draft.

  **`format/agent` and `format/human` did not format.** Both returned the
  AST — `format/human` said "same as parse for now" — while publishing
  `{formatted, ok}`. `fmt::format_agent` is what `--fmt-compact` has always
  used. Both now return formatted source, verified over the wire.
- **Six published "CI floors", enforced by nothing.** The ontology's
  `ci_floors` section named `MIN_PARSE >= 98`, `MIN_HEAL >= 40`, a
  `native-lexer ratio <= 1.100` and three more, under a doc comment saying
  they were "read from `.github/workflows/ci.yml`". **That file contained
  none of them** — no reliability-bench job, no heal threshold, no ratio gate.
  `UNIFICATION.md` went further and described a "new CI step" parsing
  `benchmarks/TOKEN_REPORT.md`; there was no such step. An agent reading
  either believed six regressions were gated.

  Two of the six were not even true: the file-oracle structural-heal
  contribution is **1**, not `>= 2`, and the stage-3 refine smoke is 0 without
  a wrapper. Those are now stated as observations. The three real ones are
  measured and enforced by `scripts/check-ci-floors.sh`, which CI runs —
  verified by raising `MIN_PARSE` to 100 and watching it fail. Reading the
  ratio also has a trap worth knowing: `TOKEN_REPORT.md` has four `**Total**`
  rows and the first one is *source bytes* (1.055), so the check anchors on
  the section heading rather than the row shape. It passed for the wrong
  reason on its first run.
- **`data` and `extend` were missing from the published AST kinds.** The
  ontology enumerated 18 item families; `ItemKind` has 20. The two absent
  were records/sums (`data Point(…)`) and methods (`extend Type { … }`) —
  three of the constructs the human-mode guides lead with. Nothing compared
  the two lists, and the published names were a parallel vocabulary (`Mod`,
  `EffectDef`, `SpecBlock`) that made comparing them awkward enough that
  nobody had. The names now match the variants exactly and a test scrapes
  `ast.rs` to compare both ways.
- **The ten examples the ontology publishes were only *parsed*.** They are
  what an agent grounds in when it asks what MAGE looks like, and the test
  bar was "parses" — the same weaker criterion that let 43 documentation
  blocks through earlier this session. They all typecheck (verified by
  breaking one and watching the test fail), and the test now requires it.
  Likewise `framewerx_modules` checked that its 256 paths exist but not that
  the 243 symbol names it publishes appear in the files it points at.
- **Five ontology counts quoted in the docs were stale**, in the two places
  that quote them: `cli_flags (17)` for 36, `heal_patterns (~13)` for 34,
  `keywords (12)` for 102, `layer_map (31)` for 21, `effects (15)` for 22,
  and "21 sections" for 22. All 22 section sizes are now emitted from the
  committed ontology and pinned, so a claim about the ontology cannot drift
  from the ontology.
- **An integer literal adopted a width without having to fit it.**
  `f g(n: u8)` called as `g(300)` typechecked clean, as did `i8` ← 200 and
  `i32` ← 3000000000. The literal's value is now carried alongside its type
  variable and range-checked against whatever kind it ends up unified with —
  boundaries (255, 127, `0xFFFFFFFF`) still pass, and a float context still
  accepts an integer literal.

  What this does *not* do is constant-fold: `g(0 - 1)` into a `u32` is an
  expression, not a literal, and still passes. That is the remaining half of
  old item 12, and it needs a different analysis rather than a bigger range
  table.
- **`--backend` reached one dispatch path out of six.** `--run=abl-bytes`
  honoured it; every `--target=abl-*` path built a `CpuBackend` directly, so
  `--target=abl-compute --backend=cuda` printed "CpuBackend dispatch" and ran
  on the CPU. The selection is now resolved once in `main` and threaded
  through all five via `SelectedBackend::as_dyn`, each path reports the
  backend it is using, and a subprocess backend — which has no in-process
  `Backend` — says what it fell back to instead of quietly using the CPU. The
  duplicate resolution inside `run_dispatch_abl_bytes` is gone too; a bad
  `--backend` name used to print its error twice. **The GPU path itself is
  still unverified here** — that needs `--features cuda` and hardware.
- **Every fixed-size array parameter was uncallable with a literal.**
  `lower_type` dropped the declared length and used 0 while an array literal
  types with its real length, so `f take(xs: [i32; 3])` called as
  `take([1, 2, 3])` failed with "array size mismatch: 3 vs 0". Now lowered; a
  non-literal length still lowers to 0 and unifies with anything, and a
  two-element literal is still rejected.
- **`pub` on a `data` record field was a parse error**, while the same field
  in a `struct` accepted it — and MAGE_SPEC.md §4.3 spells the grammar
  `visibility? IDENT ':' type`.
- **Thirteen of the seventeen registered builtins had no arm in the
  evaluator.** `resolve` registers `assert`, `assert_eq`, `panic`, `todo`,
  `dbg`, `vec`, `format` and the rest so ordinary code resolves; only `min`,
  `max`, `abs` and the print family were implemented. **`assert` is the only
  assertion the language has** — every `@test` in the documentation reaches
  for it, and it could only ever fail with `unknown function`. Found by
  running the documentation instead of checking it, which is now
  `scripts/check-doc-evals.sh`.
- **`GlobalAvgPool` had no shape rule.** It fell into the unknown-op arm and
  was treated as shape-preserving, so a `Linear` after it was checked against
  the *width* instead of the channel count.
  `framework/framewerx/examples/resnet_classifier.mg` — a textbook ResNet head
  — reported "expects last dim 256, but the preceding layer produced
  [1, 256, 20, 2]", and **two sessions wrote the example off as an
  unadjudicable sketch**: "either the example or the shape rule is wrong, and
  telling which needs someone who knows the intended architecture". The rule
  was wrong. Adjudicating it needed one look at the arm list, not an expert.
- **The Greek agent-mode surface for AI constructs.** `MAGE_SPEC.md`
  Appendix D publishes 33 symbol rows — `Ψ` for `net`, `λ` for `layer`, `Ω`
  for `evolve`, `κ` for `kb`, `Σ` for `swarm`, `⊗` for matrix multiply — and
  **the parser consumes none of them.** Fifteen are real tokens in the lexer
  (`KwPsi`, `KwSigma`, …) and no arm matches any; `Ψ Classifier { … }` is
  `expected item, found KwPsi`. The agent mode that works is the ASCII half:
  `+f`, `v`, `m`, `S`, `E`, `I`, `T`, the control compressions, and the short
  aliases (`sw`, `topo`, `cons`, `fx`, `hx`, `gd`, `df`, `xd`) — all verified.
  The AI blocks use the same keyword in both modes. The tables are now headed
  with that status rather than deleted: the compression argument is why the
  language has two surfaces, and it is worth keeping as design.
- **`internals/05`** — `is_sub_effect`, `Forge.toml` capability grants, effect
  polymorphism (`/ *`), `E0401`/`W0410` diagnostics: none exist. Marked rather
  than rewritten, because whether to build them is a design decision.
- **`internals/03` §3.2** — four `use`-resolution steps and six import styles,
  none of which happen.

### 5. The document is wrong, not the compiler

The mirror image, and easy to get backwards.

- **`^` as Return.** One ontology line claimed it; the spec names `ret` in both
  sigil tables and mentions `^` exactly once, as bitwise XOR — and `^` is already
  the `^T` Box prefix. **I implemented `^`-as-return first**, to make the
  published claim true, then had to add a special-case newline guard when
  `m x = 7` / `^ x` parsed as `7 ^ x`. That guard was the signal. Reverted.
- **`keywords.introduces`** was two fields wearing one name: the real token kind
  for 83 keywords, a hand-written label for 19 — `agent` claimed `AgentDef`,
  `val`/`var` claimed `Let`, a token this language *removed*.
- **`types`**: `S` published as shorthand for `String` (it is the `struct`
  keyword and can never be a type); `Map[K,V]` and `Set[T]` are `{K: V}`, `{T}`.

- **And once, understating.** The spec said "**Handlers do not resume.** An
  operation call dispatches to its arm and returns like an ordinary call." The
  second sentence is mechanically exact — and *returning like an ordinary call
  is* single-shot tail resumption; the body carries on. The first sentence
  reads as "the body stops", which is not what happens. Abort via `ret` in an
  arm worked too, scoped correctly to the handled block. None of it was tested.
  A reader would have concluded handlers were unusable and either avoided them
  or started a large refactor.

- **`benchmarks/FINDINGS.md`, the document that exists to hold the honest
  numbers.** Its §1 measurement table had five stale figures (source bytes,
  dense bytes and the native-lexer total, which is the one the CI ceiling
  gates). Worse, §1 spends four sections proving the "~50 %" claim
  unsupported and then heads a later section "Where the ~50 % claim *is*
  real", asserting the binary encodings are "1-2 orders of magnitude smaller
  than the equivalent text in either MAGE or Rust". Measured against the MAGE
  text they replace: TransformerBlock 243 B → 47 B, MLP 98 B → 17 B, FamilyKb
  116 B → 68 B. That is **2-6×**, and the section now carries the table rather
  than the adjective.
- **`benchmarks/FINDINGS.md` §2 and §6 read as present tense and were two
  years of work out of date.** §2's TL;DR: "Three quarters of the corpus's own
  reference solutions don't parse through today's MAGE prototype parser", with
  a per-category table showing seven of ten categories at 0.0 %. Measured
  today: **lex 100/100, parse 99/100, effective 100/100** — every one of those
  categories parses. §6's headline "effective 39/100 under near-correct input"
  is now **70/100**, and heal-reach on perturbed input went 13 → **42 of 73**.
  The file is a chronological log and its sections are legitimately dated
  snapshots; what made them misleading is that nothing said so, and its own
  summary table carried the old numbers as current. Each superseded section now
  carries the re-measured figure beside it, and the header table is dated.
- **`rmi/docs/api.md` documents an FFI API that exists nowhere in the crate.**
  `FfiValue` and `FfiFuncPtr` are in no source file; `register` takes an
  `FfiBinding`, not `(name, sig, ptr)`; `call` deals in `Val`; and
  `call_unchecked` is documented **`unsafe`** and is not. The real thing passes
  RMIL values to safe Rust closures (`Box<dyn Fn(&[Val]) -> Result<Val,
  String>>`), so the block painted a *more* alarming surface than exists —
  raw `*mut u8` pointers and an unsafe entry point where there are neither.
  `unsafe` in a signature is a contract, and documenting one that is not there
  is as much a defect as omitting one that is. Also every **Module:** line said
  `framewerx::…` when the crate is `rmi` — all twelve wrong, all twelve modules
  present, so a reader following them concludes the modules are missing. Found
  while checking whether the vendored crate repeated the "all reviewed" claim,
  which is the only reason it was found. Measured across the whole file:
  **27 of 228 documented items — 12% — exist nowhere in `src/`.** Correcting
  `CheckpointManager` took that to 23; every method in *its* block was wrong
  too (`save_checkpoint` for `save`, `&str` ids for `Uuid`, a constructor that
  never took those arguments), and the *type* resolves, which is exactly what
  made it hard to notice — nothing fails until you call a method.
  `scripts/check-rmi-api-doc.sh` ratcheted that to **0** — every documented name
  now exists — and fails outright on an unresolvable module path.

  Draining it turned up one mistake made over and over, in five separate
  sections: **a documented lookup keyed by *name* where the real API takes a
  `Uuid`.** `Ontology`, `AIConceptsOntology`, `AIHistoryKB`,
  `CheckpointManager` and the hash ring all had it. That shape survives review
  because the call reads correctly — `get_concept("attention")` is exactly what
  you would expect to write — and only the type checker objects. Two others
  were worse than a compile error: `KeyValueStore` documented
  `with_compression` when the real builder is `without_compression`
  (compression is on by default, so the reader enables what is already on), and
  `ArchitectureBuilder::add_skip_connection(from: usize, to: usize)` where the
  real `residual_add` takes a node `Uuid` — positions offered where identities
  are wanted. The check is deliberately name-level: it cannot
  see a wrong signature on a function that exists, which is most of what was
  wrong here, so `cargo doc` stays the authority and the file says so.

  Following that admission with a measurement: of 162 documented functions,
  **66 are defined exactly once** in `src/` and so can be compared by arity
  without ambiguity. Four disagreed — `add_fact` (2 params documented, 3 real),
  `similarity` (documented as a method, actually an associated function on
  `&[f32]`), and `Protocol::encode`/`decode`, which **belong to `Frame` in a
  different module** and have different shapes. All four fixed; the set is now
  clean. The other 96 share a name with another definition (`new`, `len`,
  `save`), so a name-keyed comparison cannot tell which one it is looking at.

  **No arity checker was added, deliberately.** It would cover 41% of the
  functions and carries a false-positive class — my own `/* … */` placeholders
  tripped it twice while writing the fixes above. A check that is right 41% of
  the time and cries wolf is the kind this session has spent its length
  cleaning up after. The measurement is worth repeating by hand after a
  refactor; it is not worth a ratchet.
- **`ontology.md` was invisible to the checker, and wrong in all three of its
  enumerations.** It contains no `pub` items, so the name check reported zero
  documented items and passed — a file the instrument cannot see reads exactly
  like a file with nothing wrong. Checked by hand: **six of eleven documented
  relations do not exist** (`related_to`, `extends`, `used_for`, `builds_on`,
  `introduced_by`, `superseded_by`) and six real ones were missing; the domain
  set is ten variants, not the five headings ML/DL/SYM/NS/MAS, of which only
  two have any counterpart; and the concept-property table named `name` as the
  unique identifier when it is `id: Uuid` — the same name-versus-id error the
  API docs made in five places.

  The taxonomies were *measured* rather than rewritten: **166 documented nodes,
  28 shipped concepts, 12 in common.** They are a survey of the field, which is
  a fine thing for the document to hold and a misleading thing to present as a
  crate's contents, so the file now says which it is. **A prose document is not
  covered by a check that reads code blocks**, and the passing result said
  nothing about it either way.
- **Feature coverage, swept.** Five feature flags exist across the crates.
  `prototype/cuda` and `ribosome/tls` are built by CI and both hold — TLS runs
  168 tests against 164 without it, all green, verified locally because CI
  cannot see this branch. The other three are **never built by CI or any
  script**:

  - `rmi/gpu` — compiles fine and its **6 `wgpu_backend` tests pass**. Nothing
    had run them; nothing had rotted either. A clean negative result.
  - `rmi/wasm` — pulls five dependencies (`wasm-bindgen`, `js-sys`, `web-sys`,
    `getrandom`, `uuid/js`) and **no source file references
    `feature = "wasm"`**. That is defensible as dependency configuration for a
    wasm32 target rather than dead code, but nothing builds it, so whether it
    still resolves is unknown.
  - `rmi/full = ["cuda", "gpu"]` — **unbuildable**, because `cuda` is (item 15).

  The pattern holds one level up from the tests: a feature nobody builds is a
  compilation unit nobody checks, and two of the three were fine. Worth knowing
  *which* two.
- **The rename broke a shell script, and that is its fifth victim.**
  `scripts/demo_agent_workflow.sh` had `ABL="$DEMO_DIR/flash_attention_block.abl"`
  rewritten to `Agentic Binary Language="…"` — which bash reads as the command
  `Agentic`, and every `"$ABL"` became `$Agentic` (unset) followed by two
  literal words. Nine occurrences. The demo died at step 3 of 5 and **still
  exited 0**, so even running it did not obviously fail. Nothing referenced the
  script, so nothing ran it.

  That is now five defects from one mechanical edit: the container's magic
  bytes, the decoder's error message, twelve `framewerx::` module paths, the
  `RmiError` type name, and a shell identifier. **A rename that touches
  identifiers is not a documentation risk, it is a code risk**, and the only
  reason this one was cheap is that the script had no callers — which is also
  why it stayed broken.
- **Swept for the rest, and the shape has a third instance.** After the CUDA
  pin and `eval_bench`, I looked for every check whose preconditions might not
  be met: **8 `#[ignore]`d tests across five crates.** Two are the benches, now
  both run. The other **six are `rmi`'s CUDA tests, and they cannot run on any
  hardware** — they live in `compute/cuda_full.rs`, which no `mod` declaration
  references, so they never compile. Not "never run": *not code*. Trying to run
  them with a GPU present turned up more: `rmi`'s `cuda` feature fails to
  **build**, because it pulls `cudarc 0.10` for that same dead file, and
  `cudarc` needs a build-time toolkit the real CUDA path deliberately avoids.
  See item 15. **A test that cannot compile is quieter than a test that fails**,
  and `#[ignore]` looks the same either way.
- **`eval_bench` had been failing for 76 commits, on this branch, unnoticed —
  including through everything this session reported as green.** It computes
  71/73, not the 73/73 that `README.md`, `ARCHITECTURE.md`, `DOCS.md` and
  `DIRECT_CODEGEN_STRATEGY.md` all claim. Two fixtures bind `m` and `v` — the
  agent-mode sigils for `let mut` and `let` — which the parser has rejected
  since 2026-08-12 by design, with a diagnostic that names the collision.
  `master` has no such rejection; the rule and the breakage are both on
  `handoff`, in the 81 unpushed commits.

  **It went unseen because the bench is `#[ignore]`d.** Plain `cargo test`
  skips it, `scripts/test-all.sh` skips it without `--bench`, and CI's step
  that *does* run it has never fired, because the branch has never been pushed.
  Every "all crates green" in this session was true of the default suite and
  said nothing about this. The fixtures were stale, not the rule — renamed to
  `mp` and `xs`, and it is 73/73 again.

  Three conditions had to hold at once for a red test to look green for 76
  commits: **ignored by default, gated behind a flag, and on a branch CI cannot
  see.** Any one of them alone would have been survivable. This is open item 0
  — the unpushed branch — with a cost attached rather than a preference.
- **The last unchecked pin in the repository was wrong.** Seven documents claim
  the CUDA test count, all pinned to one measured key — and every
  `--check-docs` run in this session reported *"CUDA counts not checked (no
  --cuda run supplied one)"*, which is the honest fallback doing its job and
  also the reason nobody noticed. Running it on the hardware the documents name
  (dual RTX 3090 Ti, driver 610.88, exactly as claimed): **1,229 tests, not
  1,071.** All seven stale, in `HANDOFF.md`, `MEASUREMENTS.md`,
  `ARCHITECTURE.md`, `ROADMAP.md`, both `test-all` scripts and `ci.yml`.

  The figure dates from 2026-08-05 and the suite has grown since. Nothing was
  broken — every CUDA test passes — but **a pin that cannot run is a pin that
  reports "not checked" forever**, and seven places inherited a number from the
  last time someone had a GPU in front of them. That is open item 2 (the GPU
  runner) stated as a consequence rather than a wish: without unattended CUDA
  CI, this figure goes stale again the moment the suite grows, and the only
  thing that catches it is a human with the hardware choosing to look.
- **The correct description of the container format was in the repository the
  whole time.** `ARCHITECTURE.md` §1 lists the symbol table, in the right place
  and the right shape — while `MAGE_ONTOLOGY.json`'s `abl.format`,
  `AGENT_PROTOCOL.md`, and the module docs in `abl.rs` and `main.rs` all omitted
  it, so a decoder written from any of those stopped 100 bytes early. **Five
  descriptions of one format, one of them right, and nobody had compared them.**
  That reframes the finding: it was never that the format was undocumented, but
  that the documentation was unreconciled. `every_description_of_the_container_format_agrees`
  now compares all five, on the three facts that were wrong — the magic bytes,
  the version, the symbol section — and fails if a section lands in one file and
  misses four. Verified by stripping the symbol lines out of
  `AGENT_PROTOCOL.md` and watching it name the file. It is deliberately shallow:
  the decoder-conformance check already exists next to it, and this one guards
  *agreement*, which is the thing that actually broke. The rest of
  `ARCHITECTURE.md` audits
  clean — every source-map file exists, the five-workspace claim holds (`rmi` is
  an implicit workspace root, and `cargo metadata` confirms it), there is no
  root `Cargo.toml`, and the layout counts match their pins. Its one stale
  sentence explained `forge`'s test count by the ribosome/germline split, a
  number that has since moved again for an unrelated reason.
- **`SPINE_COLLABORATION.md` §8 is accurate, and every command in it fails on
  the first attempt anyway.** All six claimed bridge functions exist, the "5
  tests" is exactly 5, and `--spine=profile|swarm|frame` all work and return
  what the section describes — the frame's `byte_len`, FNV `content_digest` and
  `exec: false` verified against a real 141-byte container. What is missing is
  the **input schema**: nothing in the document says what `agent.json` contains,
  and the profile's input key is `agent` while its *output* key is `name`. The
  natural first guess is `{"name": …}`, mirroring the output, and it fails with
  `missing field \`agent\``. A §7b now gives the minimum fields for all three,
  measured from a run. **A correct document can still be unusable**, and that
  gap does not show up in any check this repository has, because every claim in
  it is true.
- **`MAGE_ONTOLOGY.md` — the prose ontology — had 13 of 23 RAP method
  signatures wrong.** Every method it names exists; the drift is in the
  parameters, checked against the JSON's `rap_methods` (itself verified by
  `every_published_rap_key_is_real`). The sharpest are `skb/query`
  (`{database, category}` for `{by, value}`) and `attribute/expand` /
  `attribute/compress` (`{source}` for `{name}`), where **a caller following
  the page gets an empty result rather than an error** — the arm reads names
  that were not sent and falls back to the empty string. The `mode` parameter
  on the two language services was never real: both modes lex and parse
  through the same entry point, which is the point of the dual surface. All 13
  corrected; `DOCS.md` already says to prefer the JSON, and the page now says
  why.

  Its "255 safety rules across 8 databases" is **correct** — and checking it
  found the inverse defect in the code: the test pinning that number is called
  `rule_total_count_210` and asserts `255`. A test name is documentation that
  grep reaches first; anyone looking up the rule count found 210 in the name
  and never read the body. Renamed.

  Swept for others — test names of the form `*_count_N`, `*_total_N`,
  `exactly_N`, `all_N` across all five crates. **There is exactly one**, and it
  was that one. A negative result worth recording so the next reader does not
  repeat the search: the shape is real but it was not systemic here.
- **`ARCHITECTURE_DSL.md` is the one document whose measured claims all held.**
  Every byte figure re-derived exactly from the checked-in construct: source
  271 B, container 141 B, expr 110 B, single-block 126 B / 95 B, decompiles to
  72 layers, `dispatched=72 unsupported=[]`. Worth recording *because* it is the
  exception — and the reason is visible in the prose, which reads as pasted from
  a run rather than written around one. That is the same split
  `AGENT_PROTOCOL.md` showed from the other side: its decode block survived and
  the hand-written encode figures did not.

  One real gap, now stated in the file: **its token ratios cannot be reproduced
  from this repository.** The cl100k tokenizer is in `agentic-eval`, a separate
  repo, and the "839 manual tokens" baseline is a hand-expanded source that is
  not checked in — a reconstruction lands at 2,749 B against the 2,320 B
  stated, which proves nothing but that the naming differed. Committing that
  file beside the other constructs would make the headline claim checkable; it
  is a one-file fix nobody has done.
- **`DOCS.md` — the document that sets the anti-drift rule — had drifted.** It
  states the rule ("when a design document and a measurement disagree, the
  measurement wins"), explains that stating a rule was not enough, describes the
  checker built to enforce it, and then gives the checker's coverage as "58
  claims with all three modes, 44 by default". ROADMAP step 158 says "51 and
  38". The measurement is **78**. Three numbers for one figure, in the paragraph
  explaining why that happens.

  My first correction restated it as 78 — and was wrong **within the same
  commit**, because that commit added two more pins and the run reported 80.
  So the number is now gone entirely, replaced by "run the command". A count
  that changes whenever someone does the right thing should not be written
  down: the checker cannot verify its own total without circularity, so any
  figure there is one nobody measures. Two other counts
  there were pinnable and are — the root document count and the ontology's
  section count, which said 21 against a measured 22. The index itself is sound:
  every one of the 23 links resolves, and every root `.md` is indexed except
  `DOCS.md` itself.
- **`RIBOSOME.md` and `GERMLINE.md` under-report their own evidence.** Both
  carry tables of "the tests that assert a property this system is judged on",
  and both list a subset: Ribosome names 15 of 19 and calls 11 scenarios 10;
  Germline names 4 of its 6 closed-loop scenarios. **Every name they do list
  exists** — this is omission, not invention, and it is the first documentation
  drift this session that costs the project credit rather than misleading a
  reader. The omitted tests are not filler. Ribosome's includes the regression
  test for `cache_hit_ratio`, which reported a *perfect* reuse ratio for a build
  that failed every action while `Fitness::reuse` paid it the maximum score.
  Germline's two are `the_runner_drives_a_real_workload_to_a_bounded_stop` and
  its failure twin — the ones that answer the obvious sceptical question about
  an RSI loop, which is whether anything in the path is a stub. Also two
  modules (`mac`, `tls`) missing from Ribosome's architecture table, and `tls`
  matters: two documents turn on `rustls` staying out of the default build, and
  a module table that never mentions it invites the reader to assume there is
  nothing to keep out.
- **`rmi/docs/protocol.md` describes a wire format the implementation does not
  speak.** Checked after `api.md`, on the same suspicion that one bad doc is
  rarely alone. The header diagram is wrong in every field: the magic is
  **`FWRX`, documented `FRWX`** — the W and R transposed, with the hex spelling
  out the same wrong order so the two agreed with each other and not the code;
  `flags` and `message_type` are the wrong widths and the wrong way round;
  `MsgId` and `Timestamp` are not in the header at all (those 16 bytes are
  `payload_length` and `checksum`); there is no sender-ID section and no
  attachment count. The checksum is **XXH64 stored inside the header**, not a
  CRC32 trailer — wrong algorithm, wrong width, wrong position, and computed
  over different bytes in a different order. A client written from that page
  fails at byte 4, and every one of these is a silent interop failure rather
  than a compile error, which is what makes a wrong wire-format diagram worse
  than a missing one. The other two docs were nearly clean: one stale
  `SymbolEmbedding`, and two `pub fn`s that are deliberately illustrative.
- **`AGENT_PROTOCOL.md`, nine figures at once.** The document that tells an
  agent how to emit bytes rather than text opened with "the genuine ~50×
  efficiency win", and its own worked example three sections down measures
  **0.225** — 1866 B of text to 420 B of container, which is 4.4×. It also
  gave the container as v1 (it is v3) with 300 bytes (420), "header overhead
  ~50 bytes" (framing is 195, nearly half the file at this size), three of the
  five item hashes stale, "12 opcode families, 95 ops" (7 and 107), "31 neural
  opcodes by name" (21), and "~3 orders of magnitude" more programs per
  response (nearer 5-8× per transformer block). Every one of them is
  reproducible with a command printed in the same file. The decode block
  underneath was accurate, which is the tell: the outputs that were *pasted
  from a run* survived, and the ones written by hand around them did not.
- **The README, four figures at once.** The front page's binary-IR block was
  labelled *(measured)* and three of its numbers were not: it printed the
  container header as `ABL1 02 00` (version is **3**), sized the source at
  `327 B` (the file it names is **139 B**), and derived "**71.9 % smaller**"
  from that wrong size (the real reduction is **33.8 %**). The one measured
  number, 92 bytes, was right — which is how the rest survived. It also said
  the container "decompiles to the exact net above"; it decompiles to an
  equivalent net whose **layer names are regenerated** (`fc1` → `l_linear_1`),
  because the container stores ops, not identifiers. Everything else on that
  page does hold: form 1, form 3, the composition operators, the Quick Start
  snippet (→ `30`), the whole `forge` workflow (→ `120`), and all eight
  `mage-parse` targets it lists.

**The rule:** the ontology *describes* the language; it does not define it. When
a generated artifact and the spec disagree, the artifact is the likelier
suspect. Reaching for a special-case parser guard to support a one-character
spelling means the spelling is wrong, not the parser.

### 6. Parse ambiguity, and diagnostics naming the wrong token

- **A newline did not end a statement before `(` or `[`.** `v a = 1` followed by
  `(2 + 3)` called the literal `1`. Reported as "a `(` after a block", which was
  one instance of a general rule.
- **Keyword-as-identifier, five positions.** `agent`, `swarm`, `kb` collided in
  effect annotations, expression position, `expect_ident`, module declarations
  and use paths. **Patched per-position three times before generalising** — each
  patch fixed only where someone had hit it and left the rest broken.
- **A type name blocked a function of the same name.** `define_type` mirrors
  into the value namespace so enum constructors resolve, and duplicate
  detection could not tell that copy from a definition — so `S Point { … }`
  beside `f Point(…) -> Point`, the ordinary constructor pattern, reported
  `duplicate definition: Point`. Worst for `sp`, where a spec block *names the
  function it constrains*: the contract feature could not be used as designed
  at all.
- **The prelude reserved eighty words globally.** `M net { }` reported
  `duplicate definition: net` against a definition the author never wrote and
  could not see. Source definitions now shadow prelude names.
- **A module held at most one expression-body function.** `+f a(x) = x + 1`
  followed by another `+f` failed with `expected expression, found KwF 'f'`: an
  expression body runs to the end of the expression, and the `+` opening the
  *next item* was consumed as an addition operator whose right side was the
  keyword. So the form worked only for the last item in a file — which is why
  every example that used it happened to have one. `parse_expr_bp` now stops at
  a `+` that begins a line and is followed by a declaration keyword. Found by
  running `README.md`'s own second example, which had never been executed.
- **All four AI blocks reported the token kind instead of the word, in the
  same place.** Each spec section promises "a field the parser does not
  recognise is an error naming it", and each block has *two* rejection paths:
  an `Ident` arm that names the field, and a generic arm for everything else.
  The recognised fields are keywords, so any wrong word that happens to be a
  keyword lands in the generic arm — `agent A { brain: x }` said "unknown agent
  field `brain`" and `agent A { handle: x }` said "found KwHandle", the same
  mistake reported two ways with only one of them usable. `evolve` named
  nothing at all and `kb` named the alternatives but not the word. All four now
  name it and list the valid fields. Found by checking one block; the test that
  pins it caught the fourth, `swarm`, which I had not looked at. The `evolve`
  test also parses each of the seven fields it advertises, so the list cannot
  become a second wrong claim.
- **The sigil-letter diagnostic blamed the binding keyword.** `var v = 3` gave
  `unresolved name: `var`` — the statement was never recognised as a binding, so
  the keyword fell through to expression position and the error landed on the
  one token written correctly.

### 7. Unsound, under a comment saying otherwise

A shape the other six cannot produce, because it lives in `unsafe` code that no
test exercises and no checker reads. All four are in
`RecursiveMachineIntelligence/src/runtime/memory_pool.rs` — 9 `unsafe` blocks
and 4 `unsafe impl`, compiled unconditionally, which `SECURITY_AUDIT.md`
described as "1 audited `unsafe` in lib.rs, 3 in the CUDA FFI shim — all
reviewed, FFI-boundary only".

- **`TensorBuffer::Drop` freed a layout the allocator never handed out.**
  `from_vec` forgot the `Vec`'s capacity; `Drop` rebuilt it as
  `from_raw_parts(ptr, len, len)`. Capacity differs from length in the
  *ordinary* case — `Vec::with_capacity(64)` with three pushes, or any growth
  pattern — so this was undefined behaviour **on the normal path**, under a
  SAFETY comment asserting "the pointer, len, and capacity match what was
  passed to `mem::forget`".
- **`TensorBuffer::slice` added `offset + len` unchecked.**
  `slice(usize::MAX, 1)` wraps to 0, passes the `> self.len` bound, and offsets
  the pointer by `usize::MAX`. Two plain integers through a safe public method.
- **`Slab::new` with zero capacity** called `alloc_zeroed` on a **zero-sized
  layout** — documented UB, at *construction*, before any allocation was
  requested. Reachable in one call: `PoolConfig`'s fields are public and
  `with_config` passes `initial_capacity` straight through. `growth_factor:
  0.0` reaches the same place via `alloc()`, since a float-to-int cast
  saturates to 0.
- **`Slab::new`'s `size * capacity` was unchecked.** Measured rather than
  assumed: the process **aborts** (`memory allocation of 2305843009213693960
  bytes failed`, exit `0xc0000409`), because `free_list` wants one index per
  claimed block and fails first. A crash from a library constructor, not an
  out-of-bounds access — I wrote "out-of-bounds" first and corrected it after
  running it, and the two deserve different words.

All four have regression tests, each verified by removing its guard. The
overflow test needed sharpening for the same reason the rest of this document
exists: its first version used `usize::MAX / 32`, whose wrapped product
`Layout::from_size_align` rejects anyway, so it passed with the guard removed
and pinned nothing. It now uses `2^58 + 1`, where the product wraps to exactly
64 — a layout the allocator accepts.

Two things left for whoever owns that crate. The four `unsafe impl Send`/`Sync`
are defensible under the refcount discipline, but the counter is read `Relaxed`
while gating mutation, which is a synchronisation decision rather than a
counter. And `compute/cuda_full.rs` holds 16 more `unsafe` blocks in a file no
`mod` declaration references, so it never compiles — dead code that reads as
reviewed and that one line silently activates.

**What made this findable was not reading the code.** It was that a document
claimed the code had been reviewed, and the claim was specific enough to check.

---

---

## What the compiler actually is

Corrected facts, each verified by running it. Several documents said otherwise.

**The effect system is the security boundary.** There are 17 built-in kinds
(§11.2). A function acquires one three ways: by annotation, through a capability
handle (`io.println(…)` — the receiver names the capability), or by calling a
recognised builtin by bare name. A `pub` function must declare what it performs;
a private one infers silently and its effects reach its public callers.
Annotation is an **upper bound** — over-declaring is deliberately allowed and
never warned about. `handle … with` discharges an effect **per block, not per
function**, and a handler's own effects are attributed honestly.

**Handlers resume, single-shot and implicitly** — the arm's value becomes the
operation's value and the body continues — and an arm may `ret` instead, which
aborts the handled block only and leaves the enclosing function running. *Multi-shot* resumption is
deliberately absent — decided 2026-08-19, `MAGE_SPEC.md` §11.6 (item 10).

What it does *not* have: no effect hierarchy (`/ io` grants nothing else), no
per-file or per-module capability grants, no effect polymorphism.

**There is no module system.** `use` parses and is discarded; it now warns. The
library surface is **global** — 31 vocabulary combinators
(`resolve::VOCABULARY`), 20 capability namespaces (`hir::CAPABILITY_NAMESPACES`)
and the builtin functions are in scope everywhere, with no import and no tokens
spent on one. For a language optimising for token efficiency that is plausibly
right rather than a gap, but it was written down nowhere — and it is what makes
a hand-written `stdlib/` unreachable by construction.

**36 of 126 `.mg` files were not MAGE.** `stdlib/` is 25 files, 4,402 lines, and
all of it Rust — read by nothing, checked by nothing, and resolvable-around
because imports are nominal. It now carries a `README.md` saying what it is and
the ordered prerequisites for making it real. `check-mg-sources.sh` is the
consumer it never had; its sketch list only shrinks, and is down from 36 to 30.
Only `stdlib/` and four `framewerx` files remain.

---

## Traps, so you do not repeat them

- **`${#arr[@]}` on a never-assigned associative array is *unbound*.** Under
  `set -u`, `declare -A ACTUAL` followed by a `read` loop that assigns nothing
  makes `[ "${#ACTUAL[@]}" -eq 0 ]` abort the line — so the empty-input guard
  in `check-doc-counts.sh` never ran, and the script went on to report
  "documentation disagrees with measurement" for all 74 claims when what had
  actually happened was that nothing was measured. Count with a plain integer
  assigned before the loop, so it is always bound. Corollary, and the reason
  this mattered more than the syntax: **a checker must distinguish "wrong" from
  "not checked"**, because the two have opposite remedies — edit the doc versus
  run the suite — and reporting the wrong one sends someone to change numbers
  that were correct.
- **`grep -c` exits 1 when the count is zero.** `cargo clippy … | grep -c
  '^warning' && git commit …` printed `0` and then did not commit: a clean
  result is a *failure* exit for `grep`, so the `&&` chain stopped. Same
  family as the `grep -q` pipefail trap below, opposite direction.
- **Editing a shell script while it runs.** `bash` reads a script
  incrementally, so an edit that changes byte offsets can make the running
  copy execute a *fragment* of a line. The symptom is a nonsense error naming
  a line that is a comment — `test-all.sh: line 160: a: command not found`.
  Nothing was wrong with the script; the run was reading a file that moved
  under it. Wait for the run, or copy the script first.
- **`cmd | grep -q` under `set -o pipefail`.** `grep -q` exits at the first
  match, the writer takes SIGPIPE, and the pipeline reports failure — so every
  match reads as no-match. Cost two separate bugs, the second *despite a comment
  about it in the first file*. Capture the output, then match.
- **A tool's output means nothing until you know what you pointed it at.** A
  probe loop reported `ok` for every case because the shell was already inside
  `prototype/`, so `./prototype/target/release/mage-parse` did not exist and the
  missing-binary message matched no error pattern.
- **`bash` from PowerShell is WSL's**, which cannot open a `C:/…` path; a Windows
  path with backslashes gets read as escapes. Prefer Git's bash and repo-relative
  paths.
- **Shell mode bits do not survive a Windows checkout.** `chmod +x` locally is
  not enough; use `git update-index --chmod=+x`, and invoke via `bash` anyway.
- **MAGE is not Rust.** Struct literals are `@P { x: 1 }`, not `P { x: 1 }`
  (which parses as a *map*). Booleans are `1b`/`0b`. `[T]~` is a type suffix, not
  a value one. Receivers are `self`, not `&self`.
- **`remove_dir_all` then `create_dir_all` on a fixed path is unsound on
  Windows.** Deletion returns while the directory is still open somewhere,
  leaving it delete-pending: it exists, cannot be opened, cannot be recreated,
  and `os error 183` points at the creation rather than the deletion that caused
  it.
- **Tight timeouts are flaky under whole-suite load.** Both flaky tests found so
  far were invisible to CI and visible only locally, and both were found by
  running the **whole** suite rather than one crate.
- **`gh pr merge --auto` merges immediately** when the repo has no required
  status checks. It does not queue and does not warn.
- **Regenerate the ontology from a *rebuilt* binary.** One commit shipped a stale
  `MAGE_ONTOLOGY.json` because `--emit-ontology` ran from a binary built before
  the change. `cargo test` builds the test harness, not `mage-parse`.

---

## The pattern worth carrying forward

Almost every bug was found by **running** something. To find the next one, pick
a surface nobody has run, and run it.

Two refinements, both from this session. First, **follow up on what you just
fixed.** Three of the largest finds began as a single question about a change
already made — *does the vendored crate repeat this claim? is the FFI section
alone? is `api.md` alone?* — and each answered yes-and-worse. A defect is
evidence about its neighbourhood, not just about itself.

Second, **a claim specific enough to check is a thread worth pulling even
through code.** The one class of bug not found by running things — four
memory-safety defects in `rmi`'s allocator — was found because a security
document said that code had been reviewed and named what the review concluded.
Vague claims are unfalsifiable and therefore safe to ignore; precise wrong ones
lead somewhere.

Five counter-lessons, each of which cost real time:

**A comment agreeing with your expectation is not evidence.** It is usually what
the author *meant*. Six times now a comment stated the correct rule while the
code beside it did something weaker — "No else → must be unit"; "unifies with any
int width"; "Return the struct type"; the `guard` evaluator admitting its own
fall-through; `resolve.rs` claiming capability handles were "tracked by the
effect system"; and a parser comment saying `!`-as-break was "handled via context
in statement parsing" when the only other `Bang` arm was the `!` *type*.

**Reading is not verification, including when you wrote it.** An earlier draft
claimed a nested `"` inside an f-string ends it early, and three examples were
written around the workaround. It is false. The claim came from a parse error
whose real cause was never isolated, and it was written *into a document about
not doing that*.

**Fixing a class of bug confers no immunity to committing another instance.** The
misspelled-operation bug was written into the first version of the feature built
specifically to eliminate that class, and shipped past twelve new tests, a green
CI run, and the output pin.

**Verify a test by breaking the fix.** Four tests across these sessions passed
against a build with the fix deleted. One asserted statement counts when the
thing that changes is the *tail expression*; one called `types::check` when the
validation lives in a different pass; one compared a generated file against
output from the same run; and one used an overflow input that `Layout` rejects
for an unrelated reason. Verifying against the wrong entry point is a way to be
green about nothing, and it is invisible unless you break the thing on purpose
and watch. **Every guard added this session was verified this way**, and it is
the single cheapest habit in this document.

**Say what you did not check.** Several instruments here are deliberately
partial — name-level rather than signature-level, four docs rather than every
markdown file, one artifact checked for freshness and another deliberately not.
Each says so where a reader will find it. An instrument that overstates its
reach is the same defect as a document that overstates a measurement, and it is
harder to notice because it arrives wearing a green tick.

---

## Notes on the shape of the work

- Prototype tests **1,066 → 1,202**, all green — checked against the live run, so
  it tracks forward rather than freezing at the session that wrote it. Total
  across five crates **2,915**; documented-count pins **84**, up from 46 — the
  four newest hold `SECURITY_AUDIT.md`'s `unsafe` inventory, a claim that had
  been wrong twice.
- Every typechecker fix has landed without breaking an existing test **except
  one**, where widening `collection_elem` made `sum("hi")` legal and
  `vocab_rejects_non_collection` caught it. That is the datapoint showing the
  suite has teeth rather than merely being large.
- **A bug report is an observation, not a specification.** Three of this
  session's starting items described the symptom correctly and the *rule*
  wrongly — "a `(` after a block" was really any newline; "`unresolved name:
  val`" was one of two different wrong errors. Reproduce before believing.
- `git log` is the real record. The commit messages carry the reasoning,
  including the false starts, the retractions, and the one case where a fix was
  implemented in the wrong direction and reverted.
