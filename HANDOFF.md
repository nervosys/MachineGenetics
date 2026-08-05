# Handoff — 2026-08-04/05

What changed, what state it is in, and what to look at next. Written for someone
picking this up cold.

---

## Where things stand

`master` is green and released. Everything below is verified, not asserted —
each claim has a command beside it.

| | |
|---|---|
| Tests | **2,774** — rmi 1,380 · prototype 1,066 · ribosome 164 · germline 112 · forge 52 |
| CUDA | **1,071 passing** on dual RTX 3090 Ti, driver 610.88 |
| Warnings | 0 compiler, 0 clippy in the four owned crates (`rmi` keeps 2 — vendored) |
| Vulnerabilities | 0 Rust across five lockfiles, 0 npm |
| CI | 10 jobs, green on `master` |
| Release | `v0.3.0`, with the promo video attached as a release asset |

Reproduce all of it:

```sh
scripts/test-all.sh --check-docs          # everything + documentation check
scripts/test-all.sh --cuda --bench --check-docs   # + GPU and the benchmark harnesses
```

---

## What happened, in one paragraph

The session began as a cleanup audit and turned into two distinct pieces of
work. The first built out the Ribosome build engine — multi-language support,
extraction into its own crate along with Germline, a CLI, optional TLS — and
repaired a CI pipeline that had been red since June. The second was accidental:
running documented commands rather than reading them found **six typechecker
bugs, one soundness hole, and five stale documented figures**. The repository's
code was in better shape than the claims about it.

---

## The instruments now in place

These exist because the same failure kept recurring: a claim written once,
never revisited, and quietly wrong. Each fails in **both** directions — a claim
that breaks *and* a known-bad entry that starts passing without being pruned.

| Command | Checks |
|---|---|
| `scripts/test-all.sh --check-docs` | 38 documented test counts against the run that just produced them (51 with `--cuda --bench`) |
| `scripts/check-examples.sh` | which shipped examples typecheck, against a recorded list |
| CI `audit` job | `cargo audit` over all five lockfiles separately |
| CI ontology step | `MAGE_ONTOLOGY.json` matches a fresh `--emit-ontology` |
| CI version step | `mage-parse --version` matches the tool id Ribosome keys on |
| CI dependency guard | `ribosome` depends on no MAGE crate, no `germline`, and no TLS stack by default |

**If you add a measured claim to a document, add it to `CHECKS` in
`scripts/check-doc-counts.sh` in the same commit.** The one figure that stayed
stale after the checker existed was one nobody had listed.

---

## Open items

| # | Item | State |
|---|---|---|
| 1 | **10 of 12 examples do not typecheck** | All share `use std::x;` — Rust's `::` where MAGE wants `.`. A bulk conversion was tried and fails deeper in every file, so this needs per-file judgement about what each demonstrates. Pinned by `check-examples.sh`, so it cannot rot further. **Best next task.** |
| 2 | GPU CI runner | Correctness is verified on the hardware here and recorded. What is missing is a self-hosted runner so `cuda-gpu` runs unattended — an account action, declined once already. |
| 3 | TLS trust posture | The transport seam and a `rustls` implementation exist behind `--features tls`. The posture (pinned self-signed / mutual TLS / public PKI) is deliberately the operator's; `acceptor`/`connector` take your config. |
| 4 | `rmi`'s 2 clippy warnings | Left alone on purpose: vendored, must stay syncable against its own upstream. |
| 5 | RAP error shape | An unknown method returns `{"result":{"error":…}}` — an HTTP-200-shaped success containing an error, not a JSON-RPC `error` member. Fixing it is a client-visible wire change, so it is a decision, not a cleanup. |
| 6 | `int` literal constraint | The fix is a post-hoc check in `default_int_literals`, not a real integer-kind constraint threaded through `unify`. Correct for the programs it rejects; the principled version is larger. |

---

## Traps this session hit, so you do not

- **`cmd | grep -q` under `set -o pipefail`.** `grep -q` exits at the first
  match, the writer takes SIGPIPE, and the pipeline reports failure — so every
  match reads as no-match. Cost two separate bugs, in `purge-video-from-history.sh`
  and later in `check-examples.sh` *despite a comment about it in the first
  file*. Capture the output, then match.
- **`bash` from PowerShell is WSL's**, which cannot open a `C:/…` path; and a
  Windows path with backslashes gets read as escapes. Prefer Git's bash and use
  repo-relative paths.
- **Shell mode bits do not survive a Windows checkout.** `chmod +x` locally is
  not enough; use `git update-index --chmod=+x`, and invoke scripts via `bash`
  anyway.
- **MAGE is not Rust.** Struct literals are `@P { x: 1 }`, not `P { x: 1 }`
  (which parses as a *map*). Booleans are `1b`/`0b` — there is no `true`. I lost
  time to both. `MAGE_ONTOLOGY.json` and `--build=schema` answer these
  authoritatively; read them before guessing.
- **A tool's output means nothing until you know what you pointed it at.**
  `cargo audit` flagged `crossbeam-epoch` in `rmi`, which turned out to be a
  stale artifact in one working copy — `rmi` git-ignores its lockfile, so there
  was no committed pin to be stale. A local finding is not a repository finding.

---

## The pattern worth carrying forward

Six typechecker bugs, and **three of them were places where a comment stated the
correct rule and the code beside it did something weaker** — "No else → must be
unit" returning the branch type; "unifies with any int width" enforcing nothing;
"Return the struct type" returning a fresh variable. A comment agreeing with
your expectation is not evidence. It is usually what the author *meant*.

Equally: every one of the six was found by **running** a documented command,
never by reading documentation. `--version` did not exist. `--fix` was
unreachable in the documented argument order. Nine of thirteen RAP methods named
in the roadmap did not exist. The published ABL wire version was wrong. The
agent-facing capability index omitted the entire core loop.

If you want to find the next one, pick a surface nobody has run and run it.

---

## Notes on the shape of the work

- Every typechecker fix landed without breaking an existing test — **except one**,
  where widening `collection_elem` made `sum("hi")` legal and
  `vocab_rejects_non_collection` caught it. That is the datapoint showing the
  suite has teeth rather than merely being large.
- `git log` is the real record. The commit messages carry the reasoning,
  including the false starts and the two occasions I nearly attributed a
  pre-existing failure to my own change and checked first.
