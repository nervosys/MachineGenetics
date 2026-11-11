# MAGE (Machine Genetics) Language Specification

**Version**: 1.0.0 (Draft)
**Status**: Draft — partially implemented, and **not** reconciled with the
ab-initio changes (optional `;`, layout blocks, no `let`, return/parameter type
inference — roadmap Phase K). Where this document and the prototype disagree,
**the prototype is authoritative**; `mage-parse --build=schema` and
`MAGE_ONTOLOGY.json` are generated from the implementation and cannot drift. See
[DOCS.md](DOCS.md).

---

> Just as DNA encodes biological life through a compact molecular language, MAGE encodes intelligent systems through a compact computational language. It is the genetic code for machines — a language in which AI writes, reasons, optimizes, and evolves itself.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Dual Syntax Modes](#2-dual-syntax-modes)
3. [Lexical Grammar](#3-lexical-grammar)
4. [Syntactic Grammar](#4-syntactic-grammar)
5. [Neural Computation](#5-neural-computation)
6. [Tensor Algebra](#6-tensor-algebra)
7. [Symbolic Reasoning](#7-symbolic-reasoning)
8. [Evolutionary Computation](#8-evolutionary-computation)
9. [Agents and Swarms](#9-agents-and-swarms)
10. [Type System](#10-type-system)
11. [Effect System](#11-effect-system)
12. [Contract System](#12-contract-system)
13. [Ownership and Borrowing](#13-ownership-and-borrowing)
14. [Module System](#14-module-system)
15. [Hardware Acceleration Model](#15-hardware-acceleration-model)
16. [Appendix A: Full Grammar](#appendix-a-full-grammar)
17. [Appendix B: Dual Syntax Mapping Table](#appendix-b-dual-syntax-mapping-table)
18. [Appendix C: Operator Precedence](#appendix-c-operator-precedence)
19. [Appendix D: Agent Mode Symbol Reference](#appendix-d-Agent-mode-symbol-reference)

---

## 1. Introduction

MAGE (Machine Genetics) is a systems programming language designed for the age of artificial intelligence. It combines the safety and performance model of Rust with modern language design, first-class primitives for neural computation, symbolic reasoning, evolutionary optimization, and multi-agent coordination — all within a dual-syntax system that serves both human programmers and AI agents.

### 1.1 Design Principles

1. **Intelligence is a first-class construct.** Neural networks, knowledge bases, rule engines, and evolutionary algorithms are language-level declarations, not library calls. The compiler understands their structure, verifies their types, and targets optimal hardware.

2. **Dual encoding.** Human mode uses clear, modern keywords (`val`, `var`, `data`, `extend`, `guard`, `defer`) that improve on Rust's conventions while remaining instantly readable. Agent mode compresses every concept into minimal symbols — Greek letters for AI constructs, mathematical operators for tensor algebra — achieving the density of hexadecimal applied to intelligence.

3. **Safety without ceremony.** Ownership, borrowing, and lifetimes are enforced but fully inferred. No lifetime annotations, no `PhantomData`, no `Pin`. The Safety Knowledge Base (SKB) encodes 255 rules across eight databases (*designed, not built: no compiler stage consults them — see §7.3*). In agent mode, **all safety constructs are handled by the compiler and SKB** — `unsafe` blocks, lifetime annotations, `Send`/`Sync` bounds, and `Pin<T>` are entirely elided from the language surface, maximizing token efficiency while the compiler maintains full safety guarantees.

4. **Effects make side effects visible.** Every function declares its effects (`/ io`, `/ gpu`, `/ llm`). Pure functions have no annotation. Algebraic effect handlers provide structured concurrency and composable I/O.

5. **Hardware-aware by default.** Tensor operations dispatch to the best available backend (CPU SIMD → GPU → NPU) at compile time. Shape checking is static. Autograd is built into the compiler.

6. **Self-improving.** Evolutionary computation blocks (`evolve`) enable programs to optimize their own parameters, architectures, and strategies through genetic algorithms with compile-time fitness verification.

7. **Neurosymbolic fusion.** Agents combine neural reasoning (LLM, vision, RL) with symbolic knowledge (rules, facts, queries) in a single coherent type system. No impedance mismatch between statistical and logical AI.

### 1.2 Notation

This specification uses Extended Backus-Naur Form (EBNF):

```
A B               Sequence
A | B             Alternation
[ A ]             Optional (zero or one)
{ A }             Repetition (zero or more)
'literal'         Terminal
UPPER_CASE        Non-terminal
```

**LL(1) property**: All productions are LL(1)-parseable. No backtracking.

**Encoding**: Source files are UTF-8. Agent mode uses Unicode symbols from Greek, Mathematical Operators, and Arrows blocks.

**File extension**: `.mg` (Machine Genetic code).

---

## 2. Dual Syntax Modes

MAGE supports two interchangeable surface syntaxes that parse to the same AST:

| Mode      | Purpose                              | Measured cost, relative to Human (§2.4) |
| --------- | ------------------------------------ | ------- |
| **Human** | Human-readable, terse keywords       | 1       |
| **Agent** | Symbol-compressed                    | 0.96 in bytes; 1.00 in MAGE tokens, cl100k tokens and learner bits |

*This column said "~3×" density for Agent mode. Measured over the 106-file
corpus (plan S2, §2.4), the sigil surface saves 3.8% of bytes and nothing in any
token unit, so the figure was wrong and is replaced by the measurement.*

No pragma selects a mode. The lexer has one keyword table, so every file may use the spellings of either mode, or both (§3.1); the formatter decides which one it prints.

Both modes print the same AST, so converting between them (`mage-parse --fmt-expand` and `--fmt-compact`) loses nothing the compiler sees, and every `.mg` file in the tree round-trips through both. It is not byte-for-byte: comments are dropped by the lexer, so neither printer keeps them. The compiler accepts both in the same project.

### 2.1 Human mode Keywords

Human mode uses clear, modern keywords that improve on Rust's conventions. Core declarations use `val`/`var` (instead of `let`/`let mut`), `data` (unifying `struct` and `enum`), and `extend` (instead of `impl`). Collection and wrapper types use lightweight literal syntax: `[T]~` for growable arrays, `?T` for optionals, `T or E` for error unions, `{K: V}` for maps. Modern control constructs include `guard`, `defer`, `|>` pipeline, and `is` pattern checks.

**Core declarations (Rust-compatible):**
`fn`, `val`, `var`, `const`, `data`, `trait`, `extend`, `mod`, `use`,
`pub`, `type`, `static`, `unsafe`, `async`

**Control flow (Rust-compatible):**
`if`, `else`, `for`, `in`, `match`, `loop`, `while`, `break`, `continue`,
`return`

**Shared with Rust (unchanged):**
`extern`

**Clauses:** `where`

**Async:** `async`, `.await`

**MAGE-unique — AI constructs:**
`net`, `layer`, `tensor`, `param`, `train`, `grad`, `agent`, `kb`, `fact`,
`rule`, `query`, `evolve`, `genome`, `mutate`, `fitness`, `select`, `crossover`,
`rl`, `policy`, `reward`, `effect`, `handle`, `spec`

**Keyword mapping from Rust:**

Human mode preserves Rust keywords 1:1. The only additions are MAGE's AI and effect-system constructs, which have no Rust equivalent:

| Rust           | MAGE Human  | Notes     |
| -------------- | -------------- | --------- |
| `fn`           | `fn`           | Identical |
| `pub`          | `pub`          | Identical |
| `let`           | `val`           | ~~Immutable binding~~ → clearer intent |
| `let mut`       | `var`           | ~~Mutable binding~~ → single keyword   |
| `const`        | `const`        | Identical |
| `struct`        | `data`          | Unified data declaration (record form) |
| `enum`          | `data`          | Unified data declaration (sum form)    |
| `trait`        | `trait`        | Identical |
| `impl`          | `extend`        | Method extension blocks                |
| `mod`          | `mod`          | Identical |
| `use`          | `use`          | Identical |
| `type`         | `type`         | Identical |
| `static`       | `static`       | Retired (E0554): write `const` |
| `unsafe`       | `unsafe`       | Reserved, not yet supported |
| `async`        | `async`        | Written `async name(…)`; `async fn` does not parse |
| `.await`       | `.await`       | Identical |
| `if`           | `if`           | Identical |
| `else`         | `else`         | Identical |
| `match`        | `match`        | Identical |
| `for`          | `for`          | Identical |
| `in`           | `in`           | Identical |
| `while`        | `while`        | Identical |
| `loop`         | `loop`         | Identical |
| `break`        | `break`        | Identical |
| `continue`     | `continue`     | Identical |
| `return`       | `return`       | Identical |
| `where`        | `where`        | Identical |
| `&mut`         | `&!`           | Exclusive borrow; `&mut` does not parse |
| `impl X for Y`  | `impl X for Y`  | Trait implementation; `extend T { … }` adds methods without a trait |

### 2.2 Agent mode Symbols

Agent mode maps every concept to 1-2 characters. Like hexadecimal compresses 4 bits into one symbol, Agent mode compresses high-level concepts into atomic glyphs:

| Concept        | Human          | Agent      | Category       |
| -------------- | -------------- | ---------- | -------------- |
| Function       | `fn`           | `f`        | Declaration    |
| Public         | `pub`          | `+`        | Visibility     |
| Variable       | `val`          | `v`        | Declaration    |
| Mutable        | `var`          | `m`        | Declaration    |
| Constant       | `const`        | `c`        | Declaration    |
| Data (record)  | `data`         | `D`        | Declaration    |
| Data (sum)     | `data`         | `D`        | Declaration    |
| Trait          | `trait`        | `T`        | Declaration    |
| Extend         | `extend`       | `xd`       | Declaration    |
| Module         | `mod`          | `M`        | Declaration    |
| Import         | `use`          | `u`        | Declaration    |
| Type alias     | `type`         | `Y`        | Declaration    |
| Static (retired in plan 9.2, E0554: write `C NAME: T = value;`) | `static` | `Z` | Declaration |
| Neural net     | `net`          | `Ψ` lexed only (D.1) | AI — Neural    |
| Layer          | `layer`        | `λ` lexed only (D.1) | AI — Neural    |
| Tensor         | `tensor`       | `Φ` lexed only (D.1) | AI — Algebra   |
| Parameter      | `param`        | `Π` lexed only (D.1) | AI — Algebra   |
| Train          | `train`        | `Θ` lexed only (D.1) | AI — Learning  |
| Gradient       | `grad`         | `∇`        | AI — Algebra   |
| Agent          | `agent`        | `α` lexed only (D.1) | AI — Agent     |
| Swarm          | `swarm`        | `sw`       | AI — Agent     |
| Knowledge base | `kb`           | `κ` lexed only (D.1) | AI — Symbolic  |
| Rule           | `rule`         | `ρ` lexed only (D.1) | AI — Symbolic  |
| Fact           | `fact`         | `fact`     | AI — Symbolic  |
| Evolve         | `evolve`       | `Ω` lexed only (D.1) | AI — Evolution |
| Genome         | `genome`       | `Γ` lexed only (D.1) | AI — Evolution |
| Fitness        | `fitness`      | `φ` lexed only (D.1) | AI — Evolution |
| Policy         | `policy`       | `Ξ` lexed only (D.1) | AI — RL        |
| Reward         | `reward`       | `reward`   | AI — RL        |
| If             | `if`           | `?`        | Control        |
| Else           | `else`         | `:`        | Control        |
| Match          | `match`        | `?=`       | Control        |
| For            | `for`          | `@`        | Control        |
| In (for sep)   | `in`           | `:`        | Control        |
| Loop           | `loop`         | `@@`       | Control        |
| While          | `while`        | `@w`       | Control        |
| Break          | `break`        | `!`        | Control        |
| Continue       | `continue`     | `>>`       | Control        |
| Return         | `return`       | `ret`      | Control        |
| Effect         | `effect`       | `fx`       | Effects        |
| Handle         | `handle`       | `hx`       | Effects        |
| Spec           | `spec`         | `sp`       | Contracts      |
| Extern (reserved, not yet supported) | `extern` | `xn` | FFI |
| Await          | `.await`       | `.await`   | Async          |
| Async          | `async`        | `af`       | Async          |
| Unsafe (reserved, not yet supported) | `unsafe` | *(elided)* | Safety→SKB |
| Where          | `where`        | `~>`       | Clause         |
| Mutable ref    | `&!`           | `&!`       | Type           |
| True / False   | `1b`/`0b`      | `1b`/`0b`  | Literal        |
| Matmul         | `⊗`            | `⊗`        | Tensor op      |
| Hadamard       | `⊙`            | `⊙`        | Tensor op      |
| Transpose      | `⊤`            | `⊤`        | Tensor op      |
| Flatten        | `.flatten()`   | `⊥`        | Tensor op      |
| String         | `String`       | `s`        | Type           |
| `&str`         | `&str`         | `&s`       | Type           |
| `[T]~`       | `[T]~`       | `[T]~`     | Type           |
| `?T`    | `?T`    | `?T`       | Type           |
| `Result<T,E>`  | `Result<T,E>`  | `R[T,E]`   | Type           |
| `^T`       | `^T`       | `^T`       | Type           |
| `{K: V}` | `{K: V}` | `{K:V}`    | Type           |
| Path separator | `::`           | `.`        | Path           |

See [Appendix D](#appendix-d-Agent-mode-symbol-reference) for the complete symbol table.

---

### 2.3 One flat namespace

**MAGE has no module system, and this is the design rather than a gap.** Every
name — the standard vocabulary of §8, the capability namespaces of §11, and
every item a program declares — lives in one namespace and is in scope
everywhere. There is no import, no path qualification, and no visibility
boundary between files.

The reason is the language's premise. An import is pure overhead in tokens: it
names something the compiler already knows, costs a line at the top of every
file, and buys separation that matters when a library is large and unstable. The
standard library here is **31 words** (§8) plus **20 capability namespaces**
(§11.3), both fixed and both published in `MAGE_ONTOLOGY.json`. An agent
generating a program should not spend tokens re-deriving what it can already
reach.

What follows from that:

- `pub` (`+`) controls whether an item is part of a module's published surface
  for effect purposes — a `pub` function must declare its effects (§11.4) — not
  whether another file can see it. Everything can see everything.
- **`use` is an error.** It parses, so a Rust-shaped import gets a diagnostic
  naming this section rather than a syntax error, but it cannot bring anything
  into scope and no longer typechecks. It was a warning until 2026-08-19, while
  this decision was still open.
- `mod` (`M`) likewise declares a name and introduces no scope.
- There is no `stdlib/`. A directory of that name held 25 sketches in Rust
  syntax, read by nothing; it was deleted when this decision was made.

**If this is ever revisited**, the cost is not the parser — `use` and `mod`
already parse. It is that every name in every published table becomes
path-qualified, and every agent-facing document that teaches a bare name has to
teach a path instead.

### 2.4 Which surface is cheaper, by unit (plan S2)

Both surfaces print every `.mg` file losslessly (`mage-parse --fmt-compact` is
the sigil surface, `--fmt-expand` the keyword surface). Each is measured over the
106 tracked files in four units. Totals are over all 106 files, except bits,
which are over the 21 held-out files. Ratio is sigil over keyword, so below 1
means the sigil surface is cheaper.

| unit | sigil | keyword | sigil / keyword | per-file median |
| --- | ---: | ---: | ---: | ---: |
| UTF-8 bytes | 93,331 | 96,983 | 0.962 | 0.961 |
| MAGE lexer tokens (`--count-tokens`) | 22,807 | 22,765 | 1.002 | 1.000 |
| cl100k BPE tokens | 29,560 | 29,498 | 1.002 | 1.000 |
| learner bits, held-out (`ng:4:14:0.02`) | 60,985 | 60,977 | 1.000 | 1.002 |

Learner bits are bpb × bytes over the held-out files: sigil 3.2140 × 18,975 B,
keyword 3.0867 × 19,755 B. The per-file median for bits is over the 21 held-out
files only, one learner run per file.

Reproduce with:

```sh
cargo build --release --manifest-path prototype/Cargo.toml
cargo build --release --manifest-path arena/Cargo.toml
TOKENS_OF=<agentic-eval tokens_of binary, built with --features real-tokens> \
  python scripts/surface-units.py --json surface-units.json
```

`TOKENS_OF` is optional. Without it, the script falls back to Python `tiktoken`
and, failing that, reports the BPE unit as unavailable rather than estimating it.

**Reading.** The sigil surface is cheaper in bytes, by 3.8% in total. It is not
cheaper in any token unit. In MAGE lexer tokens and in cl100k BPE tokens it is
0.2% more expensive, because `f` and `fn` are each one token. In learner bits it
ties with the keyword surface to within 0.01%. The table in §2 said ~3× agent
density for the sigil surface, and now gives these figures instead. On this
corpus, the saving is in bytes only.

**Caveats.**
- One learner configuration, `ng:4:14:0.02`, one run. The learner is
  deterministic, so no seed variance was measured, and a different learner could
  rank the surfaces differently.
- Train and held-out come from one small corpus: about 75 KB of training text
  and 19 KB of held-out text, split by sorted path, every 5th file held out. The
  files share authors and style, so held-out is not independent of training.
- Training uses whole 4 KB steps. Sigil trained on 73,728 of 74,356 bytes and
  keyword on 76,800 of 77,228 bytes.
- `prototype/examples/sandboxed_agent.mg` holds only comments, and the lexer
  drops comments, so both surfaces are empty for it. It contributes 0 bytes and
  0 bits, and is left out of the per-file ratios. Neither surface keeps comments.

---

## 3. Lexical Grammar

This section states what `prototype/src/lexer.rs` does. The lexer is lax: it
produces a token for whatever it scans and leaves checking to the parser and
the checker, so where a form below is described as accepted, nothing here
promises it is meaningful. `kb/lexicon.mg` is the machine-readable statement of
the reserved spellings; tests hold both it and §3.3 and §3.6 of this document to
`lexer::KEYWORDS` and to the lexer.

### 3.1 Source Encoding

```
source_file = { token | whitespace | comment }* EOF ;
```

A source file is UTF-8 text. The lexer scans bytes. Any character that no rule
below takes becomes an `Error` token covering that whole character; the lexer
does not stop. A byte-order mark is not recognised, so it lexes as an `Error`
token. The lexer reads no pragma: it has one keyword table, so Human and Agent
spellings (§2) lex alike and may be mixed in one file.

### 3.2 Whitespace and Comments

```
whitespace    = ( ' ' | '\t' | '\n' | '\r' )+ ;
comment       = line_comment | block_comment ;
line_comment  = '//' { any_char - '\n' }* ;
block_comment = '/*' { any_char | block_comment }* [ '*/' ] ;  /* nestable */
```

Whitespace and comments are dropped before parsing. A line comment ends before
the newline. A block comment that is never closed runs to the end of the file.

### 3.3 Keywords

A spelling is listed here when it is reserved by `lexer::KEYWORDS`, or is half
of an `abbreviates` pair in `kb/lexicon.mg`, or lexes as one of the `Kw*` tokens
(the Greek sigils and the `@` contracts). Every other spelling the lexer
produces is a literal (§3.5) or an operator or punctuation mark (§3.6). Quoted
spellings in the blocks of this subsection are exactly that set.

A reserved word is a whole identifier: `fnord` is an identifier, not `fn`
followed by `ord`. Both modes' spellings are keywords in every file.

**Abbreviation pairs.** Each production is named for the Human spelling; its
first alternative is the Agent sigil the formatter prints in Agent mode, its
second the Human spelling. `:` abbreviates both `in` (after a `for` pattern) and
`else` (after an `if` block); which is read depends on the grammar. `R`, `cons`, `topo`, `consensus` and `topology` are not reserved: they lex as
identifiers, and the formatter prints the first three for `or`, `consensus` and
`topology`. (`or` is reserved.)

```
fn       = 'f'    | 'fn' ;
pub      = '+'    | 'pub' ;
struct   = 'S'    | 'struct' ;
enum     = 'E'    | 'enum' ;
trait    = 'T'    | 'trait' ;
impl     = 'I'    | 'impl' ;
const    = 'C'    | 'const' ;
data     = 'D'    | 'data' ;
val      = 'v'    | 'val' ;
var      = 'm'    | 'var' ;
async    = 'af'   | 'async' ;
return   = 'ret'  | 'return' ;
effect   = 'fx'   | 'effect' ;
handle   = 'hx'   | 'handle' ;
spec     = 'sp'   | 'spec' ;
guard    = 'gd'   | 'guard' ;
defer    = 'df'   | 'defer' ;
swarm    = 'sw'   | 'swarm' ;
extend   = 'xd'   | 'extend' ;
mod      = 'M'    | 'mod' ;
use      = 'u'    | 'use' ;
type     = 'Y'    | 'type' ;
static   = 'Z'    | 'static' ;
extern   = 'xn'   | 'extern' ;
if       = '?'    | 'if' ;
match    = '?='   | 'match' ;
for      = '@'    | 'for' ;
in       = ':'    | 'in' ;
else     = ':'    | 'else' ;
while    = '@w'   | 'while' ;
loop     = '@@'   | 'loop' ;
break    = '!'    | 'break' ;
continue = '>>'   | 'continue' ;
where    = '~>'   | 'where' ;
or       = 'R'    | 'or' ;
consensus = 'cons' | 'consensus' ;
topology  = 'topo' | 'topology' ;
```

**Keywords with no abbreviation.** These have one spelling, used in both modes.
`mut` lexes as the same token as `m`, and `let` is its own token.

```
binding_keyword = 'let' | 'mut' | 'uf' | 'U' ;
agent_keyword   = 'agent' | 'unsafe' ;
value_keyword   = 'is' | 'Ok' | 'Err' | 'Some' | 'None' | '_' | '_T' ;
swarm_keyword   = 'swarm_map_reduce' | 'swarm_pipeline' | 'swarm_saga'
                | 'swarm_fan_out' | 'swarm_race' | 'pipeline'
                | 'grammar_extension' ;
neural_keyword  = 'net' | 'layer' | 'tensor' | 'param' | 'train' | 'grad'
                | 'forward' ;
symbolic_keyword = 'kb' | 'fact' | 'rule' | 'query' ;
evolution_keyword = 'evolve' | 'genome' | 'mutate' | 'fitness' | 'select'
                | 'crossover' | 'population' | 'generations' ;
rl_keyword      = 'rl' | 'policy' | 'reward' ;
```

**Sigils with no Human spelling.** The Greek letters are single tokens, each its
own kind, and the five `@` contracts are single tokens that must not be followed
by a letter, digit or `_` (`@require` is `@` then an identifier).

```
greek_sigil = 'Ψ' | 'λ' | 'Φ' | 'Π' | 'Θ' | '∇' | 'α' | 'Σ' | 'κ' | 'ρ'
            | 'Ω' | 'Γ' | 'φ' | 'Ξ' | 'μ' | 'χ' ;
contract    = '@req' | '@ens' | '@inv' | '@fx' | '@perf' ;
```

The lexer has no keyword `true`, `false`, `as`, `self`, `Self`, `crate` or
`super`; they are identifiers, and `1b` and `0b` are the booleans (§3.5).

### 3.4 Identifiers

```
identifier = ID_START { ID_CONTINUE }* ;
ID_START   = 'A'..'Z' | 'a'..'z' | '_' ;
ID_CONTINUE = ID_START | '0'..'9' ;
```

Identifiers are ASCII. A word that is in §3.3 is a keyword, not an identifier;
the prefixes `f"`, `p"` and `ep"` begin a string (§3.5). There are no raw
identifiers.

### 3.5 Literals

```
literal = int_literal | float_literal | bool_literal | string_literal
        | format_string | print_string | eprint_string | char_literal ;

/* Integer literals */
int_literal   = dec_literal | hex_literal | oct_literal | bin_literal ;
dec_literal   = DEC_DIGIT { DEC_DIGIT | '_' }* [ int_suffix ] ;
hex_literal   = ( '0x' | '0X' ) { HEX_DIGIT | '_' }* [ int_suffix ] ;
oct_literal   = '0o' { OCT_DIGIT | '_' }* [ int_suffix ] ;
bin_literal   = '0b' ( BIN_DIGIT | '_' ) { BIN_DIGIT | '_' }* [ int_suffix ] ;
int_suffix    = 'i8' | 'i16' | 'i32' | 'i64' | 'i128' | 'isize'
              | 'u8' | 'u16' | 'u32' | 'u64' | 'u128' | 'usize' ;

/* Float literals */
float_literal = DEC_DIGIT { DEC_DIGIT | '_' }* [ '.' DEC_DIGIT { DEC_DIGIT | '_' }* ]
                [ exponent ] [ float_suffix ] ;   /* a fraction or an exponent is required */
exponent      = ( 'e' | 'E' ) [ '+' | '-' ] { DEC_DIGIT | '_' }* ;
float_suffix  = 'f32' | 'f64' ;

/* Boolean literals: the same in both modes */
bool_literal  = '1b' | '0b' ;

/* String literals */
string_literal = '"' { any_char | '\\' any_char }* '"' ;
format_string  = 'f"'  { any_char | '\\' any_char | interpolation }* '"' ;
print_string   = 'p"'  { any_char | '\\' any_char | interpolation }* '"' ;
eprint_string  = 'ep"' { any_char | '\\' any_char | interpolation }* '"' ;
interpolation  = '{' { any_char }* '}' ;          /* nesting braces; '{{' is a literal brace */

/* Character literals */
char_literal   = "'" ( any_char | '\\' any_char ) [ "'" ] ;
```

Integer notes. A suffix is taken only when the next character is not a letter,
digit or `_`. `0b` followed by `0`, `1` or `_` is a binary integer; `0b` and
`1b` with no such digit are the booleans (`0b1` is the integer 1, `0b` is
false). A decimal number is a float as soon as it has a fractional part (a `.`
followed by a digit, so `1.` is an integer and then a `.`) or an exponent (`1e5`
is a float). A float suffix is accepted only on a float, so `1f32` is the integer
`1` and then the identifier `f32`. The lexer does not check the range of the value, that a prefix is followed by
digits, or that an exponent has digits.

String notes. A backslash makes the next character part of the literal, so
`\"` does not end it; the lexer checks nothing else about escapes, a string may
contain newlines, and one that is never closed runs to the end of the file. In
`f"…"`, `p"…"` and `ep"…"` a `"` inside a `{…}` group does not end the string.
The evaluator decodes the escapes `\n`, `\t`, `\r`, `\0`, `\\`, `\"` and `\'`
in string and character literals; any other backslash pair is kept as written,
backslash included. There are no raw strings, byte literals or byte strings, and
no `\x` or `\u{…}` escapes. A character literal's closing quote is not required
by the lexer.

A literal without a suffix takes its width from where it is used: `1` is any
integer or float type and `1.5` either float type (`val a: f32 = 1.5`). Left open by its
uses, an integer literal is `i32` and a float literal `f64`. A suffix fixes the
type, so `val x: f64 = 1.5f32` is refused.

There is no tensor literal token. `tensor` is a keyword (§3.3) and a tensor
value is written as a list (§6.5).

### 3.6 Operators and Punctuation

A listed spelling is one token; where several listed spellings begin alike the
longest one is taken. The spellings that are also keywords (`+`, `?`, `?=`, `@`,
`@@`, `@w`, `:`, `!`, `>>`, `~>`) are in §3.3 and are not repeated here.

```
/* Arithmetic */
MINUS = '-' ;  STAR = '*' ;  SLASH = '/' ;  PERCENT = '%' ;

/* Comparison */
EQ = '==' ;  NEQ = '!=' ;  LT = '<' ;  GT = '>' ;  LE = '<=' ;  GE = '>=' ;

/* Logical and bitwise */
AND = '&&' ;  OR = '||' ;  BIT_AND = '&' ;  BIT_OR = '|' ;  BIT_XOR = '^' ;
SHL = '<<' ;

/* Assignment */
ASSIGN = '=' ;  PLUS_EQ = '+=' ;  MINUS_EQ = '-=' ;  STAR_EQ = '*=' ;
SLASH_EQ = '/=' ;  PERCENT_EQ = '%=' ;  AND_EQ = '&=' ;  OR_EQ = '|=' ;
XOR_EQ = '^=' ;  SHL_EQ = '<<=' ;  SHR_EQ = '>>=' ;

/* Other multi-character operators */
AND_NOT = '&!' ;  AND_TILDE = '&~' ;  PERCENT_NOT = '%!' ;  HASH_TILDE = '#~' ;
PIPE = '|>' ;  TODO = '??' ;  UNIMPLEMENTED = '???' ;  COLON_QUESTION = ':?' ;

/* Tensor symbols */
MATMUL = '⊗' ;  HADAMARD = '⊙' ;  TRANSPOSE = '⊤' ;  FLATTEN = '⊥' ;
TENSOR_PIPELINE = '▸' ;

/* Delimiters */
LPAREN = '(' ;  RPAREN = ')' ;  LBRACE = '{' ;  RBRACE = '}' ;
LBRACK = '[' ;  RBRACK = ']' ;

/* Punctuation */
SEMI = ';' ;  COMMA = ',' ;  DOT = '.' ;  ARROW = '->' ;  FAT_ARROW = '=>' ;
HASH = '#' ;  DOTDOT = '..' ;  DOTDOTEQ = '..=' ;  TILDE = '~' ;  DOLLAR = '$' ;
```

`::`, `.*`, `.T` and `.w` are not tokens: `::` is two `:` tokens, and the others
are a `.` followed by `*`, or by an identifier. `>>` is always one token; the lexer does not split it.

---

## 4. Syntactic Grammar

This section describes what the parser accepts, construct by construct. It
carries no EBNF of its own, because a hand-written grammar in prose drifts from
the parser, and the one that stood here did. **`kb/grammar.mg` is the normative
grammar.** It states the parser's productions as `rhs(Production, "…")` facts
over token-kind names, cites the parser lines each one comes from, records
where the grammar is deliberately broader than the parser (`GAP`), and is held
to the parser by tests. Where a rule below needs to be exact, it names the
production to read there.

Human and Agent spellings (§2, Appendix B) both parse in any file, and the
examples mix them freely. Everything the examples below show is checked: each
block parses and typechecks.

### 4.1 Compilation Unit

A program is a sequence of items (`Module`, `Item`). Every item may be preceded
by attributes and contracts (§4.9) and by a visibility marker. Visibility is
`pub` (Agent: `+`); there is no `pub(crate)`. The item forms are functions,
`struct`, `enum`, `data`, `trait`, `impl`, `extend`, `mod`, `use`, `type`,
`const`, `effect`, `spec`, `agent`, `swarm`, `net`, `kb`, `evolve` and `train`.
`static` is retired (E0554): write `const`.

```mg
pub f one() -> i32 { 1 }
const LIMIT: i32 = 10;
type Id = i32;
f main() -> i32 { one() + LIMIT }
```

The neural, symbolic, evolutionary and agent items (`net`, `kb`, `evolve`,
`train`, `agent`, `swarm`) have their own sections (§5, §7, §8, §9); their
productions are `NetDef`, `KbDef`, `EvolveDef`, `TrainDef`, `AgentDef` and
`SwarmDef`. A block statement can contain a nested function but no other item
(§4.7).

### 4.2 Function Definitions

A function is `f` (or `fn`), a name, optional generics, parameters, an optional
`->` return type, an optional `where` clause, an optional effect annotation
`/ effect, …`, and a body (`FunctionDef`). `af` declares an async function and
`uf` an unsafe one; `async fn` and `async f` do not parse. The body is a block,
`= expr`, or `;` (a bodiless declaration, which is useful inside a trait or
effect and is rejected by the checker on a free function). A return type may
be followed by `or Type`. The `where` clause is introduced by `~>` or `where`
(`WhereClause`), and generics use `<…>` or `[…]` (`Generics`).

```mg
f add(a: i32, b: i32) -> i32 { a + b }
f twice(x: i32) -> i32 = x * 2
f first<T: Copy>(x: T) -> T { x }
f fails(x: i32) -> i32 or i32 { Ok(x) }

f main() -> i32 { add(1, 2) + twice(3) + first(4) }
```

A parameter's type is optional in the grammar (`PaParamHead`), and the receiver
forms are `&self`, `&!self` (exclusive borrow) and `!self` (consume). `&mut
self` does not parse.

**Default arguments.** A parameter may declare a default, and a caller may omit
it. Only *trailing* defaults may be omitted: in `f g(a: i32, b: i32 = 2, c: i32)`
all three arguments are still required, because a default in the middle would
make a positional call ambiguous. The required arity is therefore the position
after the last parameter without a default.

Defaults are evaluated in the callee's environment, left to right, at each call
that omits them — so a later default may refer to an earlier parameter:

```mg
f scaled(a: i32, b: i32 = a * 2) -> i32 { a + b }

f uses() -> i32 {
    v both = scaled(5, 1)   // 6
    v one = scaled(5)       // 15
    both + one              // 21
}
```

An async function is declared with `af`:

```mg
af fetch() -> i32 { 2 }
f main() -> i32 { 0 }
```

### 4.3 Data Types

`data`, `struct` and `enum` declare named types. Fields are always named
except in the sum form, and a declaration has no trailing `;` (see `DataDef`,
`StructDef`, `EnumDef`, `ItEnumVariant`, `PaDataField` and `PaDataVariant` in
`kb/grammar.mg`).

- `data N(name: T, …)` is a record. A field may carry a default (`x: i32 = 0`).
  Tuple-style `data P(i32)` does not parse.
- `data N = V | V(T)` is a sum whose variants carry unnamed payloads.
- `data N` alone declares a unit type.
- `struct N { name: T, … }` has braces, a leading `+` makes a field public, and
  commas are optional.
- `enum N { A, B(T), C { f: T } }` has unit, tuple and record variants. A
  discriminant (`A = 1`) does not parse.

```mg
data P(x: i32, y: i32 = 0)
data Maybe = Nothing | Just(i32)
data Marker
struct Account { +id: i32, bal: i32 }
enum Shape { Dot, Circle(i32), Rect { w: i32, h: i32 } }
f main() -> i32 { 0 }
```

A record value is written `@Name { field: value, … }`, where a bare `field`
stands for `field: field`. The plain `Name { … }` form is not a literal.

```mg
data P(x: i32, y: i32)
f main() -> i32 {
    val p = @P { x: 1, y: 2 }
    val x = 3
    val q = @P { x, y: 4 }
    p.x + q.x
}
```

Single capital letters that are Agent-mode keywords (`S`, `E`, `T`, `I`, `M`,
`U`, `C`) are poor type names; prefer longer ones.

**Traits, `impl` and `extend`.**

`trait N { items }` declares a trait whose methods may have bodies or end in
`;` (`TraitDef`). `impl [generics] Type [for Type] { items }` is the general
implementation block, including the trait form (`ImplBlock`). `extend Type {
items }` adds methods to a type and takes neither generics nor `for`
(`ExtendBlock`); `extend T for X` does not parse.

```mg
trait Area { f area(self) -> i32; }
struct Sq { s: i32 }
impl Area for Sq { f area(self) -> i32 { self.s * self.s } }
extend Sq { f side(self) -> i32 { self.s } }
f main() -> i32 { 0 }
```

### 4.4 Modules and Imports

`mod name { items }` and `mod name;` declare a module (`ModuleDef`). An import
separates its path with `.`, never `::`, and has three forms: `use a.b`,
`use a.b.*` and `use a.{b as c, d}` (`UseDecl`). The parser accepts all three,
and the checker then refuses every one of them, because there is one flat
namespace and no module system (§2.3).

```mg
mod inner { f helper() -> i32 { 1 } }
f main() -> i32 { 0 }
```

**Invalid MAGE today**, the checker refuses an import with E0425:

```mg
use a.b
use a.b.*
use a.{b as c, d}
f main() -> i32 { 0 }
```

### 4.5 Types

A type is one of the following (`Type`, and the `Ty…` productions).

- **Paths and generics.** A name with optional arguments in `<…>` or `[…]`
  (`TyPath`). `Box`, `Rc`, `HashMap` and similar are ordinary path types, not
  keywords. `R[T, E]` is Result (`TyResult`); `Ptr[T]` and `Simd[T, n]` are
  pointer and SIMD types; `dyn T` is a trait object.
- **Sigil prefixes.** `&T` shared reference, `&!T` exclusive reference, `?T`
  optional, and the further prefixes of §3.6 and Appendix B. `&mut T`,
  `*const T` and `*mut T` do not parse.
- **Collections.** `[T]~` growable array, `[T; n]` array, `[T]` slice, `(A, B)`
  tuple, `{K: V}` map, `{T}` set.
- **Functions.** `f(A, B) -> R`.
- **Units.** `f64[m]`, `f32[m / s ^ 2]` (`TyDimensioned`, §10.6).
- **Neural and evolutionary types.** `tensor[T, d, …]` and `param[T, d, …]`,
  where each dimension is an integer literal or a name (`TyTensor`, `TyParam`,
  `TyDim`); `genome[T]`; `policy[A, B]`; and the bare keyword `kb`. The forms
  `Tensor<f32, [2, 3]>`, `Genome<T>`, `Policy<…>`, `LLM` and `KnowledgeBase` are
  not types.

```mg
f g(a: &i32, b: &!i32, c: [i32]~, d: [i32; 3], e: (i32, bool), h: ?i32,
    r: R[i32, bool], t: tensor[f32, 2, 3], m: {i32: bool}, k: f(i32) -> i32,
    bx: Box<i32>, p: param[f32, 4], gn: genome[i32], u: f64[m]) -> i32 { 0 }
f main() -> i32 { 0 }
```

### 4.6 Expressions

Operator precedence is Appendix C and `StOr` … `StPostfix` in `kb/grammar.mg`.
The primary forms are in `PrefixExpr`, and the postfix forms in `StPostfixOp`.

- **Literals and names.** Lexical forms are §3.5; booleans are `1b` and `0b`.
- **Operators.** Binary, unary and compound-assignment operators (`+=`, `<<=`,
  and the other bitwise forms), the pipeline `|>`, and the tensor operators of
  §6. `?` after an expression is the try operator, `e as T` a cast, and
  `e is pattern` a pattern test.
- **Calls and access.** `f(args)`, `e.field`, `e.method(args)`, `e[i]` and
  `e[a..b]`. A method's type arguments take a turbofish, `e.m::<T>()`; the form
  `e.m<T>()` does not parse. `await` is not an operator: `.await` is an ordinary
  field or method access.
- **Aggregates.** Arrays `[a, b]` and `[x; n]`, tuples `(a, b)`, maps
  `{k: v}`, closures `|x: T| e`, and records `@Name { f: e }` (§4.3).
- **Ranges.** `a..b` and `a..=b` parse only in an index, in a `for` head and in
  an `@` loop head; they are not free expressions.
- **Conditionals.** `if c { … } else if c { … } else { … }`, Agent
  `? c { … } : ? c { … } : { … }` (the space in `: ?` matters, Appendix B.3).
  `match e { pattern => expr, … }` (Agent `?=`) takes optional commas between
  arms (`PxIf`, `PxMatchArms`).
- **Loops.** `for p in e { … }`, Agent `@ p : e { … }`, `@ p in e` and
  `@ p of e`; `while c { … }` (`@w c`); `loop { … }` (`@@`). Also
  `break`, `continue` and `return`, each with an optional value where one
  makes sense (`PxFor`).
- **Constructors.** `Some(e)`, `None`, `Ok(e)` and `Err(e)` (`PxSumCtor`).
- **Effects.** `handle { body } with Name { op(args) => expr, … }`
  (`HandleExpr`, §11.5).
- **Gradient.** `grad(expr, name)`; the second argument must be a name (§5.6).

```mg
data P(x: i32, y: i32)
f double(n: i32) -> i32 { n * 2 }
f get() -> ?i32 { Some(1) }
f try_it() -> ?i32 {
    val a = get()?
    Some(a + 1)
}
f main() -> i32 {
    val p = @P { x: 1, y: 2 }
    val xs = [1, 2, 3, 4]
    val table = {1: 2}
    val inc = |n: i32| n + 1
    val mid = xs[1..3]
    val wide = 5 as i64
    val d = p.x |> double
    val flag = p.x == 1 && 1b || 0b
    val big = if p.x > 1 { 1 } else if p.y > 1 { 2 } else { 3 }
    var total = 0
    for x in xs { total += x }
    for i in 0..3 { total += i }
    @ x : xs { total += x }
    @w total < 100 { total += 1 }
    val t = ?= total { 0 => 1, _ => 2 }
    inc(d) + big + t
}
```

```mg
f main() -> i32 {
    var n = 0
    @@ {
        n += 1
        if n > 3 { break }
    }
    val len = [1].len::<i32>()
    n
}
```

### 4.7 Statements

A block is `{ statements, optional final expression }` (`Block`, `BlockBody`,
`Stmt`). A statement is a binding, a `guard`, a `defer`, a nested function, or
an expression. The terminating `;` is optional: a newline or the closing brace
ends a statement.

- `val` / `var` (Agent `v` / `m`) bind an immutable and a mutable name, with a
  pattern and an optional type (`StLet`). A keyword spelling is not a binding
  name, so `m`, `v`, `u` and the like are refused with a message.
- `guard cond else { … }` runs the block when `cond` is false, and the block
  must leave the scope. `or` and `:` also introduce the block (`StGuard`).
- `defer expr` runs `expr` when the scope ends (`StDefer`).
- An item inside a block is restricted to a nested `f`.

```mg
f demo(x: i32) -> i32 {
    guard x > 0 else { return 0 }
    defer print("done")
    val y: i32 = x + 1
    f inner(a: i32) -> i32 { a * 2 }
    var total = inner(y)
    total += 1
    total
}
```

### 4.8 Patterns

Patterns appear in `val`, `for`, `match` arms and `is` tests
(`Pattern`). The parser accepts the wildcard `_`, a name, a literal (integer,
float, string, character, boolean), a tuple `(p, q)`, a slice `[h, ..t]`, a
constructor `Name(p, …)` including `Some`, `None`, `Ok` and `Err`
(`PxSumPat`), and a record pattern `@Name { field, field: pattern }`. There are
no or-patterns (`p | q`), range patterns, `&p` patterns or match guards; a
struct pattern without the `@` does not parse.

```mg
f classify(r: ?i32, pair: (i32, i32), xs: [i32]) -> i32 {
    val a = match r { Some(1) => 1, Some(n) => n, None => 0 }
    val (p, q) = pair
    val b = match xs { [h, ..t] => h, _ => 0 }
    val c = match "k" { "k" => 1, _ => 2 }
    a + p + q + b + c
}
```

### 4.9 Attributes and Contracts

An attribute is `@name` with optional parentheses (`Attr`); `#[…]` does not
parse. `@req(…)`, `@ens(…)` and `@inv(…)` are contracts (`Contract`), take an
expression and an optional message string, and bind to the next item (§12).
`@perf`, `@fx` and `@spec` are not item attributes: they are items of a `spec`
body (`SpecDef`, `EfSpecItem`).

```mg
@req(x > 0, "x must be positive")
@ens(result >= x)
pub f lift(x: i32) -> i32 { x + 1 }

@inline
f id(x: i32) -> i32 { x }

spec Pos(x: i32) -> i32 { @req(x > 0) @ens(result > 0) }
f main() -> i32 { lift(1) + id(1) }
```

### 4.10 Integer Arithmetic

Integer `+`, `-`, `*`, `/`, `%`, unary `-` and `abs` **trap on overflow**: the
evaluation fails with an error naming the operation, for example
`integer overflow in 9223372036854775807 + 1`, in every build. Division and remainder by zero
fail the same way.

Modular arithmetic is asked for by name:

```text
wrapping_add(a, b)    wrapping_sub(a, b)    wrapping_mul(a, b)
```

**Why trap.** Before 2026-09-25 the operators were whatever the host
compiler's defaults were: a panic in a debug build and a silent wrap in a
release build, so one program had two meanings. Of the two consistent choices,
trapping is the one that cannot change a program's answer without saying so,
which is the property a language built for provable safety owes its users.
Code that wants wrapping, such as hashing or generated byte streams, says so
at the call site, where a reader can see it.

### 4.11 Fuel

Every evaluation may be given a **fuel** budget. Under one, evaluation is
total: it ends with a value, an error, or exhaustion, and never with a hang or
a crash. Fuel is the unit a search compares candidates' cost in, and the
kernel calibrates it to joules by measurement.

The costs, as `prototype/src/eval.rs` charges them:

| event | cost |
| --- | --- |
| evaluating any expression | 1 |
| a builtin returning a list or string | its length, after the call |
| `range(n)`, `a..b`, `[x; n]` | the length, **before** allocating |
| exceeding the budget | the run fails with `fuel exhausted`, having spent all of it |

Allocation whose size comes from a scalar is charged before it happens,
because charging afterwards would let `range(1000000000000)` exhaust memory
before the budget noticed. Every other builtin's output is bounded by inputs
already paid for, so its after-the-fact charge keeps total allocation within a
constant factor of fuel.

Under fuel, call depth is bounded at 256, and the evaluation runs on a thread
whose stack is sized for that bound. A depth limit means nothing over an
unknown stack; a generated program must fail as a candidate, not abort the
process.

Exhaustion is not a fault in the program, which may have been about to finish.
A search should treat it as *too expensive*, not *wrong*. Without a budget,
evaluation is unbounded, as it always was.

### 4.12 MAGE-core

**MAGE-core** is the checked subset the RSI kernel runs on the loop's behalf:
generated candidates, the reward, and the harness policies after them.
`mage-parse --check --core` requires it.

1. **Every function and method declares a role** (§11.6). A missing role is
   `E0551`: code with no stated ceiling on what it may do.
2. **No `unsafe`**, whether an `unsafe` function or an `unsafe` block.
3. **Only core items**: functions, `struct`/`data`/`enum`, type aliases,
   constants, effect declarations, traits, `impl`/`extend` blocks, and `spec`
   contracts. `use` and `mod` (there is no module system, §2.3), `static`, and
   the domain constructs the kernel does not run (`net`, `train`, `evolve`,
   `kb`, `agent`, `swarm`) are outside it, as `E0552`.

What a role allows is the effect system's business (§11.6). MAGE-core is
structural, so the two cannot disagree about the same fact. The subset is
deliberately small and is expected to grow, one construct at a time, as the
kernel comes to run it.

---

## 5. Neural Computation

MAGE treats neural networks as first-class language constructs. A `net` block declares a network architecture; `layer` statements define its topology; `train` blocks define optimization loops. The compiler verifies shape compatibility, selects hardware targets, and generates optimized kernels.

### 5.1 Network Definition

```mg
net Classifier {
    layer hidden: Linear(784, 256)
    layer act: ReLU(256)
    layer drop: Dropout(0.3)
    layer out: Linear(256, 10)
    forward { out(drop(act(hidden))) }
}
```

**Every layer is named.** The name is how `forward` refers to it, and how a
shape error names the layer it is about. The kind is a **surface name from the
layer map** (`Linear`, `Conv2D`, `Attention`, `Embed`, `Dropout`, `Softmax`,
`ReLU`, `GELU`, `SiLU`, `Sigmoid`, `Tanh`, `Mish`, `Softplus`, `LayerNorm`,
`BatchNorm`, `RMSNorm`, `GroupNorm`, `InstanceNorm`, `MaxPool`, `AvgPool`,
`GlobalAvgPool`, `Unify`, `Resolve`, `Infer`, `Plan`, `Send`, `Recv`, `Spawn`,
`Delegate`, `Hash`, `Typeof`), and it is **case-sensitive**: `layer b: Lienar(…)`
is an error naming the unknown kind, not a silently identity layer.

**Grammar:**

```
net_def = 'net' IDENT '{' { layer_def }* [ forward_def ] '}' ;

layer_def = 'layer' IDENT ':' layer_kind '(' layer_args ')' [ ';' ] ;

layer_kind = IDENT ;   /* a surface name from the layer map, case-sensitive */

forward_def = 'forward' block ;
```

### 5.2 Layer Types

These are the surface names the compiler maps to opcodes. The list is
published in `MAGE_ONTOLOGY.json` under `layer_map`, generated from the same
table the checker uses, so it cannot drift.

| Family | Layers |
| ------ | ------ |
| Dense / conv | `Linear(in, out)`, `Conv2D(ch_in, ch_out, k)` |
| Attention | `Attention(dim, heads)`, `Embed(vocab, dim)` |
| Normalisation | `LayerNorm(dim)`, `BatchNorm(n)`, `RMSNorm(dim)`, `GroupNorm(g, n)`, `InstanceNorm(n)` |
| Pooling | `MaxPool(k)`, `AvgPool(k)`, `GlobalAvgPool()` |
| Regularisation | `Dropout(rate)` |
| Symbolic | `Unify`, `Resolve`, `Infer`, `Plan` |
| Agentic | `Send`, `Recv`, `Spawn`, `Delegate` |
| Utility | `Hash`, `Typeof` |

### 5.3 Activation Functions

Activations are **layers**, not arguments: `ReLU`, `GELU`, `SiLU`, `Sigmoid`,
`Tanh`, `Mish`, `Softplus`, `Softmax`. A `Linear` layer takes no activation
parameter — add the activation as the next layer and name it, so `forward` can
refer to it.

### 5.4 Training Blocks

```mg
net Classifier {
    layer hidden: Linear(784, 256)
    layer out: Linear(256, 10)
    forward { out(hidden) }
}

train mnist_training {
    net: Classifier
    dataset: "mnist"
    optimizer: adam
    loss: cross_entropy
    epochs: 100
    batch_size: 64
    lr_schedule: cosine
    warmup_steps: 500
    weight_decay: 0.01
    clip_grad: 1.0
    seed: 42
}
```

The field naming the network is **`net`**, and the data source is
**`dataset`**. There are no callbacks: a `train` block is declarative, and a
field it does not recognise is an error naming the field rather than a silently
ignored line.

**Grammar:**

```
train_def = 'train' IDENT '{' { train_field }* '}' ;

train_field = ( 'net' | 'dataset' | 'optimizer' | 'loss' | 'epochs'
              | 'batch_size' | 'body' | 'inputs' | 'targets' | 'val_split'
              | 'checkpoint' | 'patience' | 'plateau_patience' | 'lr_factor'
              | 'prompt' | 'max_tokens' | 'temperature' | 'top_k' | 'top_p'
              | 'seed' | 'clip_grad' | 'warmup_steps' | 'lr_schedule'
              | 'weight_decay' | 'tied_embeddings' )
              ':' expression [ ',' ] ;
```

### 5.5 LLM Integration

MAGE provides native types for language model invocation:

```mg
pub fn summarize(text: String) -> String / llm {
    llm.generate(f"Summarize the following text:\n{text}")
}
```

`llm` is a **capability namespace** — in scope everywhere, with nothing to
import. Reaching it is what puts `llm` in the inferred set, and a public
function must declare it. There is no `LLM` type, no `Prompt` type and no
named-argument syntax; a prompt is a string, and an f-string builds it.

Because the call is an effect, it is also *handleable*: wrap it in
`handle { … } with` an effect you declare, and a test never reaches a model.

### 5.6 Autograd

`grad(e, w)` is the derivative of `e` with respect to the variable `w`, and
`∇(e, w)` is the same thing in sigil mode.

```mg
f slope(w: f64) -> f64 { grad(w * w * 3.0 + w * 2.0, w) }
```

`slope(4.0)` is `26.0`, and it is exact: this is the chain rule, not a finite
difference.

**The typing rule.**

```
Γ ⊢ e : f32     Γ ⊢ w : f32     diff(e) ⊑ AlmostEverywhere
──────────────────────────────────────────────────
                  Γ ⊢ grad(e, w) : typeof(w)
```

Three things in that rule are decisions rather than notation:

* **The gradient has the shape of `w`, not of `e`** — which is why `e` must be
  scalar. The derivative of a non-scalar is a Jacobian, a different construct.
* **`w` must be a name.** `grad(loss, x + 1)` asks for a derivative with
  respect to an expression, which is not a question with an answer.
* **`diff(e) ⊑ AlmostEverywhere` is the differentiability obligation**, checked
  by the pass in `prototype/src/differentiable.rs` and *not* by the type
  checker. `diff` is a property of the call graph — an `e` whose only sin is
  calling a function that compares two floats has no derivative, and nothing in
  the expression says so. `NotDifferentiable` is an error naming the reason;
  `Unknown` is a **warning**, because refusing a program the compiler merely
  failed to analyse is worse than saying so.

This program is **invalid**, and this is the diagnostic it gets:

```mg
+f noisy(x: f64) -> f64 / io { println("tick"); x * 2.0 }
f bad(w: f64) -> f64 { grad(noisy(w), w) }
```

> error: in `bad`: `grad` needs an expression with a derivative, and this one
> has none — performs the `IO` effect, so its output is not a function of its
> inputs

**What is implemented, and what is not.** Scalar `f64`/`f32` gradients work and
run. `grad` over `tensor` and `param` is **designed and not built**, and the
type checker refuses it with that sentence rather than accepting a program that
cannot run — the evaluator has no tensor value at all. Nested `grad` is refused
too: a second derivative needs a second level of dual numbers.

The implementation is forward-mode automatic differentiation, and forward mode
specifically because `grad(e, w)` names **one** `w`: one pass gives ∂e/∂w
exactly. Reverse mode is the right algorithm for many parameters at once, which
is what the `train` block of §5.4 does through its own tape.

Alongside it, the `train` block stays declarative: the optimiser, schedule and
loss are fields, and the backward pass is the runtime's business rather than
something the source expresses.

---

## 6. Tensor Algebra

Tensors are first-class types with compile-time shape checking and automatic hardware dispatch.

### 6.1 Tensor Types

The type is `tensor[T, dims…]`, and a learnable parameter is
`param[T, dims…]`. Both are **lowercase keywords**, and the shape is a
comma-separated list inside the same brackets — not a nested `[…]`, and not
`<…>`:

```mg
// Statically shaped tensors, in a signature
f classify(
    image: tensor[f32, 3, 224, 224],     // 3x224x224 image
    weights: param[f32, 512, 256],       // learnable weight matrix
    bias: param[f32, 256],
) -> tensor[f32, 3, 224, 224] {
    image
}

// A dimension may be a name rather than a literal — `B` is a shape variable.
// (`f16` and `bf16` are not type names today; `f32` and `f64` are.)
f batched(x: tensor[f32, B, 512, 512]) -> tensor[f32, B, 512, 512] {
    x
}
```

### 6.2 Tensor Operations

**Design, not implementation.** None of the operators below parse today —
`A @ B` in particular cannot, because `@` is the for-loop sigil. Tensor
computation is expressed through `net` blocks and their layers (§5), where the
shapes *are* checked; the operator surface is recorded here as intent.

| Operation | Intended surface | Description |
| --------- | ---------------- | ----------- |
| Matrix multiply | `A @ B` | [M,K]x[K,N] -> [M,N] |
| Element-wise multiply | `A .* B` | Hadamard product |
| Element-wise add | `A + B` | Broadcast-compatible |
| Transpose | `A.T` | Swap last two dims |
| Reshape / flatten | `A.reshape([2,3])`, `A.flatten()` | |
| Reductions | `A.sum()`, `A.mean(axis: 0)` | |
| Gradient | `grad(loss, w)` | Autograd (§5.6) — scalar only; over tensors it is designed and not built |
| Slice / concat / stack | `A[0..3, ..]`, `cat([A, B], axis: 0)`, `stack([A, B])` | |

### 6.3 Shape Checking

The compiler verifies tensor shape compatibility at compile time:

Shape checking happens where shapes are declared: in a `net` block, across
adjacent layers. A `Linear(784, 256)` followed by a `Linear(128, 10)` is a
shape error naming both layers, and an unknown layer kind is an error rather
than a silent identity — that one cost a real bug (`layer b: Lienar(128, 64)`
lowered to a pass-through and the net trained around it).

Shape *variables* are written as names in the type, and a function generic over
them needs no `const` parameters:

```mg
f linear(
    x: tensor[f32, M, K],
    w: param[f32, K, N],
    b: param[f32, N],
) -> tensor[f32, M, N] {
    x
}
```

### 6.4 Hardware Dispatch

Tensor operations automatically target the best available hardware:

```
Dispatch priority: NPU → GPU (CUDA/ROCm/Metal) → CPU (AVX-512/NEON) → Scalar

Annotations override automatic dispatch:
  @target(gpu)     — force GPU execution
  @target(cpu)     — force CPU execution
  @target(npu)     — force NPU execution
  @precision(f16)  — use half precision
  @precision(bf16) — use bfloat16
```

### 6.5 Tensor Literals

There is **no `tensor!` literal and no constructor function**: `tensor![…]` is
a macro, and MAGE has no macros. A tensor-shaped value is written as a list
literal, and its shape lives in the annotation:

```mg
f identity3() -> [[f64]~]~ {
    [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ]
}

f zeros(n: usize) -> [f64]~ {
    map(range(n), |i| 0.0)
}
```

---

## 7. Symbolic Reasoning

MAGE integrates symbolic AI as language-level constructs: knowledge bases with facts and rules, logical inference, and queryable rule engines.

### 7.1 Knowledge Base Definition

```mg
kb TypeRules {
    // Facts — ground truth assertions.
    fact numeric("i32");
    fact numeric("f64");
    fact unsigned("u32");

    // Rules — a head, its parameters, and a body block of terms.
    rule integer(t: str) { numeric(t) }
    rule castable(t: str) { numeric(t) }
}
```

**A rule is `rule head(params) { body }`** — not Prolog's `head :- body`, which
is a parse error. There is no `query` item inside a `kb` block either; a query
is a *use* of the knowledge base, not a declaration in it.

**Grammar:**

```
kb_def = 'kb' IDENT '{' { kb_item }* '}' ;

kb_item = 'fact' IDENT '(' arg_list ')' ';'
        | 'rule' IDENT '(' param_list ')' [ 'where' literal { ',' literal }* ] block ;

arg_list = ( IDENT | STRING | NUMBER ) { ',' ( IDENT | STRING | NUMBER ) }* ;

block    = '{' literal { '&&' literal }* '}' ;
literal  = [ '!' ] IDENT '(' term { ',' term }* ')'
         | term ( '==' | '!=' ) term ;
term     = IDENT | STRING | NUMBER ;
```

**What a rule means.** A rule derives its head for every way of satisfying
its body.

- **Every identifier in a rule is a logic variable**: the head's parameters,
  and any name that appears only in the body (`y` below). `_` is a fresh
  variable each time it is written. Constants in a rule are string or number
  literals. In a `fact`, an identifier is a constant.
- **The body is one conjunction**: the `where` conditions and the block's
  literals, joined by `&&`.
- **`!p(..)` is negation**: true when no fact matches. It is stratified: a
  predicate is computed only after everything it negates is complete, and a
  predicate that depends on its own negation is an error.
- **`a == b` and `a != b` compare** two variables or constants. They filter
  bindings and match no stored fact.
- **A rule must be safe.** Every head parameter, every named variable in a
  negated literal and every variable in a comparison must appear in a
  positive literal of the body. `_` in a
  negated literal is a wildcard local to it: `!parent(_, x)` holds when no
  fact `parent(y, x)` exists for any `y`.

```mg
kb Family {
    fact parent("ann", "bob");
    fact parent("bob", "cy");
    fact person("ann");
    fact person("bob");
    fact person("cy");
    rule grandparent(x: str, z: str) { parent(x, y) && parent(y, z) }
    rule root(x: str) { person(x) && !parent(_, x) }
}
```

`grandparent("ann", "cy")` and `root("ann")` are derived. A rule or fact the
engine cannot evaluate is **E0570** (`KbRule`), and the message says why: a
body holding something other than predicate calls, an unbound parameter, an
unstratifiable negation, or a fact argument that is not a constant.

**Corrected 2026-10-29.** The engine read only a rule's `where` conditions,
ignored its block, and took a lowercase parameter for a constant, so the first
example in this section derived nothing. A fact argument it could not read was
dropped, changing the fact's arity. Both were silent. The binary form
(`--target=abl-*`) still lowers only the `where` conditions and has no
negation; the two are being made one engine.

### 7.2 Runtime Querying

`kb` is a capability namespace — in scope everywhere, with nothing to import.
It **deliberately attributes no effect**: no built-in kind names a store, and
inventing one would infer an effect that §11.4 then refuses in an annotation.
For a store you want gated, declare an `effect` and call its operations, which
is also what makes it mockable:

```mg
kb TypeRules {
    fact numeric("i32");
    rule castable(t: str) { numeric(t) }
}

effect Rules {
    f castable(from: str, to: str) -> bool;
}

pub fn check_types(from: str, to: str) -> bool / rules {
    Rules.castable(from, to)
}
```

### 7.3 Integration with the Safety Knowledge Base (SKB)

The SKB from the compiler holds **255 rules** across eight databases:
- Ownership (40), Borrow (40), Lifetime (35)
- TypeSafety (40), Concurrency (35), FFI (20)
- AgentElision (30), SwarmSafety (15)

**Corrected 2026-09-02.** This list read "9,157 rules" across six differently
named categories — Ownership and borrowing 2,100, Type safety 1,800,
Concurrency 1,500, FFI safety 1,200, Memory layout 1,300, API contracts 1,257.
That figure appears in four documents with **three mutually inconsistent
decompositions**, and none of them matches the compiler, which serves the
counts above from the rules in `kb/rules/safety.mg`, read by `builtin_rules()` in `prototype/src/skb.rs`, pinned by a
test. `skb/README.md` records where the number came from and what is designed
rather than built.

Agents can query the SKB at compile time:

**Not implemented.** There is no `std::skb`, no `skb` namespace and no
compile-time query API; the block below is **invalid MAGE**, and records the
intent. The reachable form today is the same as §7.2 — declare an effect for
the store and handle it:

```mg
effect Skb {
    f check(category: str, code: str) -> [str]~;
}

pub fn validate_borrow(code: str) -> [str]~ / skb {
    Skb.check("borrow", code)
}
```

---

## 8. Evolutionary Computation

> **Retired in plan 9.2.** The `evolve` item had no callers in any `.mg` file and is now an error (E0554). Write an evolutionary search as an ordinary function, or as an arena agent. The sections below describe the retired form and are kept for reference.

MAGE has first-class support for genetic algorithms, neuroevolution, and evolutionary strategies. The `evolve` block declaratively specifies population, fitness, selection, crossover, and mutation — the compiler generates optimized parallel evolution loops.

### 8.1 Evolve Block

**Retired in plan 9.2 (E0554), so this block is Invalid MAGE today.** Write an evolutionary search as an ordinary function, or as an arena agent.

```mg
evolve NeuralArchSearch {
    genome: [i32]~
    population: 200
    generations: 1000

    fitness { 0.98 }
    select { 8 }
    crossover { 0.7 }
    mutate { 0.02 }
}
```

**Seven fields, and each strategy is a block, not a call.** `fitness`,
`select`, `crossover` and `mutate` take `{ … }` — there is no
`select tournament(k: 8)` form, no `target` field and no `on_generation`
callback, and a field the parser does not recognise is an error naming it.

**Grammar:**

```
evolve_def = 'evolve' IDENT '{' { evolve_field }* '}' ;

evolve_field = 'genome' ':' type [ ';' | ',' ]
             | 'population' ':' expression [ ';' | ',' ]
             | 'generations' ':' expression [ ';' | ',' ]
             | 'fitness' block
             | 'select' block
             | 'crossover' block
             | 'mutate' block ;
```

### 8.2 Genome Types

A genome is an ordinary type. There are **no derive macros** — `#[derive(…)]`
does not parse, and neither does MAGE's own `@d(…)` generate genetic
operators; the `evolve` block's `crossover` and `mutate` blocks are where those
live.

```mg
+S ArchGenome {
    layers: [LayerGene]~,
    learning_rate: f64,
    dropout_rate: f64,
}

+E LayerGene {
    Dense(u32),
    Conv2d(u32, u32),
    Attention(u32, u32),
    Skip,
}
```

### 8.3 Reinforcement Learning

**Not implemented.** `rl`, `policy` and `reward` are reserved words that no
parser arm consumes: an `rl` block is a parse error (`expected item, found
KwRl`), and there is no `std::rl`. What follows records the intent.

**Invalid MAGE today:**

```mg
rl CartPole {
    policy: ppo
    reward: total
    episodes: 10000
}
```

What is reachable today is the ordinary loop: a `net` for the policy, a
`train` block for the optimisation, and `/ gpu` on whatever reaches the
device.

### 8.4 Self-Improvement

The combination of evolutionary computation and neural networks enables **recursive self-improvement**: programs that optimize their own architectures, hyperparameters, and strategies:

**Retired in plan 9.2 (E0554), so this block is Invalid MAGE today.**

```mg
// A MAGE program that evolves its own compiler optimisation passes.
+E OptimizationPass { Inline, Unroll, Vectorize, DeadCode }

f score(passes: [OptimizationPass]~) -> f64 {
    (len(passes) as f64) / 4.0
}

evolve CompilerOptimizer {
    genome: [OptimizationPass]~
    population: 50
    generations: 500

    fitness { score([Inline, Unroll]) }
    select { 4 }
    crossover { 0.6 }
    mutate { 0.05 }
}
```

The `fitness` block is where the recursion lives: it is ordinary MAGE, so it
may compile and measure MAGE — including the compiler itself. What the block
form costs is the callback and the multi-objective `target`; what it buys is
that the search is *declared*, and therefore checkable, rather than assembled
at run time.

---

## 9. Agents and Swarms

Agents are autonomous computational entities that combine neural reasoning, symbolic knowledge, and evolutionary adaptation. MAGE's agent system is built on structured effects and capability-based security.

### 9.1 Agent Definition

**An `agent` block declares a role, not a class.** It has exactly two fields,
both lists of bare identifiers, and it carries **no state and no methods** —
there is no `brain`, no `memory`, no `handle`, and nothing to override:

```mg
agent CodeReviewer {
    capabilities: [llm, io]
    requires_approval: [publish]
}

// The work is an ordinary function. The annotation is what ties it to the
// role: what this function can reach is what the checker enforces.
+f review(code: str) -> str / llm, io {
    v analysis = llm.generate(f"Review this code:\n{code}")
    p"{analysis}"
    analysis
}
```

`capabilities` lists what the role may use; `requires_approval` lists
operations it may *request* but not perform unilaterally. `mage-parse --check`
reports each agent as **Verified** when every name is in the known set and
**Partial** otherwise — a report, not an error.

**Grammar:**

```
agent_def = 'agent' IDENT '{' { agent_field }* '}' ;

agent_field = 'capabilities' ':' '[' [ IDENT { ',' IDENT }* ] ']'
            | 'requires_approval' ':' '[' [ IDENT { ',' IDENT }* ] ']' ;
```

### 9.2 Swarm Definition (First-Class Construct)

Swarms are first-class language constructs that manage a coordinated group of agents:

```mg
agent CodeReviewer { capabilities: [llm] }

swarm ReviewTeam {
    agent: CodeReviewer
    size: 5
    topology: mesh
    consensus: majority
}

// Dispatch and aggregation are code, not fields: `map` fans out, `fold` fans
// in, and the consensus the block *declares* is computed here rather than
// assumed.
f review(file: str) -> bool / llm { len(llm.generate(file)) > 0 }

+f team_verdict(files: [str]~) -> bool / llm {
    v votes = map(files, |file| review(file))
    v yes = len(filter(votes, |vote| vote))
    yes * 2 > len(votes)
}
```

**Four fields, and no behaviour blocks.** There is no `dispatch`, `aggregate`
or `on_failure` — a field the parser does not recognise is an error naming it.

**Grammar:**
```ebnf
swarm_def = 'swarm' IDENT '{' { swarm_field }* '}' ;

swarm_field = 'agent' ':' IDENT [ ';' ]
            | 'size' ':' expr [ ';' ]
            | 'topology' ':' IDENT [ ';' ]   (* star | ring | mesh | broadcast | tree *)
            | 'consensus' ':' IDENT [ ';' ]  (* majority | unanimous | weighted | quorum *)
            ;
```

**Topologies:**

| Topology    | Description                           | Ordering      |
| ----------- | ------------------------------------- | ------------- |
| `star`      | Hub-and-spoke, coordinator routes all | Hub sees all  |
| `ring`      | Sequential pipeline                   | Ordered       |
| `mesh`      | All-to-all, fully connected           | No guarantee  |
| `broadcast` | Simultaneous fan-out to all agents    | Simultaneous  |
| `tree`      | Hierarchical, sub-coordinators        | Level-ordered |

**Consensus strategies:**

| Strategy    | Description                           |
| ----------- | ------------------------------------- |
| `majority`  | > 50% of agents must agree            |
| `unanimous` | All agents must agree                 |
| `weighted`  | Agents vote with configurable weights |
| `quorum`    | Configurable threshold (e.g., 3 of 5) |

The compiler enforces swarm safety rules (SWM-*) from the SKB: deadlock prevention,
capability propagation, topology connectivity, and agent Send+Sync requirements.

### 9.3 Swarm Operations (Library API)

For dynamic swarm usage, a library API is also available:

**There is no library API.** `std::agent` does not exist, and neither do
`Swarm`, `SwarmConfig` or `ConsensusStrategy`. Dynamic swarm usage is the same
code as static: functions, `map` and `fold`, with the capabilities in the
annotation.

```mg
agent CodeReviewer { capabilities: [agent] }

swarm ReviewTeam {
    agent: CodeReviewer
    size: 5
    consensus: majority
}

f review_one(path: str) -> str / fs, agent {
    agent.spawn(path)
    fs.read_to_string(path)
}

+f distributed_review(files: [str]~) -> [str]~ / fs, agent {
    map(files, |file| review_one(file))
}
```

The five `swarm_*` orchestration keywords (`swarm_map_reduce`,
`swarm_pipeline`, `swarm_saga`, `swarm_fan_out`, `swarm_race`) are reserved by
the lexer and consumed by nothing; writing one is a parse error that says so.

### 9.4 Capability-Based Security

All agent operations are gated by capabilities — fine-grained permissions that can be requested, leased, and revoked:

**There is no `Capability` type, no `Region`, and no runtime request.** The
gate is static, and it is the one described in §11:

- **Reaching a capability namespace is the request.** `fs.read_to_string(p)`
  puts `fs` in the function's inferred set — there is no separate call to
  forget, and no way to reach the resource without the checker seeing it.
- **The `/ effect` annotation is the grant**, required on every `pub` function
  for everything it performs (`inferred ⊆ declared`).
- **The check happens before the program runs**, not at the moment of use.
- **`handle … with` is revocation**, scoped to a block: it discharges an
  effect, so code inside it can no longer reach the real resource.

```mg
effect Analyze {
    f run(code: str) -> str;
}

f analyse(code: str) -> str / analyze { Analyze.run(code) }

// Sandboxed: the handler supplies the answer, so this function performs
// nothing at all — no file I/O, no network, no model call.
+f sandboxed_analysis(code: str) -> str {
    handle {
        analyse(code)
    } with Analyze {
        run(source) => f"{len(source)} bytes",
    }
}
```

---

## 10. Type System

### 10.1 Overview

MAGE's type system extends Rust's with:

1. **Tensor types** — compile-time shape verification, autograd tracking
2. **Neural types** — `net`, `layer`, `Param` as typed constructs
3. **Genome types** — typed genotypes with derive-based mutation/crossover
4. **Knowledge types** — `KnowledgeBase`, rules, facts
5. **Agent types** — typed message protocols, capability contracts
6. **Lifetime inference** — no user-visible lifetime annotations
7. **Borrow mode checking** — shared (`&T`) and exclusive (`&!T`) references do not unify; a mismatch is a borrow mode mismatch error
8. **Effect types** — every function has an effect signature

### 10.2 Type Judgment

$$\Gamma; \Sigma; \Delta; \Phi \vdash e : \tau \dashv \varepsilon$$

where:
- $\Gamma$ — type environment (variable → type)
- $\Sigma$ — SKB context (safety rules)
- $\Delta$ — effect environment (active handlers)
- $\Phi$ — shape environment (tensor dimensions)
- $e$ — expression
- $\tau$ — type
- $\varepsilon$ — effect set

### 10.3 Core Typing Rules

$$
\frac{x : \tau \in \Gamma}{\Gamma \vdash x : \tau \dashv \emptyset} \quad \text{[T-Var]}
$$

$$
\frac{\Gamma \vdash f : (\tau_1, \ldots, \tau_n) \xrightarrow{\varepsilon_f} \tau_r \quad \Gamma \vdash e_i : \tau_i \dashv \varepsilon_i}{\Gamma \vdash f(e_1, \ldots, e_n) : \tau_r \dashv \varepsilon_f \cup \bigcup_i \varepsilon_i} \quad \text{[T-App]}
$$

$$
\frac{\Gamma \vdash e : \tau \dashv \varepsilon \quad \Gamma, x : \tau \vdash e' : \tau' \dashv \varepsilon'}{\Gamma \vdash \text{val } x = e; \; e' : \tau' \dashv \varepsilon \cup \varepsilon'} \quad \text{[T-Let]}
$$

### 10.4 Tensor Typing Rules

$$
\frac{A : \text{Tensor}\langle T, [M, K]\rangle \quad B : \text{Tensor}\langle T, [K, N]\rangle}{A \otimes B : \text{Tensor}\langle T, [M, N]\rangle} \quad \text{[T-Matmul]}
$$

$$
\frac{A : \text{Tensor}\langle T, S\rangle \quad B : \text{Tensor}\langle T, S\rangle}{A + B : \text{Tensor}\langle T, S\rangle} \quad \text{[T-TensorAdd]}
$$

$$
\frac{e : T \quad w : T \quad T \in \{f32, f64\} \quad \text{diff}(e) \sqsubseteq \text{AlmostEverywhere}}{\text{grad}(e, w) : T} \quad \text{[T-Grad]}
$$

### 10.5 Type Inference

Bidirectional type checking with Hindley-Milner unification, extended for:
- **Shape unification**: tensor dimension variables solved via arithmetic constraints
- **Effect unification**: effect variables solved via set-union constraints
- **Genome type derivation**: crossover/mutate signatures inferred from data fields

### 10.6 Units of Measure

A float type may carry a unit of measure in square brackets: `f64[m/s]`,
`f32[kg*m/s^2]`, `[f64[N]]~`. A float takes no type arguments, so on `f32`
and `f64` the brackets hold a unit. `<...>` was not used: after a type name it
is already a generic argument list (`Path<T>`), in an expression it is
less-than (`x as f64 < y`), and a nested `>>` lexes as a shift. An undelimited
form such as `f64 @ m/s` was not used either: a unit's `/` would run on into a
return type's `/ effect` annotation.

```
dimensioned = ('f32' | 'f64') '[' unit ']' ;
unit        = factor { ('*' | '/') factor } ;
factor      = UNIT_NAME [ '^' [ '-' ] INT ] | '1' ;
```

Each `/` divides by the one factor after it, so `J/kg/K` is J·kg⁻¹·K⁻¹. The
parser stores a unit in one canonical spelling (repeated names merged,
numerator first, then each denominator after a `/`, `1` when nothing is
left), and both surfaces print that spelling: `f64[s^-1*m]` prints as
`f64[m/s]` under `--fmt-compact` and `--fmt-expand` alike.

**Dimensions.** A dimension is a vector of integer exponents over the seven SI
base units `kg`, `m`, `s`, `A`, `K`, `mol` and `cd`. The derived names `N`,
`J`, `W`, `Pa` and `Hz` expand to base exponents, so `f64[N]` and
`f64[kg*m/s^2]` are the same type. A bare `f64`, and every integer type, is
dimensionless.

**Rules.**

- `+`, `-`, `%`, the comparisons, `min`/`max`, `=`, `+=` and `-=` need equal
  dimensions on both sides; the result keeps the dimension. `clamp(x, lo, hi)`
  needs all three in one dimension, and keeps it.
- `*` adds exponents and `/` subtracts them. `*=` and `/=` need a
  dimensionless right-hand side, since they cannot change their target's unit.
- A numeric literal is **polymorphic**. Under `+`, `-` and the comparisons it
  takes the other operand's unit (`x + 1.0`, `x > 0.0`); under `*` and `/` it
  is a pure number (`2.0 * x` has the unit of `x`, `1.0 / t` has the inverse of
  the unit of `t`). A `val` bound to a literal without an annotation stays
  polymorphic; write `val g: f64[m/s^2] = 9.81` to fix its unit.
- Parameters, returns, struct fields, consts and annotated bindings carry their
  declared dimension. A call checks every argument against its parameter, and
  a function's body and each `ret` are checked against its declared return.
  `if` and `match` branches must agree.
- `sqrt` halves every exponent and needs them all even (`sqrt` of `m^2` is
  `m`; of `m` it is refused). `abs`, `relu`, `floor`, `ceil`, `round` and `trunc` keep
  the unit. `exp`, `ln`, `log`, `log2`, `log10`, `sin`, `cos`, `tan`, `asin`,
  `acos`, `atan`, `sinh`, `cosh`, `tanh` and `sigmoid` need a dimensionless
  argument and return a dimensionless value. Each may be written `f(x)` or
  `x.f()`. Each takes exactly one number, and a wrong count or a non-number is
  refused at `--check`.
- `relu(x)` is `max(x, 0.0)` and takes one number, so it is written either way
  too; its derivative is 1 for `x > 0` and 0 otherwise, including at 0.
  `clamp(x, lo, hi)` limits `x` to `[lo, hi]`; it takes three numbers, is a
  call only, and `lo > hi` is a run-time error. Its derivative with respect to
  `x` is 1 for `lo < x < hi` and 0 otherwise. With all three integers the
  result is an integer, as for `min` and `max`; otherwise it is a float.
- `x as f64` drops a unit and `x as f64[m]` asserts one. This is the deliberate
  escape hatch.

Where the checker cannot tell a dimension (a method of a user type, an
unannotated closure parameter, an index into an untyped collection) it says
nothing: a mismatch is reported only between two dimensions it knows. Code
with no unit annotations, where every known dimension is dimensionless,
cannot fail the check, so the corpus typechecks unchanged.

**Errors.** A mismatch is **E0562** (`UnitMismatch`), and the message names
both dimensions:

```text
cannot add m and s
cannot compare m and s
`speed` returns m/s, but is declared to return m
argument 1 of `speed` is s, but the parameter is m
cannot take the square root of m: its exponents are not all even
`exp` needs a dimensionless argument, but was given m
```

An unknown unit name (`f64[ft]`) is E0412 (`UnresolvedType`), and its message
lists the units that exist.

**Runtime.** Units are erased once checked. The evaluator, convexity,
differentiability and refutation all see the base float type, so an annotated
program runs exactly as the unannotated one does.

**Not covered.** Unit conversion and prefixes (`km`, `ms`, `g`: a value is
in base SI units or it is unannotated), affine units such as °C (`K` is a
difference of temperatures only), generics over units (a function cannot be
polymorphic in the unit of its argument; `f sq(x: f64) -> f64` takes only a
dimensionless value), units on integer types, and units inside `tensor[...]`
element types.

The program below typechecks. Changing `b.x + vel * dt` to `b.x + vel` makes
it **E0562** (`cannot add m and m/s`).

```mg
S Body { x: f64[m], vel: f64[m/s] }

f step(b: Body, a: f64[m/s^2], dt: f64[s]) -> Body {
    val vel = b.vel + a * dt;
    @Body { x: b.x + vel * dt, vel: vel }
}

f kinetic(mass: f64[kg], vel: f64[m/s]) -> f64[J] {
    0.5 * mass * vel * vel
}
```

---

## 11. Effect System

### 11.1 Overview

Every function has an effect signature. Effects are algebraic — declared, composed, and handled.

### 11.2 Standard Effects

These nineteen names are the built-in effect kinds. Each may be written in a `/ …`
annotation with no declaration; any other name must be declared by an `effect`
block (§11.5), or it is an error (§11.4).

| Effect       | Domain                       | Description                     |
| ------------ | ---------------------------- | ------------------------------- |
| `io`         | read, write, seek, close     | File and stream I/O             |
| `net`        | connect, listen, send        | Network I/O                     |
| `fs`         | open, stat, mkdir, remove    | Filesystem operations           |
| `async`      | spawn, join, select          | Asynchronous task management    |
| `alloc`      | alloc, dealloc, realloc      | Heap memory allocation          |
| `panic`      | panic, catch_panic           | Unwinding / structured panics   |
| `ffi`        | call_foreign                 | Foreign function invocation     |
| `env`        | get_var, set_var             | Environment variable access     |
| `time`       | now, sleep, timeout          | Clock and timer access          |
| **`gpu`**    | **dispatch, synchronize**    | **GPU computation**             |
| **`npu`**    | **dispatch, synchronize**    | **Neural processing unit**      |
| **`llm`**    | **generate, embed, analyze** | **Language model invocation**   |
| **`evolve`** | **evaluate, select, mutate** | **Evolutionary computation**    |
| **`learn`**  | **forward, backward, step**  | **Training / gradient descent** |
| **`rng`**    | **random, seed, sample**     | **Random number generation**    |
| `agent`      | lifecycle, message, lease    | Agent coordination              |
| `proc`       | spawn, exec, tool invocation | Process and system access       |
| `heldout`    | read                         | Held-out evaluation data (§11.6) |
| `promote`    | promote, journal             | Changing authority (§11.6)      |

The middle column names each effect's **domain**. It is not a table of callable
operations: a built-in effect has no `effect` block, so nothing declares
`dispatch` or `lifecycle` and calling them performs nothing. A function acquires
a built-in effect three ways, and only three:

1. **By annotation.** `/ gpu` puts `gpu` in the function's set, and it
   propagates to every caller. This works for all nineteen.
2. **Through a capability handle.** `io.println(…)`, `net.connect(…)`,
   `llm.generate(…)` — the receiver names the capability, and the capability is
   what gets attributed. This is the same rule as `Audit.record(…)` performing
   `audit` (§11.5): the effect comes from the receiver, not the operation.

   | Namespace | Effect | | Namespace | Effect |
   | --- | --- | --- | --- | --- |
   | `io` `log` | `io` | | `gpu` | `gpu` |
   | `fs` | `fs` | | `llm` | `llm` |
   | `net` `http` | `net` | | `rng` | `rng` |
   | `env` | `env` | | `agent` `swarm` | `agent` |
   | `time` | `time` | | `os` `sys` `process` `tools` | `proc` |
   | `mem` | `alloc` | | `json` `kb` `db` | — |

   `json` computes over values already in hand. `kb` and `db` reach a store that
   no built-in kind names; declare `effect Db { … }` and call it, which is what
   `examples/effects-showcase` does.

3. **By calling a recognized builtin.** A fixed set of names is attributed by
   name alone:

   | Effect  | Names attributed on call |
   | ------- | ------------------------ |
   | `io`    | `print` `println` `eprint` `eprintln` `write` `writeln` |
   | `panic` | `panic` |

   Of the names that perform a built-in effect, only these seven can be called
   bare. Every other effect has no bare function to attribute: `read_line`, `open`, `connect`, `spawn`, `alloc`,
   `get_env`, `now`, `sleep` and the rest are not callable by that name, so their
   effects come through an annotation or a capability handle from route 2 —
   `io.read_line(…)`, `fs.read(…)`, `net.connect(…)`, `env.get_env(…)`,
   `time.now()`, `time.sleep(…)`.

   The standard vocabulary wins every collision: `join` is the vocabulary's
   pure `([str], str) -> str`, so calling it is pure and does **not** mean a
   thread join. Beyond `io` and `panic`, no effect attributes any name — `fs`,
   `net`, `async`, `alloc`, `env`, `time`, `ffi`, `npu`, `evolve`, `learn` and
   the rest are reached by annotation, by a capability handle where the
   namespace table has one, or by declaring an `effect` block whose operations
   you can actually call and handle.

This last table is deliberately short, and route 2 is preferred over it.
Attribution by bare name claims a word out of the whole program's namespace, so
a user function that happens to share one inherits an effect it does not
perform. A capability handle cannot collide that way: the receiver has to be the
namespace.

A **declared effect wins the name.** An `effect Io { … }` block in the module
makes `Io.…` that module's own effect, checked against its own operation list,
rather than the built-in capability.

### 11.3 Effect Typing Rules

$$
\frac{\text{body has no effect operations}}{\Gamma \vdash f : \tau_1 \rightarrow \tau_2 \dashv \emptyset} \quad \text{[E-Pure]}
$$

$$
\frac{f : \tau_1 \xrightarrow{\varepsilon_f} \tau_2 \quad g : \tau_2 \xrightarrow{\varepsilon_g} \tau_3}{g \circ f : \tau_1 \xrightarrow{\varepsilon_f \cup \varepsilon_g} \tau_3} \quad \text{[E-Compose]}
$$

$$
\frac{\Gamma; \Delta, (\text{eff} \mapsto h) \vdash e : \tau \dashv \varepsilon \cup \{\text{eff}\}}{\Gamma; \Delta \vdash \text{handle } e \text{ with } h : \tau \dashv \varepsilon} \quad \text{[E-Handle]}
$$

### 11.4 Effect Inference

Effects are inferred bottom-up: leaf functions first, callers accumulating the
union of everything they reach.

Annotations are **not** optional documentation. The rule the compiler enforces
is *under*-declaration, at the module boundary:

- a **private** function may omit its annotation and infer silently;
- a **`pub` function, or `main`,** must declare every effect it performs;
- any function that *does* annotate is held to it — inferred ⊆ declared —
  whether it is public or not.

Over-declaration is always accepted, so an annotation is an upper bound rather
than an exact description. This is sound because effects propagate
transitively: whatever a private function performs surfaces in the inferred set
of every public caller that reaches it. The capability gate holds at the module
surface while internal code pays no annotation tokens.

An effect name must resolve — a built-in kind from §11.2, or an `effect` block.
An annotation naming neither is an error, so a misspelling cannot silently
become a new effect that is enforced and matches nothing.

### 11.5 Performing and Handling

An `effect` block declares an effect and its operations:

```mg
effect Audit {
    f record(entry: String) -> usize;
}
```

Calling an operation performs the effect — this is the introduction rule, and
what puts `audit` in the function's inferred set:

```mg
effect Audit {
    f record(entry: String) -> usize;
}

f transcribe(entry: String) -> usize / audit {
    Audit.record(entry)
}
```

`handle … with` is the elimination rule of [E-Handle]. It removes the effect
from the block it wraps, so the handling function can be pure:

```mg
effect Audit {
    f record(entry: String) -> usize;
}

f transcribe(entry: String) -> usize / audit {
    Audit.record(entry)
}

f summarize(entry: String) -> String {
    val n = handle { transcribe(entry) } with Audit {
        record(e) => len(chars(e))
    }
    f"recorded {n} chars"
}
```

The subtraction is per handled *block*, not per function: an unhandled call
beside a handled one still reports. The arm's own effects are attributed to the
handling function, so handling `audit` by writing a file yields `/ fs` — a
handler exchanges one effect for the effects of handling it.

Handlers are found **dynamically** (the innermost handler for an operation
wins) and evaluated **lexically** (an arm sees the scope the handler was
written in, not the frame that performed the operation).

**Resumption is single-shot and implicit.** An operation call dispatches to its
arm, and the arm's value becomes the value of the call — so the body carries on
from where it left off. Two operations in sequence each resume, and the arm is
re-evaluated per call rather than computed once:

```mg
effect A { f ask() -> i32; }

f work() -> i32 / a {
    v got = A.ask()
    got + 100
}

f run() -> i32 {
    handle { work() } with A { ask() => 7 }   // 107
}
```

**An arm may abort instead, with `ret`.** That discards the rest of the handled
body and makes `ret`'s value the value of the whole `handle` expression. It does
*not* return from the enclosing function:

```mg
effect A { f ask() -> i32; }

f work() -> i32 / a {
    v got = A.ask()
    got + 100
}

f caller() -> i32 {
    v r = handle { work() } with A { ask() => ret 7 }
    r + 1000                                          // 1007
}
```

#### Single-shot is the whole of it, and that is final

**Resumption is single-shot. There is no `resume` keyword, no reified
continuation, and neither is planned.** This is a decision, not a gap awaiting
work — it was recorded as "missing" for some time, which invited the reading
that multi-shot was coming.

The continuation cannot be stored in a variable, returned, or invoked twice.
Handlers that need none of that — state, reader, logging, tracing, retry
policies, capability interception and test mocking — are the handlers MAGE
exists to serve, and all of them work today. What is genuinely excluded is
generators, backtracking search, and any scheduler that re-enters a suspended
computation more than once.

The reason to stop here is cost, and it is concrete. Multi-shot needs the
continuation as a first-class value, which in a dual-surface language means a
keyword *and* a sigil on every handler arm that uses it, plus a rule in the
effect system for what a resumed continuation performs and whether those
effects are re-attributed. It also needs the evaluator to hold the continuation
somewhere it can be copied, which the tree-walking evaluator cannot: the
continuation lives in the Rust call stack. That is a CPS or CEK rewrite across
every expression form. For a language whose premise is that tokens are scarce
and behaviour must be predictable to an agent, paying a permanent per-arm token
cost and an evaluator rewrite to enable backtracking is the wrong trade.

If a program needs multi-shot control, express it as data rather than control
flow: return the choices and let the caller iterate. That is more tokens at the
call site and fewer everywhere else.

Nothing here is irreversible. Should a real MAGE program need multi-shot, this
section is the thing to change first, and it should be changed with a program
that needs it in hand — not on the strength of the feature existing elsewhere.

### 11.6 Roles

A function may declare a **role**, which puts a ceiling on the effects it may
perform — directly, or through anything it calls:

```text
@role(candidate)   pure: computes, and nothing else
@role(proposer)    learn, alloc, rng
@role(mutator)     alloc, rng
@role(attacker)    learn, alloc, rng
@role(learner)     learn, gpu, alloc, rng
@role(evaluator)   a learner's effects, plus heldout
@role(gate)        an evaluator's effects, plus promote
```

A function whose inferred or declared effects exceed its role is an error,
`E0551` (`RoleViolation`). So is a role name not in this table. A role
narrows what a function may do and never widens it: effect declarations at
trust boundaries (§11.4) still apply.

**Why.** A self-improving system fails worst in two ways: a candidate that
reads the test set, and a harness change that rewrites its own evaluator. Both
are effects (`heldout` and `promote`), and effect inference is already
transitive, so a role turns both into type errors rather than things an audit
must catch. Until 2026-09-25 `@role(…)` parsed and was ignored.

The three agent roles (plan 7.2) are for the code that writes, varies and
attacks candidates. Each may sample, and none may use a GPU, read held-out
data or promote. An attacker has a proposer's powers and no more, because
it attacks the proxy and must not see what the proxy is checked against.
### 11.7 Tunable parameters

A **tunable parameter** is a `const` whose `@param(...)` attribute states the
domain its value must lie in. The domain is part of the declaration, so an
agent that tunes the value can read the admissible range without evaluating
anything, and the typechecker refuses a value outside it.

```mg
@param(0.0, 1.0)
const LR: f64 = 0.02;      // closed real interval [0.0, 1.0]

@param(1, 64)
const LAYERS: i64 = 4;     // closed integer interval [1, 64]

@param(in, 1, 2, 4, 8)
const HEADS: i64 = 4;      // finite set {1, 2, 4, 8}
```

- **Real interval** `@param(lo, hi)` on an `f32` or `f64` constant: closed,
  both bounds included.
- **Integer interval** `@param(lo, hi)` on an integer constant (`i8`..`i64`,
  `u8`..`u64`, `isize`, `usize`): closed, bounds must be integers.
- **Finite set** `@param(in, a, b, ...)` on either kind: the value must be one
  of the listed members, which must be of the constant's kind.

Negative bounds and values are written with a leading `-`, as in
`@param(-1.0, 1.0)` with the value `-0.5`. The default value must be a numeric
literal, optionally negated, so it can be read without evaluation.

A `@param` declaration is refused with **E0553** (`ParamDomain`) when it is
annotated on anything other than a `const`; when the domain is malformed (the
wrong number of arguments, a non-numeric bound, `lo > hi`, or an empty `in`
set); when the constant's type is neither integer nor float, or a bound or set
member is not of the constant's kind; when the value is not a numeric literal;
or when the value lies outside its domain. The message names the constant,
its value and its domain, for example "`LR` = 1.5 is outside its domain
[0, 1]". The diagnostic is an ordinary type error, so an out-of-domain
parameter stops the program at the typechecker.

### 11.8 Convexity (plan 6.2)

The convexity pass (`prototype/src/convexity.rs`) certifies the curvature of an
expression with the disciplined convex programming (DCP) rules of Grant and
Boyd. Verdicts are `Constant`, `Affine`, `Convex`, `Concave`, and `Unknown`.
`Unknown` means DCP cannot certify the expression. It does not mean the
expression is non-convex: `2 * x * x` is convex and is `Unknown`.

- A literal is `Constant`. A variable or parameter is `Affine`.
- Sums add curvatures, and negation swaps convex and concave.
- A product or quotient with a constant operand scales the other operand, and a
  negative constant flips its curvature. A product of two non-constant operands
  is `Unknown`, except `x * x` for affine `x`, which is convex.
- `exp` is convex and increasing. `ln`, `log` and `sqrt` are concave and
  increasing. `abs` is convex. `max` of two or more arguments is convex and
  `min` is concave, when every argument is.
- Composition: `f(g)` is convex when `f` is convex and `g` is affine, or `f` is
  nondecreasing and `g` convex, or `f` is nonincreasing and `g` concave.
  Dually for concave.
- Every other call, including `sin`, `cos` and a one-argument `max`, is `Unknown`.

Each expression also has a sign (`Nonneg`, `Nonpos` or `Unknown`), because
`abs` and the scaling rules depend on it.

Domains are not checked, and constant sub-expressions are not evaluated:
`ln(2) * exp(x)` is `Unknown`. Plan 6.3 reads a `Convex` or `Concave` verdict to
choose an exact solver.

### 11.9 Solver choice (plan 6.3)

The solver module (`prototype/src/solve.rs`) minimises or maximises a function
over a box of `Real` parameters. Its choice follows the convexity certificate:

- Projected gradient runs only when the curvature is `Convex` (or `Affine` or
  `Constant`) for minimisation, or `Concave` (or `Affine` or `Constant`) for
  maximisation, and every domain is `Real`.
- Otherwise a (1+1) evolution strategy runs, with the 1/5th success rule. Its
  reason names each check that failed: the curvature verdict, the sense, or a
  non-real domain.

`Unknown` is not non-convexity, so it falls back to the evolution strategy.
`2 * x * x` is convex but unknown to DCP, and it takes that route.

Each run returns a `Solution` that records the solver, the reason, the point,
the value, and the evaluations and fuel spent. Its JSON form is the journal
entry for the run. Fuel, from `eval::run_metered`, is the cost measure for both
solvers. `mage-parse --solve <file.mg|-> <fn> [--max] [--fuel N] [--seed S]`
solves a function over its `@param` box and prints that JSON record.

### 11.10 System identification with `--solve` (plan C4)

A physical parameter can be recovered from a trajectory. Write the objective as
the squared error between a simulation with the candidate parameters and a
reference trajectory, then solve it. Each parameter's domain comes from the
`@param` const with the same name, and the simulation is an ordinary loop.

```mg
@param(0.5, 10.0)
const K: f64 = 1.0;

f sse(k: f64) -> f64 {
    val reference = [0.96, 0.88, 0.78];   // literal samples from a known run
    var x = 1.0;
    var err = 0.0;
    for i in 0..3 {
        x = x + 0.1 * (0.0 - k) * x;
        err = err + (x - reference[i]) * (x - reference[i]);
    }
    err
}
```

`mage-parse --solve system_id_spring.mg sse --seed 7` prints the record. The two
examples are `prototype/examples/system_id_spring.mg` (stiffness `k` and damping
`c`, fitted together) and `prototype/examples/system_id_pendulum.mg` (length
`L`). A loop over an integrator is not certified by the convexity pass, so both
run the (1+1) evolution strategy. Each recovers its known value within 0.01. The
test is `prototype/tests/system_id.rs`.

### 11.11 A physics program task (plan C3)

Agents write integrators in MAGE, a held-out rollout ranks them, and a
contract refuses the ones that do not conserve energy. The system is an
undamped unit-mass spring, `x'' = -k x`. A candidate is one function:

```mg
@role(candidate)
f step(s: [f64], k: f64, dt: f64) -> [f64] {
    val vel = s[1] - dt * k * s[0];
    [s[0] + dt * vel, vel]
}
```

The state is `[x, v]`. Tuples have no field access (`s.0` does not parse), and
a struct would have to be declared by both the candidate and the task, so it
is a two-element list. Everything else belongs to the task,
`prototype/examples/physics_task/task.mg.in`: the energy, the contract, the
rollout, the reference trajectories and `score`. The candidate never owns the
contract, so it cannot drop it. The task is not a `.mg` file because it calls
`step` and so does not typecheck alone. The harness appends a candidate to it
and runs the result under fuel:

```text
cat task.mg.in symplectic_euler.mg | mage-parse --eval - score --fuel 1000000
```

Every step passes through `advance`, which carries the contract:

```text
sp advance {
    @ens(abs(energy(result, k) - e0) <= DRIFT * e0, "energy stays within DRIFT of its initial value")
}
```

`e0` is the trajectory's initial energy and `DRIFT` is 0.1. At the task's
`w dt` of 0.1 and 0.075, symplectic Euler's energy error stays below 5.3% and
velocity Verlet's below 0.3%. Explicit Euler multiplies the energy by
`1 + k dt^2` every step, so it crosses 10% at step 10. The bound is on drift
from `e0`, not on the change across one step. A one-step bound can be
written, as `energy(result, k) <= energy(old(s), k) * 1.01`, and it runs. But
no one-step bound separates the two integrators. At `w dt = 0.1`, symplectic
Euler's largest one-step gain is 1.0049% and explicit Euler's is exactly
1.0000%, so a 1% bound refuses symplectic Euler.

The score is the RMS position error over two held-out cases, `(k, x0, v0)` =
`(4, 1, 0)` and `(2.25, 0, 1.5)`, with 100 steps of `dt = 0.05` each. The
reference is the exact solution, so a candidate's error is its own. The
literals come from `scripts/physics-task-reference.py`. The candidate is pure,
since its role caps it, so it cannot read them.

The fixed population and what happens to each:

| candidate | verdict |
| --- | --- |
| `velocity_verlet.mg` | 0.00130 |
| `symplectic_euler.mg` | 0.0255 |
| `frozen.mg` (returns `s`) | 1.03, last: energy is conserved exactly, so only the score catches it |
| `explicit_euler.mg` | refused at step 10 |
| `verlet_missing_half.mg` (`dt * dt * a` for `0.5 * dt * dt * a`) | refused at step 26 |

A refusal names the clause:

```text
contract violated: `@ens(abs(energy(result, k) - e0) <= DRIFT * e0, "energy stays within DRIFT of its initial value")` of `advance` (sp advance, line 42:5) is false: `advance` returned [0.5707904498999999, -1.7650160200000002]
```

The harness is `prototype/tests/physics_task.rs`. It admits a candidate only
if the candidate is a single `@role(candidate) f step` that passes
`--check --core`, then composes it with the task, checks the result, and
scores it. Its break-check deletes `sp advance`, and explicit Euler then
scores 0.21 instead of being refused.

The arena runs this task as `arena physics-task [--fuel N] [FILE…]`
(`arena/src/physics_task.rs`). It is the fixed-harness task kind the arena
lacked: each candidate is admitted, spliced into `task.mg.in`, and run under
fuel, and the verdict is a score or a refusal. A contract violation is a
refusal of class `contract`, distinct from `fuel`, which is a refusal of
class `fuel`.

### 11.12 Contact as a convex step (plan C5)

One time step of a ball on a floor is a box-constrained quadratic program. The
unconstrained step is `y_free = y + v dt - g dt^2 / 2` and `v_free = v - g dt`.
The contact step is the `(yp, vp)` that minimises the squared distance to it,
with `yp` in the box `[0, 100]`, the floor:

```mg
@param(0.0, 100.0)
const YP: f64 = 1.0;

@param(-50.0, 50.0)
const VP: f64 = 0.0;

f contact_floor(yp: f64, vp: f64) -> f64 {
    val y0 = 0.001;
    val v0 = -1.0;
    val dt = 0.01;
    val g = 9.81;
    val y_free = y0 + v0 * dt - 0.5 * g * dt * dt;
    val v_free = v0 - g * dt;
    (yp - y_free) * (yp - y_free) + (vp - v_free) * (vp - v_free)
}
```

The convexity pass certifies this form, and the state is fixed in the body
rather than passed in, so the decision variables are only `yp` and `vp`. Each
square is `e * e` for one affine term `e` written identically twice, which
`convexity.rs` certifies (the `x * x` exception). A sum of convex terms is
convex. Writing `2 * x * x` instead is `Unknown`, because the rules do not
reassociate products. The form needed is therefore a sum of squares of
affine terms, each factor written out.

`mage-parse --solve prototype/examples/contact_step.mg contact_floor` takes the
projected-gradient path (`solver` is `ProjectedGradient`) and returns
`yp = 0`, the floor, with `vp = v_free`. The file also holds `contact_free`,
a step that stays above the floor, which returns `yp = y_free` and
`vp = v_free`. Both match `sim_bounce` in `arena/src/physics.rs`: the free step
for any restitution, and the penetrating step at restitution `e = 0`, where
bounce also ends at `y = 0`. The test is `prototype/tests/contact_step.rs`.

The QP does not cover restitution or the velocity update at contact. Bounce
sets `v = -e (v - g tau)` on impact, which is not a projection, so the QP's
`vp` is the unconstrained `v_free` even when the floor is hit. With `e > 0` the
bounce step rebounds above the floor (`y = 0.00688` at `e = 0.8`), so the QP
matches only the instant of contact, not the full step. It is one step, with
no friction and no multi-step rollout.

---

## 12. Contract System

### 12.1 Contract Attributes

Contracts live in a **`spec` block that shares the function's name** (`sp` in
agent mode), or as attributes directly above the function's signature
(`@req(a > 0)` on the line before `f add(…)`), which attach to that function
the same way and are checked the same way:

```mg
sp withdraw {
    @req(1b)
    @ens(1b)
    @fx()
}

+f withdraw(balance: u64, amount: u64) -> u64 or str {
    guard balance >= amount else { ret Err("insufficient funds") }
    Ok(balance - amount)
}
```

Inside an `@ens` postcondition two more names are in scope:

- `result` is the function's return value, typed as its return type;
- `old(x)` is parameter `x` as it was on entry. MAGE values are immutable, so
  that is the argument as passed; `old` still says which side of the call a
  clause is about. It takes a parameter name and nothing else: `old(x).field`,
  not `old(x.field)`.

Outside `@ens` neither means anything special. `result` or `old` in an `@req` or `@inv`, or
`old` of something other than a parameter, is **E0561** (`ContractScope`); in a
function body, `result` and `old` are ordinary names, and an unbound one is an
unresolved name (E0425) whose message says they belong in `@ens`. Neither is a
keyword, so `v result = …` in a body is still an ordinary variable.

`--check` typechecks every clause of a spec that shares a function's name,
in that function's parameter scope: the condition must be a `bool`.
`--eval` (and the fuel-bounded evaluator the arena runs, §4.11) checks every
`@req` on entry, after the parameters are bound and before the body runs, and
every `@ens` each time the function returns. A clause that is false fails the run
with an error naming the clause, the function and where it was written:

```text
contract violated: `@ens(result.balance == old(account).balance - amount, "correct deduction")` of `withdraw` (sp withdraw, line 3:5) is false: `withdraw` returned Account { balance: 6 }
```

A call that breaks a `@req` never runs its body. The run fails with the clause
named, and the arguments it was called with:

```text
contract violated: `@req(b != 0)` of `div` (sp div, line 2:5) is false: `div` called with a = 1, b = 0
```

Where such a call should instead be refused with a value, use a `guard`.
The two examples below parse, typecheck and run.

```mg
struct Account { balance: u64 }

sp withdraw {
    @req(account.balance >= amount, "sufficient funds")
    @ens(result.balance == old(account).balance - amount, "correct deduction")
    @fx()
}

+f withdraw(account: Account, amount: u64) -> Account {
    @Account { balance: account.balance - amount }
}
```

```mg
sp sorted {
    @req(len(items) > 0, "non-empty input")
    @ens(all(range(len(result) - 1), |i| result[i] <= result[i + 1]), "output is sorted")
    @ens(len(result) == len(old(items)), "preserves length")
    @fx()
}

+f sorted(items: [i64]~) -> [i64]~ {
    sort(items)
}
```

### 12.2 Verification

Contracts are verified via:
1. **Static analysis** — SMT solver for decidable predicates
   *(designed, not built: no SMT solver is wired in. Contracts are checked by
   `--check` for type and by the evaluator at run time.)*
2. **SKB cross-reference** — matching against the 255 known safety rules
   *(designed, not built: no compiler stage consults the SKB. `types.rs`,
   `effects.rs`, `resolve.rs`, `verify.rs`, `heal.rs` and `mlir.rs` contain
   zero references to it. The rules are served to agents over RAP.)*
3. **Runtime assertion** — fallback for undecidable predicates

---

## 13. Ownership and Borrowing

**Status: design, not enforcement.** There is no borrow checker. Today the type
checker distinguishes `&T` from `&!T`: a borrow-mode mismatch is an error, and
the expressions `&x` and `&!x` have those types. No move rule and no aliasing
rule is enforced, so a use after a move is accepted, and the evaluator has value
semantics. The rules below are the intended semantics; rule 4 (shared XOR
exclusive aliasing), rule 7's lifetime encoding, and the [Own-Valid] and
[Own-Move] judgments are **not enforced**.

MAGE is designed to preserve Rust's ownership and borrowing semantics with full inference:

1. Every value has exactly one owner.
2. When the owner goes out of scope, the value is dropped.
3. Values can be moved or, if `Copy`, duplicated.
4. Borrows: any number of `&T` (shared) XOR one `&!T` (exclusive). *(Not enforced.)*
5. Borrows must not outlive the referent.
6. The compiler infers borrow mode from usage context.
7. No lifetime annotations in source code — the SKB encodes lifetime rules. *(Not enforced.)*

$$
\frac{x : \tau \in \Gamma \quad x \notin \text{moved}(\Gamma)}{\Gamma \vdash_{\text{own}} x : \text{Valid}} \quad \text{[Own-Valid]}\ \text{(not enforced)}
$$

$$
\frac{\Gamma \vdash_{\text{own}} x : \text{Valid} \quad \Gamma' = \Gamma[\text{moved} \cup \{x\}]}{\Gamma \vdash_{\text{own}} \text{move}(x) : \text{Valid} \dashv \Gamma'} \quad \text{[Own-Move]}\ \text{(not enforced)}
$$

---

## 14. Module System

### 14.1 Standard Library

**There is no module system today, and no standard library to import from.**
`mod name { … }` parses as an item and resolves nothing; `use` parses and
brings nothing into scope (the checker warns); `::` is a parse error
everywhere. Whether MAGE *should* have modules is an open design question —
an import costs tokens and buys little when the library is small, fixed and
global — and this section records the shape it would take, as **invalid MAGE**:

```mg
// File: src/lib.mg (crate root)
pub mod network;
mod internal;

// Import paths use :: separators
use std::tensor::{Tensor, Param};
use std::neural::{net, layer, train};
use std::evolve::{Genome, evolve};
use std::kb::KnowledgeBase;
use std::agent::{Agent, Swarm, Message};
use std::rl::{Env, Policy, PPO};
use std::io;
use std::collections::HashMap;
```

### 14.2 Standard Library Structure

```
std::
  io          File I/O, streams, buffering
  net         TCP, UDP, HTTP, DNS
  fs          Filesystem operations
  col         Collections (Vec, HashMap, BTree, VecDeque)
  sync        Mutex, RwLock, Channel, Barrier, Atomic
  async       Async runtime: spawn, join, select
  fmt         Formatting: Display, Debug
  str         String utilities
  math        Trigonometry, exponentials, logarithms, RNG
  time        Instant, Duration, SystemTime
  json        JSON parse, stringify, Serialize, Deserialize
  env         Environment variables, args
  process     Command, exit, signal
  skb         Safety Knowledge Base queries
  effect      Effect trait, perform, handle
  spec        Contract verification
  test        Testing framework
  neural      Neural networks, layers, activations, training
  tensor      Tensor types, operations, autograd
  evolve      Evolutionary algorithms, genomes, selection
  kb          Knowledge base, facts, rules, queries
  agent       Agents, swarms, messages, capabilities
  llm         Language model types, prompts, responses
  rl          Reinforcement learning: Env, Policy, PPO, A3C
```

---

## 15. Hardware Acceleration Model

### 15.1 Compilation Targets

MAGE compiles to native code via MLIR and LLVM, with specialized lowering passes:

| Target | Backend           | Use Case                          |
| ------ | ----------------- | --------------------------------- |
| x86-64 | LLVM              | Desktop/server CPU                |
| ARM64  | LLVM              | Mobile/embedded CPU               |
| RISC-V | LLVM              | Open-ISA embedded                 |
| CUDA   | NVPTX via MLIR    | NVIDIA GPU (tensors, neural nets) |
| ROCm   | AMDGPU via MLIR   | AMD GPU                           |
| Metal  | MetalIR via MLIR  | Apple GPU                         |
| WASM   | LLVM WASM backend | Browser/edge deployment           |
| SPIR-V | MLIR SPIR-V       | Vulkan compute                    |
| NPU    | Vendor SDK        | Neural processing units           |

### 15.2 Automatic Dispatch

Tensor and neural network operations use a compile-time cost model to select the optimal target:

```
DispatchStrategy:
  1. Query available hardware (compile-time or JIT probe)
  2. Estimate operation cost (FLOPS, memory, transfer overhead)
  3. Select target: NPU > GPU > CPU-SIMD > CPU-scalar
  4. Generate target-specific kernel
  5. Insert data transfer operations (host↔device) as needed
  6. Fuse adjacent operations where possible
```

### 15.3 SIMD Types

**Not implemented.** `f32x4` and its siblings are not type names — the
checker reports `unresolved type`. Vectorisation is the backend's business
today: a `tensor[f32, …]` lowers to whatever the selected target supports, and
`@target(cpu)` / `@precision(f16)` steer it. What follows records the intent.

**Invalid MAGE today:**

```mg
// Built-in SIMD types
val a: f32x4;     // 128-bit, 4 x f32
val b: f32x8;     // 256-bit, 8 x f32
val c: f64x4;     // 256-bit, 4 x f64
val d: f32x16;    // 512-bit, 16 x f32 (AVX-512)

// SIMD operations
val sum = a + b;
val product = a * b;
val dot = (a * b).sum();
```

---

## Appendix A: Full Grammar

The normative grammar is `kb/grammar.mg`: one `rhs(Production, "...")` fact per
alternative of each production of `prototype/src/parser.rs`, in EBNF over
production names and token names (`kb/lexicon.mg`). Juxtaposition is sequence,
`|` is alternation, `?` `*` `+` repeat, parentheses group. It is held to the
parser both ways: every `.mg` source in the repo is recognised by it, and
sentences sampled from it parse. §4 describes the same grammar construct by
construct. A few of its productions, copied verbatim:

```text
fact rhs(Module, "(ItBlockDef | Item)*");
fact rhs(Item, "(Attr | Contract)* Plus? (FunctionDef | StructDef | EnumDef | TraitDef | ImplBlock | ModuleDef | UseDecl | TypeAlias | ConstDef | StaticDef | EffectDef | SpecDef | AgentDef | SwarmDef | NetDef | KbDef | EvolveDef | TrainDef | DataDef | ExtendBlock)");
fact rhs(FunctionDef, "(KwF | KwAf | KwUf) Name Generics? Params (Arrow Type (KwOr Type)?)? WhereClause? (Slash EffectName EffectArgs? ((Comma | Plus) EffectName EffectArgs?)*)? (Assign Expr Semi? | Semi | Block)");
fact rhs(StructDef, "KwS Name Generics? LBrace (Plus? Name Colon Type Comma?)* RBrace");
fact rhs(TypeAlias, "(KwType | KwY) Name Generics? (Assign Type)? (TildeArrow ItRefineTok*)? Semi");
fact rhs(BlockBody, "Stmt* Expr?");
fact rhs(Stmt, "StLet");
fact rhs(Stmt, "StGuard");
fact rhs(Stmt, "Expr Semi");
fact rhs(StLet, "StLetKw Pattern (Colon Type)? Assign Expr Semi?");
```

A leading `+` (`Plus?` in `Item`) marks an item public, so a public function is
`+f`.

---

## Appendix B: Dual Syntax Mapping Table

The two modes are one language with two spellings of some words. Both modes lex
every spelling (§3.3), so a file may mix them. The agent spelling is the one the
formatter prints in Agent mode. **Every row below was checked against the
compiler**, and a test holds the table in B.1 to the keyword pairs of
`kb/lexicon.mg`, both ways.

### B.1 Keyword Pairs

Every Agent sigil and the Human keyword it abbreviates, and no others. A
construct with no row here has one spelling, used in both modes (B.2–B.8).
`:` appears twice: after a `for` pattern it is `in`, after an `if` block it is
`else`. `R`, `cons` and `topo` are not reserved words; they lex as identifiers
(§3.3).

| Agent  | Human       | Meaning                       |
| ------ | ----------- | ----------------------------- |
| `f`    | `fn`        | Function                      |
| `+`    | `pub`       | Public prefix                 |
| `S`    | `struct`    | Struct                        |
| `E`    | `enum`      | Enum                          |
| `T`    | `trait`     | Trait                         |
| `I`    | `impl`      | Impl block                    |
| `C`    | `const`     | Constant (uppercase; lowercase `c` is an identifier) |
| `D`    | `data`      | Data declaration, including a sum type |
| `v`    | `val`       | Immutable binding             |
| `m`    | `var`       | Mutable binding               |
| `af`   | `async`     | Async function prefix         |
| `ret`  | `return`    | Return                        |
| `fx`   | `effect`    | Effect declaration            |
| `hx`   | `handle`    | Effect handler                |
| `sp`   | `spec`      | Spec block                    |
| `gd`   | `guard`     | Guard                         |
| `df`   | `defer`     | Defer                         |
| `sw`   | `swarm`     | Swarm definition              |
| `xd`   | `extend`    | Extend block                  |
| `M`    | `mod`       | Module                        |
| `u`    | `use`       | Import                        |
| `Y`    | `type`      | Type alias                    |
| `Z`    | `static`    | Static item (retired: E0554)  |
| `xn`   | `extern`    | Extern block                  |
| `?`    | `if`        | Conditional                   |
| `?=`   | `match`     | Pattern match                 |
| `@`    | `for`       | For loop                      |
| `:`    | `in`        | Separator of a `for` loop     |
| `:`    | `else`      | Else branch                   |
| `@w`   | `while`     | While loop                    |
| `@@`   | `loop`      | Infinite loop                 |
| `!`    | `break`     | Break                         |
| `>>`   | `continue`  | Continue                      |
| `~>`   | `where`     | Where clause                  |
| `R`    | `or`        | Or                            |
| `cons` | `consensus` | Swarm consensus field         |
| `topo` | `topology`  | Swarm topology field          |

Two spellings that look like pairs are not: `uf` is the one spelling of an
unsafe function in both modes, and there is no `const fn`, `pub(crate)` or
`crate::` — there is no module system to give them a meaning.

### B.2 AI Constructs

The AI blocks use the **same keyword in both modes**: `net`, `layer`, `train`,
`kb`, `fact`, `rule`, `query`, `agent`, `swarm`, `evolve`, `genome`, `fitness`,
`select`, `crossover`, `mutate`, `population`, `generations`, `rl`, `policy`,
`reward`. Fifteen Greek letters are also reserved as tokens (`Ψ` for `net`, `Σ`
for `swarm`, and so on, listed in D.1), but **no parser arm consumes them**:
`Ψ Classifier { … }` is `expected item, found KwPsi`. There is no Greek spelling
of `population`, `reward`, `generations`, `target`, `select`, `fact` or `query`,
and no agent spelling of the type names `Tensor`, `Param`, `Genome`, `Policy`,
`LLM` or `KnowledgeBase`.

### B.3 Control Flow

| Human            | Agent          | Notes                                     |
| ---------------- | -------------- | ----------------------------------------- |
| `if c { … }`     | `? c { … }`    |                                           |
| `else`           | `:`            |                                           |
| `else if`        | `: ?`          | The space matters: `:?` is a token (`ColonQuestion`) that no parser arm consumes |
| `match e { … }`  | `?= e { … }`   | `? e { … }` also parses                   |
| `for x in y`     | `@ x : y`      | `@ x in y` also parses; `~` does not      |
| `loop`           | `@@`           |                                           |
| `while c`        | `@w c`         | `loop ?` does not parse                   |
| `return`         | `ret`          |                                           |
| `break`          | `!`            |                                           |
| `continue`       | `>>`           |                                           |
| `1b` / `0b`      | `1b` / `0b`    | Booleans, the same in both modes (§3.5)   |

### B.4 Type Syntax

| Human             | Agent      | Meaning                    |
| ----------------- | ---------- | -------------------------- |
| `String`          | `s`        | Owned string               |
| `&str`            | `&s`       | String slice               |
| `[T]~`            | `[T]~`     | Growable array             |
| `?T`              | `?T`       | Optional                   |
| `Result<T,E>`     | `R[T,E]`   | Result                     |
| `^T`              | `^T`       | Heap pointer               |
| `Rc<T>`           | `$T`       | Reference counted          |
| `Arc<T>`          | `@T`       | Atomic reference counted   |
| `{K: V}`          | `{K:V}`    | Hash map                   |
| `{K}`             | `{K}`      | Hash set                   |
| `&!T`             | `&!T`      | Exclusive reference; `&mut T` is not MAGE |
| `fn(T)->U`        | `f(T)->U`  | Function pointer           |
| `T<A>` (generics) | `T[A]`     | Generic parameters         |
| `f64[m/s]`        | `f64[m/s]` | Float with a unit (§10.6)  |

### B.5 Tensor Operations

The tensor operators are single tokens and are spelled the same in both modes.
There is no `@` or `.*` operator.

| Operator   | Position | Meaning                                   |
| ---------- | -------- | ----------------------------------------- |
| `A ⊗ B`    | infix    | Matrix multiply                           |
| `A ⊙ B`    | infix    | Hadamard product                          |
| `A⊤`       | postfix  | Transpose                                 |
| `A⊥`       | postfix  | Flatten                                   |
| `A \|> f`  | infix    | Pipeline; `A ▸ f` is the same operator    |
| `grad(l, p)` | call   | Gradient; `∇(l, p)` is the same call, and the second argument must be a name |

The layer and activation functions are not syntax. The names a program may call
without declaring them are the `builtin_function` facts in `kb/meaning.mg`
(`println`, `format`, `relu`, `sigmoid`, `tanh`, `exp`, `mean`, `dot`, and so
on); `dense`, `conv2d` and `dropout` are not among them.

### B.6 Path and Scope

| Rust        | MAGE            | Meaning        |
| ----------- | --------------- | -------------- |
| `::`        | `.`             | Path separator. `::` does not parse in either MAGE mode |
| `Foo { x }` | `@Foo { x: 1 }` | Struct literal. The `@` goes **before** the name, every field is named, and bare `Foo { … }` is a *map* |

There is no crate root, `super` or `self` path.

### B.7 Attributes and Printing

An attribute is `@name` or `@name(args)` in both modes; the `#[…]` form is not
MAGE. The formatter compresses and expands the names below
(`ATTRIBUTE_ABBREVIATIONS` in `elision.rs`). There are no macros: `println`,
`format` and `eprintln` are builtin functions, and `p"…"`, `f"…"` and `ep"…"`
are the string forms that print, format and print to stderr.

| Full name       | Compressed | Meaning              |
| --------------- | ---------- | -------------------- |
| `@derive(...)`  | `@d(...)`  | Derive               |
| `@test`         | `@t`       | Test                 |
| `@bench`        | `@b`       | Benchmark            |
| `@inline`       | `@il`      | Inline               |
| `@inline_always`| `@ila`     | Inline always        |
| `@cfg(...)`     | `@cfg(...)`| Conditional compile  |

| Function call     | String form | Meaning       |
| ----------------- | ----------- | ------------- |
| `println(...)`    | `p"..."`    | Print line    |
| `format(...)`     | `f"..."`    | Format string |
| `eprintln(...)`   | `ep"..."`   | Error print   |

### B.8 Shared Syntax (Identical in Both Modes)

- The numeric types that exist (`i8`–`i128`, `u8`–`u128`, `isize`, `usize`,
  `f32`, `f64`). **`f16` and `bf16` are not type names** — the checker reports
  `unresolved type` — though `@precision(f16)` is accepted as an attribute
- Booleans `1b` and `0b`
- Arithmetic, comparison, logical, bitwise operators
- Semicolons, braces, parentheses
- Comments (`//`, `/* */`, `///`, `//!`)
- Effect annotations (`/ io`, `/ gpu`, `/ llm`)
- Contract attributes (`@req`, `@ens`, `@inv`, `@perf`, `@fx`, `@spec`)
- Range operators (`..`, `..=`)
- Try operator (`?` postfix)
- Closures (`|x| expr`, and `fn(x) => expr`)
- The tensor operators of B.5
- ~~`tensor!` literals~~ — **there are no macros**; write a list literal

### B.9 Agent Mode Safety Philosophy

In agent mode, safety constructs are **fully handled by the compiler and SKB** (Safety Knowledge Base). The following constructs are unnecessary in agent mode:

| Human Syntax                   | Agent Mode Handling                              | SKB Rules |
| ------------------------------ | ------------------------------------------------ | --------- |
| `unsafe { ... }`               | **Parse error** — see the note below              | AEL-0001  |
| `unsafe fn`                    | Spelled `uf` in both modes; elision clears its marker | AEL-0002  |
| Lifetime annotations (`'a`)    | Inferred by compiler's LIF rules                 | AEL-0003  |
| `&!T` explicit annotation      | Inferred — compiler determines mutability        | AEL-0004  |
| `Send` / `Sync` bounds         | Derived automatically from type structure        | AEL-0005  |
| `move` keyword on closures     | Inferred — compiler determines capture mode      | AEL-0006  |
| `Pin<T>` wrapping              | Handled automatically for self-referential types | AEL-0007  |
| `dyn` / `impl` dispatch choice | Compiler selects based on call-site analysis     | AEL-0008  |
| `PhantomData<T>`               | Inserted automatically by compiler               | AEL-0012  |

> **Retired in plan 9.2.** The `unsafe { ... }` block has no parser producer, no source used it, and its `Expr::UnsafeBlock` variant and eight pass arms were removed. Put the code in a `uf` (unsafe function) instead.
>
> **`unsafe { ... }` does not parse.** Corrected 2026-09-07. It is a parse
> error in every position — expression, statement, and initialiser — with
> `expected expression, found KwUnsafe`. The AST has an `Expr::UnsafeBlock`
> variant and **eight passes handle it** (`elision`, `effects`, `fmt`, `mlir`,
> `resolve`, `types`, and `token_budget` twice); `parser.rs` constructs it
> **zero** times, so no MAGE source can produce one.
>
> `unsafe fn` *does* work, spelled `uf`, and elision clears its marker — that
> row is accurate. No `.mg` source in the repository writes one.
>
> The block form is documented as accepted-and-elided in three places (here,
> §26's equivalence table, and the keyword list in the grammar), which is why
> it reads as implemented. Eight handling arms are the evidence that someone
> intended it; the parser arm is the one that was never written.

This design maximizes token efficiency (fewer tokens in the LLM context window) while maintaining full safety guarantees through compiler infrastructure rather than language syntax.

---

## Appendix C: Operator Precedence

From highest to lowest. Every infix operator is left-associative, assignment
included: `a = b = c` groups as `(a = b) = c`. The postfix operators are one
loop and bind tighter than the prefix ones, so `-x as i32` is `-(x as i32)` and
`-x * y` is `(-x) * y`. A test in `parser.rs` holds the infix rows to the
parser's binding powers, both ways.

| Prec | Operator(s)                                                     | Description           | Assoc   |
| ---- | --------------------------------------------------------------- | --------------------- | ------- |
| 14   | `.f` field, `.m()` method, `[i]` index, `f()` call, `?`, `is`, `as`, `⊤`, `⊥` | Postfix | Postfix |
| 13   | `-` `!` `*` `&` `&!`                                            | Unary prefix          | Prefix  |
| 12   | `⊗` `⊙`                                                         | Matmul, Hadamard      | Left    |
| 11   | `*` `/` `%`                                                     | Multiplicative        | Left    |
| 10   | `+` `-`                                                         | Additive              | Left    |
| 9    | `<<` `>>`                                                       | Bit shift             | Left    |
| 8    | `&`                                                             | Bitwise AND           | Left    |
| 7    | `^`                                                             | Bitwise XOR           | Left    |
| 6    | `\|`                                                            | Bitwise OR            | Left    |
| 5    | `<` `>` `<=` `>=`                                               | Relational            | Left    |
| 4    | `==` `!=`                                                       | Equality              | Left    |
| 3    | `&&`                                                            | Logical AND           | Left    |
| 2    | `\|\|` `\|>` `▸`                                                 | Logical OR, pipelines | Left    |
| 1    | `=` `+=` `-=` `*=` `/=` `%=` `&=` `\|=` `^=` `<<=` `>>=`         | Assignment            | Left    |

`ret`, `!` (break) and `>>` (continue) are expressions that take the rest of the
expression as their operand; they are not operators of this table.

---

## Appendix D: Agent Mode Symbol Reference

The symbols of Agent mode, by category. Every symbol in the first column of
every table here is a spelling the lexer reserves (`kb/lexicon.mg`,
`lexes_as`), and a test holds the tables to that. A spelling being lexed does
not mean a parser arm consumes it, so each table says which it is. The
Human↔Agent keyword pairs are B.1; §3.3 and §3.6 give the full lexical rules.

### D.1 Greek Letters — Lexed, Not Parsed

> **Status: lexed, not parsed.** Each letter below is a single token of its own
> kind (`Ψ` produces `KwPsi`, `Σ` produces `KwSigma`), and **no parser arm
> consumes any of them**. `Ψ Classifier { … }` is `expected item, found
> KwPsi`. The constructs they were reserved for use the Human keyword in both
> modes. (`∇` is the one Greek symbol that parses; it is in D.2.) Other Greek
> letters, such as `η`, `ψ`, `Λ`, `Κ`, `σ` and `δ`, are not tokens at all and
> give `lexer error: unexpected character`.

| Symbol | Unicode | Reserved for |
| ------ | ------- | ------------ |
| `Ψ`    | U+03A8  | `net`        |
| `λ`    | U+03BB  | `layer`      |
| `Φ`    | U+03A6  | `tensor`     |
| `Π`    | U+03A0  | `param`      |
| `Θ`    | U+0398  | `train`      |
| `α`    | U+03B1  | `agent`      |
| `Σ`    | U+03A3  | `swarm`      |
| `κ`    | U+03BA  | `kb`         |
| `ρ`    | U+03C1  | `rule`       |
| `Ω`    | U+03A9  | `evolve`     |
| `Γ`    | U+0393  | `genome`     |
| `φ`    | U+03C6  | `fitness`    |
| `χ`    | U+03C7  | `crossover`  |
| `μ`    | U+03BC  | `mutate`     |
| `Ξ`    | U+039E  | `policy`     |

### D.2 Tensor and Pipeline Operators — Parsed

> **Status: parsed.** These are consumed: `∇` as a call (`grad` or `∇`
> followed by `(`), `⊗` `⊙` `▸` as infix operators with the precedences of
> Appendix C, and `⊤` `⊥` as postfix operators.

| Symbol | Unicode | Position | Meaning |
| ------ | ------- | -------- | ------- |
| `∇`    | U+2207  | call     | Gradient: `∇(loss, params)`, the second argument a name |
| `⊗`    | U+2297  | infix    | Matrix multiply  |
| `⊙`    | U+2299  | infix    | Hadamard product |
| `▸`    | U+25B8  | infix    | Pipeline, the same as the ASCII pipe operator |
| `⊤`    | U+22A4  | postfix  | Transpose        |
| `⊥`    | U+22A5  | postfix  | Flatten          |

### D.3 Declaration Sigils

> **Status: parsed.** Each is the Agent spelling of the Human keyword in B.1.

| Symbol | Meaning |
| ------ | ------- |
| `f`    | Function |
| `uf`   | Unsafe function (the one spelling in both modes) |
| `af`   | Async function prefix |
| `v`    | Immutable binding |
| `m`    | Mutable binding |
| `C`    | Constant |
| `D`    | Data declaration, including a sum type |
| `S`    | Struct |
| `E`    | Enum |
| `T`    | Trait |
| `I`    | Impl block |
| `Y`    | Type alias |
| `Z`    | Static item |
| `M`    | Module |
| `u`    | Import |
| `+`    | Public prefix |
| `xd`   | Extend block |
| `xn`   | Extern block |
| `fx`   | Effect declaration |
| `hx`   | Effect handler |
| `sp`   | Spec block |
| `gd`   | Guard |
| `df`   | Defer |

### D.4 Control Flow Sigils

> **Status: parsed**, except `:?`, which lexes as one token that no parser arm
> consumes. The working else-if is `: ?` with a space.

| Symbol | Meaning |
| ------ | ------- |
| `?`    | Conditional (`if`); also the postfix try operator |
| `?=`   | Pattern match (`match`) |
| `@`    | For loop (`for`) |
| `:`    | The `in` of a `for` loop, and `else` |
| `@w`   | While loop |
| `@@`   | Infinite loop |
| `!`    | Break |
| `>>`   | Continue |
| `ret`  | Return |
| `~>`   | Where clause |
| `R`    | `or` |
| `:?`   | Lexed, not parsed |

### D.5 Literals and Contracts

| Symbol  | Meaning |
| ------- | ------- |
| `1b`    | Boolean true, in both modes |
| `0b`    | Boolean false, in both modes |
| `@req`  | Precondition |
| `@ens`  | Postcondition |
| `@inv`  | Invariant |
| `@fx`   | Effect contract |
| `@perf` | Performance contract |

### D.6 Swarm Constructs

A swarm block has four fields and **no behaviour blocks**: `dispatch`,
`aggregate` and `on_failure` do not exist — a swarm block declares the group,
and the work is ordinary functions with `map` and `fold` (§9.2). The two
fields with an Agent spelling, and the block's keyword, are:

| Symbol | Meaning |
| ------ | ------- |
| `sw`   | Swarm definition (`swarm`) |
| `topo` | Communication topology field (`topology`) |
| `cons` | Consensus strategy field (`consensus`) |

`agent:` and `size:` have one spelling in both modes. `Σ` is a reserved token
that no parser arm consumes; `sw` is the Agent spelling that works.

### D.7 Safety Elision (Agent Mode — Handled by Compiler)

In agent mode the lifetime, `Send`/`Sync`, `Pin`, `PhantomData` and
closure-capture constructs have **no syntax**: the compiler's SKB handles them.
B.9 is the table, and the note there records that the `unsafe { … }` block does
not parse.

---

## Appendix E: Side-by-Side — Human vs Agent

The same three programs in both surfaces. **Every block below was verified with
`mage-parse --check`** — the two modes are the same language, so a construct
that is missing from one is missing from both: there is no Greek layer syntax,
no `Ψ`/`Ω`/`κ`/`α` block keywords, and nothing to import in either mode.

What actually differs is the *declaration* and *binding* spellings, and the
print form.

### E.1 A neural network and its training loop

**Human:**
```mg
net ImageClassifier {
    layer conv1: Conv2D(3, 32, 3)
    layer norm1: BatchNorm(32)
    layer conv2: Conv2D(32, 64, 3)
    layer hidden: Linear(28, 128)
    layer act: ReLU(128)
    layer drop: Dropout(0.5)
    layer out: Linear(128, 10)
    forward { out(drop(act(hidden(conv2(norm1(conv1)))))) }
}

train cifar_train {
    net: ImageClassifier
    dataset: "cifar10"
    optimizer: adam
    loss: cross_entropy
    epochs: 50
    batch_size: 128
}

pub fn main() -> i32 / io, gpu {
    println("training ImageClassifier on cifar10")
    0
}
```

**Agent:**
```mg
net ImageClassifier {
    layer conv1: Conv2D(3, 32, 3)
    layer norm1: BatchNorm(32)
    layer conv2: Conv2D(32, 64, 3)
    layer hidden: Linear(28, 128)
    layer act: ReLU(128)
    layer drop: Dropout(0.5)
    layer out: Linear(128, 10)
    forward { out(drop(act(hidden(conv2(norm1(conv1)))))) }
}

train cifar_train {
    net: ImageClassifier
    dataset: "cifar10"
    optimizer: adam
    loss: cross_entropy
    epochs: 50
    batch_size: 128
}

+f main() -> i32 / io, gpu {
    p"training ImageClassifier on cifar10"
    0
}
```

The `net` block is identical: layer names, kinds and the `forward` expression
are the same in both modes. What changes is `pub fn` → `+f`, `val` → `v`, and
`println(…)` → `p"…"`. Note the shape check is real — a `Linear` whose input
dim disagrees with the preceding layer's last dimension is an error naming
both.

### E.2 Architecture search

**Human (Invalid MAGE today: `evolve` was retired in plan 9.2, E0554):**
```mg
pub enum LayerGene {
    Dense(u32),
    Conv2d(u32, u32),
    Attention(u32, u32),
    Skip,
}

pub struct ArchGenome {
    layers: [LayerGene]~,
    lr: f64,
    dropout: f64,
}

fn score(genome: ArchGenome) -> f64 {
    genome.lr * (1.0 - genome.dropout)
}

evolve NeuralArchSearch {
    genome: ArchGenome
    population: 200
    generations: 500

    fitness { score(@ArchGenome { layers: [Skip], lr: 0.01, dropout: 0.5 }) }
    select { 8 }
    crossover { 0.7 }
    mutate { 0.02 }
}
```

**Agent (Invalid MAGE today: `evolve` was retired in plan 9.2, E0554):**
```mg
+E LayerGene {
    Dense(u32),
    Conv2d(u32, u32),
    Attention(u32, u32),
    Skip,
}

+S ArchGenome {
    layers: [LayerGene]~,
    lr: f64,
    dropout: f64,
}

f score(genome: ArchGenome) -> f64 {
    genome.lr * (1.0 - genome.dropout)
}

evolve NeuralArchSearch {
    genome: ArchGenome
    population: 200
    generations: 500

    fitness { score(@ArchGenome { layers: [Skip], lr: 0.01, dropout: 0.5 }) }
    select { 8 }
    crossover { 0.7 }
    mutate { 0.02 }
}
```

`pub struct` → `+S`, `pub enum` → `+E`, `fn` → `f`. The `evolve` block is the
same in both: its fields are fixed and its strategies are blocks.

### E.3 A code-review swarm

**Human:**
```mg
kb StyleRules {
    fact max_line_length(120);
    fact max_fn_lines(50);
    rule violation(line: str) { max_line_length(line) }
}

agent CodeReviewer {
    capabilities: [llm, fs]
}

swarm ReviewTeam {
    agent: CodeReviewer
    size: 4
    topology: mesh
    consensus: majority
}

pub struct Review { file: str, analysis: str, score: i32 }

fn review_file(path: str) -> Review / fs, llm {
    val code = fs.read_to_string(path)
    val analysis = llm.generate(f"Review this code:\n{code}")
    @Review { file: path, analysis: analysis, score: len(code) as i32 }
}

pub fn review_codebase(files: [str]~) -> [Review]~ / fs, llm {
    map(files, |file| review_file(file))
}
```

**Agent:**
```mg
kb StyleRules {
    fact max_line_length(120);
    fact max_fn_lines(50);
    rule violation(line: s) { max_line_length(line) }
}

agent CodeReviewer {
    capabilities: [llm, fs]
}

swarm ReviewTeam {
    agent: CodeReviewer
    size: 4
    topology: mesh
    consensus: majority
}

+S Review { file: s, analysis: s, score: i32 }

f review_file(path: s) -> Review / fs, llm {
    v code = fs.read_to_string(path)
    v analysis = llm.generate(f"Review this code:\n{code}")
    @Review { file: path, analysis: analysis, score: len(code) as i32 }
}

+f review_codebase(files: [s]~) -> [Review]~ / fs, llm {
    map(files, |file| review_file(file))
}
```

`kb`, `agent` and `swarm` blocks are identical across modes. The type `str`
is spelled `s` in agent mode, and the effect annotations — `/ fs, llm` — are
the same, because they are the part that has to be checked rather than
compressed.

---


*End of MAGE (Machine Genetics) Language Specification v1.0.0*
