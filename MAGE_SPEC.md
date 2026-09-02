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
16. [Appendix A: Full Grammar in BNF](#appendix-a-full-grammar-in-bnf)
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

| Mode      | Pragma              | Purpose                              | Density |
| --------- | ------------------- | ------------------------------------ | ------- |
| **Human** | (default)           | Human-readable, terse keywords       | 1×      |
| **Agent** | `#![syntax(agent)]` | Machine-optimized, symbol-compressed | ~3×     |

A `#![syntax(agent)]` pragma at the top of a `.mg` file selects Agent mode. Human is the default.

Both modes are byte-for-byte round-trippable via `mg fmt --human` and `mg fmt --agent`. The compiler accepts both in the same project.

### 2.1 Human mode Keywords

Human mode uses clear, modern keywords that improve on Rust's conventions. Core declarations use `val`/`var` (instead of `let`/`let mut`), `data` (unifying `struct` and `enum`), and `extend` (instead of `impl`). Collection and wrapper types use lightweight literal syntax: `[T]~` for growable arrays, `?T` for optionals, `T or E` for error unions, `{K: V}` for maps. Modern control constructs include `guard`, `defer`, `|>` pipeline, and `is` pattern checks.

**Core declarations (Rust-compatible):**
`fn`, `val`, `var`, `const`, `data`, `trait`, `extend`, `mod`, `use`,
`pub`, `type`, `static`, `unsafe`, `async`

**Control flow (Rust-compatible):**
`if`, `else`, `for`, `in`, `match`, `loop`, `while`, `break`, `continue`,
`return`, `yield`

**Shared with Rust (unchanged):**
`true`, `false`, `as`, `self`, `Self`, `crate`, `super`, `extern`

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
| `static`       | `static`       | Identical |
| `unsafe`       | `unsafe`       | Identical |
| `async`        | `async`        | Identical |
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
| `&mut`         | `&mut`         | Identical |
| `impl X for Y`  | `extend X for Y`| Trait implementation                   |

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
| Static         | `static`       | `Z`        | Declaration    |
| Neural net     | `net`          | `Ψ`        | AI — Neural    |
| Layer          | `layer`        | `λ`        | AI — Neural    |
| Tensor         | `Tensor`       | `Φ`        | AI — Algebra   |
| Parameter      | `Param`        | `Π`        | AI — Algebra   |
| Train          | `train`        | `Θ`        | AI — Learning  |
| Gradient       | `grad`         | `∇`        | AI — Algebra   |
| Agent          | `agent`        | `α`        | AI — Agent     |
| Swarm          | `swarm`        | `Σ`        | AI — Agent     |
| Knowledge base | `kb`           | `κ`        | AI — Symbolic  |
| Rule           | `rule`         | `ρ`        | AI — Symbolic  |
| Fact           | `fact`         | `⊢`        | AI — Symbolic  |
| Evolve         | `evolve`       | `Ω`        | AI — Evolution |
| Genome         | `Genome`       | `Γ`        | AI — Evolution |
| Fitness        | `fitness`      | `φ`        | AI — Evolution |
| Policy         | `Policy`       | `Ξ`        | AI — RL        |
| Reward         | `reward`       | `ψ`        | AI — RL        |
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
| Yield          | `yield`        | `yl`       | Control        |
| Effect         | `effect`       | `fx`       | Effects        |
| Handle         | `handle`       | `hx`       | Effects        |
| Spec           | `spec`         | `sp`       | Contracts      |
| Extern         | `extern`       | `xn`       | FFI            |
| Await          | `.await`       | `.w`       | Async          |
| Async          | `async`        | `af`       | Async          |
| Unsafe         | `unsafe`       | *(elided)* | Safety→SKB     |
| Where          | `where`        | `~>`       | Clause         |
| Mutable ref    | `&mut`         | `&m`       | Type           |
| True / False   | `true`/`false` | `1b`/`0b`  | Literal        |
| Matmul         | `@`            | `⊗`        | Tensor op      |
| Hadamard       | `.*`           | `⊙`        | Tensor op      |
| Transpose      | `.T`           | `⊤`        | Tensor op      |
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

---

## 3. Lexical Grammar

### 3.1 Source Encoding

```
source_file = BOM? PRAGMA? { token | whitespace | comment }* EOF ;
BOM         = '\u{FEFF}' ;
PRAGMA      = '#![syntax(agent)]' ;
```

### 3.2 Whitespace and Comments

```
whitespace    = ( ' ' | '\t' | '\n' | '\r' )+ ;
comment       = line_comment | block_comment ;
line_comment  = '//' { any_char - '\n' }* '\n' ;
block_comment = '/*' { any_char | block_comment }* '*/' ;  /* nestable */
```

### 3.3 Keywords

**Human mode keywords:**

```
keyword =
    /* Core declarations */
    | 'fn' | 'val' | 'var' | 'const' | 'data' | 'trait'
    | 'extend' | 'mod' | 'use' | 'type' | 'static'
    /* Visibility */
    | 'pub'
    /* Control flow */
    | 'if' | 'else' | 'for' | 'in' | 'match' | 'loop' | 'while'
    | 'break' | 'continue' | 'return' | 'yield'
    /* Boolean */
    | 'true' | 'false'
    /* Async */
    | 'async'   /* async */
    /* Special */
    | 'as' | 'where' | 'self' | 'Self' | 'crate' | 'super'
    /* Neural AI */
    | 'net' | 'layer' | 'tensor' | 'param' | 'train' | 'grad'
    /* Symbolic AI */
    | 'kb' | 'fact' | 'rule' | 'query'
    /* Evolutionary AI */
    | 'evolve' | 'genome' | 'mutate' | 'fitness' | 'select' | 'crossover'
    /* Reinforcement learning */
    | 'rl' | 'policy' | 'reward'
    /* Effects & contracts */
    | 'effect' | 'handle' | 'spec'
    /* Agents & swarms */
    | 'agent' | 'swarm'
    /* FFI & safety */
    | 'extern' | 'unsafe'
    ;
```

**Agent mode keywords** (mapped to human — see Appendix B):

```
agent_keyword =
    /* Core declarations */
    | 'f' | 'v' | 'm' | 'c' | 'D' | 'T' | 'xd' | 'M' | 'U' | 'u'
    | '+' | '~'
    /* Neural AI */
    | 'Ψ' | 'λ' | 'Φ' | 'Π' | 'Θ' | '∇'
    /* Agent */
    | 'α'
    /* Swarm */
    | 'Σ' | 'sw' /* swarm */
    /* Symbolic AI */
    | 'κ' | 'ρ'
    /* Evolution */
    | 'Ω' | 'Γ' | 'φ'
    /* RL */
    | 'Ξ' | 'ψ'
    /* Control flow */
    | '?' | '@' | '@@' | '@w' | ':' | ':?' | 'ret' | '1b' | '0b'
    /* Control flow — compressed */
    | '!' /* break */ | '>>' /* continue */ | 'yl' /* yield */
    /* Tensor ops */
    | '⊗' | '⊙' | '⊤' | '⊥'
    /* Effects & contracts — compressed */
    | 'fx' /* effect */ | 'hx' /* handle */ | 'sp' /* spec */
    /* FFI */
    | 'xn' /* extern */
    /* Async */
    | '.w' /* .await */
    /* Safety — elided (handled by compiler SKB) */
    /* 'raw' is NEVER needed in agent mode */
    ;
```

### 3.4 Identifiers

```
identifier     = XID_START { XID_CONTINUE }* ;
raw_identifier = 'r#' identifier ;
XID_START      = /* Unicode XID_Start */ | '_' ;
XID_CONTINUE   = /* Unicode XID_Continue */ | '_' ;
```

### 3.5 Literals

```
literal = int_literal | float_literal | string_literal | char_literal
        | bool_literal | byte_literal | byte_string_literal
        | tensor_literal ;

/* Integer literals */
int_literal   = dec_literal | hex_literal | oct_literal | bin_literal ;
dec_literal   = DEC_DIGIT { DEC_DIGIT | '_' }* [ int_suffix ] ;
hex_literal   = '0x' HEX_DIGIT { HEX_DIGIT | '_' }* [ int_suffix ] ;
oct_literal   = '0o' OCT_DIGIT { OCT_DIGIT | '_' }* [ int_suffix ] ;
bin_literal   = '0b' BIN_DIGIT { BIN_DIGIT | '_' }* [ int_suffix ] ;
int_suffix    = 'i8' | 'i16' | 'i32' | 'i64' | 'i128' | 'isize'
              | 'u8' | 'u16' | 'u32' | 'u64' | 'u128' | 'usize' ;

/* Float literals */
float_literal = DEC_DIGIT { DEC_DIGIT | '_' }* '.' DEC_DIGIT { DEC_DIGIT | '_' }*
                [ exponent ] [ float_suffix ] ;
exponent      = ( 'e' | 'E' ) [ '+' | '-' ] DEC_DIGIT { DEC_DIGIT | '_' }* ;
float_suffix  = 'f32' | 'f64' ;

/* String literals */
string_literal = '"' { string_char }* '"' ;
format_string  = 'format!' '(' '"' { string_char | '{' expression '}' }* '"' ')' ;
print_string   = 'println!' '(' '"' { string_char | '{' expression '}' }* '"' ')' ;
raw_string     = 'r"' { any_char - '"' }* '"'
               | 'r#"' { any_char }* '"#' ;
string_char    = any_char - ( '"' | '\\' ) | escape_sequence ;
escape_sequence = '\\' ( 'n' | 'r' | 't' | '\\' | '\'' | '"' | '0'
                | 'x' HEX_DIGIT HEX_DIGIT
                | 'u{' HEX_DIGIT{1,6} '}' ) ;

/* Character and byte literals */
char_literal         = '\'' ( any_char - ( '\'' | '\\' ) | escape_sequence ) '\'' ;
bool_literal         = 'true' | 'false' ;
byte_literal         = "b'" ( BYTE_CHAR | byte_escape ) "'" ;
byte_string_literal  = 'b"' { BYTE_CHAR | byte_escape }* '"' ;

/* Tensor literal — inline matrix/vector data */
tensor_literal = 'tensor!' '[' tensor_data ']' ;
tensor_data    = expression { ',' expression }*
               | '[' tensor_data ']' { ',' '[' tensor_data ']' }* ;
```

### 3.6 Operators and Punctuation

```
/* Arithmetic */
PLUS = '+' ;  MINUS = '-' ;  STAR = '*' ;  SLASH = '/' ;  PERCENT = '%' ;

/* Comparison */
EQ = '==' ;  NEQ = '!=' ;  LT = '<' ;  GT = '>' ;  LE = '<=' ;  GE = '>=' ;

/* Logical */
AND = '&&' ;  OR = '||' ;  NOT = '!' ;

/* Bitwise */
BIT_AND = '&' ;  BIT_OR = '|' ;  BIT_XOR = '^' ;  SHL = '<<' ;  SHR = '>>' ;

/* Tensor / linear algebra (Human mode) */
MATMUL    = '@' ;    /* matrix multiplication: A @ B */
HADAMARD  = '.*' ;   /* element-wise multiply: A .* B */
TRANSPOSE = '.T' ;   /* transpose: A.T */
PIPE      = '|>' ;   /* pipeline operator: x |> f |> g */
IS        = 'is' ;   /* pattern check: x is Some(v) */

/* Assignment */
ASSIGN = '=' ;  PLUS_EQ = '+=' ;  MINUS_EQ = '-=' ;  STAR_EQ = '*=' ;
SLASH_EQ = '/=' ;  PERCENT_EQ = '%=' ;

/* Delimiters */
LPAREN = '(' ;  RPAREN = ')' ;  LBRACE = '{' ;  RBRACE = '}' ;
LBRACK = '[' ;  RBRACK = ']' ;

/* Punctuation */
SEMI = ';' ;  COMMA = ',' ;  DOT = '.' ;  COLON = ':' ;
ARROW = '->' ;  FAT_ARROW = '=>' ;  QUESTION = '?' ;
HASH = '#' ;  AT = '@' ;  DOTDOT = '..' ;  DOTDOTEQ = '..=' ;
SCOPE = '::' ;  LT_ANGLE = '<' ;  GT_ANGLE = '>' ;
```

---

## 4. Syntactic Grammar

All productions use Human mode. Agent equivalents are in Appendix B.

### 4.1 Compilation Unit

```
compilation_unit = { item }* ;

item = { attribute }* visibility? item_kind ;

visibility = 'pub' [ '(' 'crate' ')' ] ;

item_kind = function_def | data_def | data_def | trait_def | extend_block
          | module_def | use_decl | type_alias | const_def | static_def
          | effect_def | spec_def
          | net_def | kb_def | evolve_def | agent_def
          ;
```

### 4.2 Function Definitions

```
function_def = 'fn' IDENT [ generic_params ] '(' [ param_list ] ')'
               [ '->' type ] [ where_clause ] [ effect_annotation ]
               block ;

async_function_def = 'async' function_def ;

param_list   = param { ',' param }* [ ',' ] ;
param        = IDENT ':' type [ '=' expression ] ;
self_param   = '&' 'self' | '&' 'mut' 'self' | 'self' ;

generic_params    = '<' generic_param { ',' generic_param }* '>' ;
generic_param     = IDENT [ ':' type_bound_list ] [ '=' type ] ;
type_bound_list   = type_bound { '+' type_bound }* ;

where_clause      = 'where' where_predicate { ',' where_predicate }* ;
where_predicate   = type ':' type_bound_list ;

effect_annotation = '/' effect_name { ',' effect_name }* ;
```

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

### 4.3 Data Types

```
data_def = 'data' IDENT [ generic_params ] [ where_clause ]
             ( '{' { struct_field }* '}' | '(' type_list ')' ';' | ';' ) ;
struct_field = visibility? IDENT ':' type [ ',' ] ;

data_def = 'data' IDENT [ generic_params ] [ where_clause ]
           '{' enum_variant { ',' enum_variant }* [ ',' ] '}' ;
enum_variant = IDENT [ '(' type_list ')' | '{' struct_field_list '}' | '=' expression ] ;

trait_def = 'trait' IDENT [ generic_params ] [ ':' type_bound_list ] [ where_clause ]
            '{' { trait_item }* '}' ;
trait_item = 'fn' IDENT [ generic_params ] '(' [ self_param [ ',' param_list ] ] ')'
             [ '->' type ] [ block | ';' ]
           | 'type' IDENT [ ':' type_bound_list ] [ '=' type ] ';'
           | 'const' IDENT ':' type [ '=' expression ] ';' ;

extend_block = 'extend' [ generic_params ] type [ 'for' type ] [ where_clause ]
             '{' { extend_item }* '}' ;
extend_item  = visibility? ( function_def | type_alias | const_def ) ;
```

### 4.4 Modules and Imports

```
module_def = 'mod' IDENT ( '{' { item }* '}' | ';' ) ;
use_decl   = 'use' use_path ';' ;
use_path   = path_segment { '::' path_segment }* [ '::' ( '*' | '{' use_tree_list '}' ) ] ;
```

### 4.5 Types

```
type = type_path | '&' type | '&' 'mut' type
     | 'Box' '<' type '>'      | 'Rc' '<' type '>'    | 'Arc' '<' type '>'
     | '[' type ']' '~'              /* growable array */
     | '?' type | 'Result' '<' type ',' type '>'
     | 'HashMap' '<' type ',' type '>' | 'HashSet' '<' type '>'
     | 'Tensor' '<' type ',' shape '>'     /* tensor type */
     | 'Param' '<' type ',' shape '>'      /* learnable parameter */
     | 'Genome' '<' type '>'               /* genome type */
     | 'Policy' '<' type ',' type '>'      /* RL policy */
     | 'KnowledgeBase'                     /* knowledge base */
     | 'LLM'                               /* language model handle */
     | '[' type ';' expression ']'  | '&' '[' type ']'
     | '(' [ type { ',' type }* ] ')'
     | 'fn' '(' [ type_list ] ')' [ '->' type ]
     | '*const' type | '*mut' type
     | '!' | '_' | 'String' | 'str'
     ;

shape = '[' int_literal { ',' int_literal }* ']' | '_' ;
```

### 4.6 Expressions

```
expression = literal | IDENT | prefix_expr | infix_expr | postfix_expr
           | call_expr | index_expr | field_expr | method_call_expr
           | struct_expr | tuple_expr | array_expr | vec_expr
           | closure_expr | if_expr | match_expr
           | loop_expr | for_expr | while_expr
           | block_expr | return_expr | break_expr | continue_expr
           | range_expr | cast_expr | await_expr | try_expr
           | pipe_expr | matmul_expr | grad_expr
           | tensor_literal | assign_expr ;

/* Pipeline: x |> f(_, y) |> g */
pipe_expr    = expression '|>' expression ;

/* Tensor matmul: A @ B */
matmul_expr  = expression '@' expression ;

/* Gradient: grad(loss, params) */
grad_expr    = 'grad' '(' expression ',' expression ')' ;

/* Standard  expressions (identical to Rust) */
prefix_expr  = ( '-' | '!' | '&' | '&' 'mut' | '*' ) expression ;
infix_expr   = expression binop expression ;
postfix_expr = expression '?' ;
call_expr    = expression '(' [ arg_list ] ')' ;
method_call_expr = expression '.' IDENT [ '<' type_args '>' ] '(' [ arg_list ] ')' ;
index_expr   = expression '[' expression ']' ;
field_expr   = expression '.' IDENT ;
struct_expr  = type_path '{' [ field_init_list ] '}' ;
closure_expr = '|' [ param_list ] '|' ( expression | block ) ;
when_expr    = 'if' expression block [ 'else' block ] ;
case_expr    = 'match' expression '{' { pattern '=>' expression ',' }* '}' ;
each_expr    = 'for' pattern 'in' expression block ;
spin_expr    = 'loop' block ;
till_expr    = 'while' expression block ;
emit_expr    = 'return' [ expression ] ;
halt_expr    = 'break' [ expression ] ;
skip_expr    = 'continue' ;
await_expr   = expression '.' 'await' ;
try_expr     = expression '?' ;
```

### 4.7 Statements

```
statement = ( 'val' | 'var' ) pattern [ ':' type ] '=' expression ';'
          | expression ';'
          | item ;
```

### 4.8 Patterns

```
pattern = literal | IDENT | '_'
        | '(' [ pattern { ',' pattern }* ] ')'
        | type_path '{' [ field_pattern { ',' field_pattern }* ] '}'
        | type_path '(' [ pattern { ',' pattern }* ] ')'
        | '[' [ pattern { ',' pattern }* ] [ '..' ] ']'
        | pattern '|' pattern
        | '&' pattern
        | literal '..' literal | literal '..=' literal ;
```

### 4.9 Attributes

```
attribute = '#' '[' attr_path [ '(' attr_args ')' ] ']'
          | '@' attr_name [ '(' attr_args ')' ] ;

/* Standard attributes: #[derive(...)], #[test], #[cfg(...)], #[inline] */
/* MAGE-specific: @req, @ens, @inv, @perf, @fx, @spec */
/* Neural: @target(gpu), @precision(f16), @batch(32) */
/* Evolution: @population(1000), @generations(500) */
```

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

The `grad` keyword computes gradients automatically:

**Design, not implementation.** `grad` is a reserved word with no expression
form: `grad(loss, w)` is a parse error (`expected expression, found KwGrad`),
and there is no computation-graph tracing in the compiler. The block below is
**invalid MAGE** today, and records the intent:

```mg
pub fn train_step(x: tensor[f32, B, 784], y: tensor[i64, B]) -> f32 / gpu {
    val loss = cross_entropy(forward(x), y)
    val grads = grad(loss, params())      // parse error today
    apply_grads(grads, 0.001)
    loss
}
```

What *does* exist is the `train` block of §5.4, which is declarative: the
optimiser, schedule and loss are fields, and the backward pass is the
runtime's business rather than something the source expresses.

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
| Gradient | `grad(loss, w)` | Autograd (§5.6) |
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
        | 'rule' IDENT '(' param_list ')' block ;

arg_list = ( IDENT | STRING | NUMBER ) { ',' ( IDENT | STRING | NUMBER ) }* ;
```

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
counts above from `builtin_rules()` in `prototype/src/skb.rs`, pinned by a
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

MAGE has first-class support for genetic algorithms, neuroevolution, and evolutionary strategies. The `evolve` block declaratively specifies population, fitness, selection, crossover, and mutation — the compiler generates optimized parallel evolution loops.

### 8.1 Evolve Block

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
7. **Borrow mode inference** — `&T` unifies shared and exclusive
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
\frac{A : \text{Tensor}\langle T, [M, K]\rangle \quad B : \text{Tensor}\langle T, [K, N]\rangle}{A \mathbin{@} B : \text{Tensor}\langle T, [M, N]\rangle} \quad \text{[T-Matmul]}
$$

$$
\frac{A : \text{Tensor}\langle T, S\rangle \quad B : \text{Tensor}\langle T, S\rangle}{A + B : \text{Tensor}\langle T, S\rangle} \quad \text{[T-TensorAdd]}
$$

$$
\frac{L : \text{Tensor}\langle T, []\rangle \quad P : \text{Vec}\langle\text{Param}\langle T, S_i\rangle\rangle}{\text{grad}(L, P) : \text{Vec}\langle\text{Tensor}\langle T, S_i\rangle\rangle} \quad \text{[T-Grad]}
$$

### 10.5 Type Inference

Bidirectional type checking with Hindley-Milner unification, extended for:
- **Shape unification**: tensor dimension variables solved via arithmetic constraints
- **Effect unification**: effect variables solved via set-union constraints
- **Genome type derivation**: crossover/mutate signatures inferred from data fields

---

## 11. Effect System

### 11.1 Overview

Every function has an effect signature. Effects are algebraic — declared, composed, and handled.

### 11.2 Standard Effects

These seventeen names are the built-in effect kinds. Each may be written in a `/ …`
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

The middle column names each effect's **domain**. It is not a table of callable
operations: a built-in effect has no `effect` block, so nothing declares
`dispatch` or `lifecycle` and calling them performs nothing. A function acquires
a built-in effect three ways, and only three:

1. **By annotation.** `/ gpu` puts `gpu` in the function's set, and it
   propagates to every caller. This works for all seventeen.
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
   | `io`    | `print` `println` `eprint` `eprintln` `write` `writeln` `read` `read_line` `read_to_string` |
   | `fs`    | `open` `create` `remove` `rename` `mkdir` `stat` |
   | `net`   | `connect` `listen` `bind` `send` `recv` |
   | `async` | `spawn` `select` |
   | `alloc` | `alloc` `dealloc` `realloc` |
   | `panic` | `panic` |
   | `env`   | `env` `get_env` `set_env` |
   | `time`  | `now` `sleep` `timeout` |

   The standard vocabulary wins every collision: `join` is the vocabulary's
   pure `([str], str) -> str`, so calling it is pure and does **not** mean a
   thread join. `ffi`, `npu`, `evolve` and `learn` attribute no names at all —
   reach them by annotation, or declare an `effect` block whose operations you
   can actually call and handle.

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

---

## 12. Contract System

### 12.1 Contract Attributes

Contracts live in a **`spec` block that shares the function's name** (`sp` in
agent mode) — not as attributes above the signature, which is a parse error:

```mg
sp withdraw {
    @req(1b)
    @ens(1b)
    @fx()
}

pub fn withdraw(balance: u64, amount: u64) -> u64 or str {
    guard balance >= amount else { ret Err("insufficient funds") }
    Ok(balance - amount)
}
```

`result` and `old` are **not in scope** inside `@ens`: express the
postcondition over the arguments, or enforce it in the body with a `guard`,
which is what the example above does. The older attribute form is recorded
here as intent and does not parse:

```mg
@req(balance >= amount, "sufficient funds")
@ens(result.balance == old.balance - amount, "correct deduction")
@perf(time: O(1))
@fx(pure)
pub fn withdraw(account: &mut Account, amount: u64) -> Receipt or Error {
    // ...
}

spec Sortable<T: Ord> {
    @req(items.len() > 0, "non-empty input");
    @ens(result.is_sorted(), "output is sorted");
    @ens(result.len() == items.len(), "preserves length");
    @perf(time: O(n * log(n)));
    @fx(pure);
}
```

### 12.2 Verification

Contracts are verified via:
1. **Static analysis** — SMT solver for decidable predicates
2. **SKB cross-reference** — matching against the 255 known safety rules
   *(designed, not built: no compiler stage consults the SKB. `types.rs`,
   `effects.rs`, `resolve.rs`, `verify.rs`, `heal.rs` and `mlir.rs` contain
   zero references to it. The rules are served to agents over RAP.)*
3. **Runtime assertion** — fallback for undecidable predicates

---

## 13. Ownership and Borrowing

MAGE preserves Rust's ownership and borrowing semantics with full inference:

1. Every value has exactly one owner.
2. When the owner goes out of scope, the value is dropped.
3. Values can be moved or, if `Copy`, duplicated.
4. Borrows: any number of `&T` (shared) XOR one `&mut T` (exclusive).
5. Borrows must not outlive the referent.
6. The compiler infers borrow mode from usage context.
7. No lifetime annotations in source code — the SKB encodes lifetime rules.

$$
\frac{x : \tau \in \Gamma \quad x \notin \text{moved}(\Gamma)}{\Gamma \vdash_{\text{own}} x : \text{Valid}} \quad \text{[Own-Valid]}
$$

$$
\frac{\Gamma \vdash_{\text{own}} x : \text{Valid} \quad \Gamma' = \Gamma[\text{moved} \cup \{x\}]}{\Gamma \vdash_{\text{own}} \text{move}(x) : \text{Valid} \dashv \Gamma'} \quad \text{[Own-Move]}
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

## Appendix A: Full Grammar in BNF

```bnf
<compilation_unit> ::= <item_list>
<item_list>        ::= <item> <item_list> | ε

<item>     ::= <attribute_list> <visibility> <item_kind>
             | <attribute_list> <item_kind>
<visibility> ::= "exp" | "exp" "(" "crate" ")"

<item_kind> ::= <function_def> | <data_def> | <data_def>
              | <trait_def> | <extend_block> | <module_def>
              | <use_decl> | <type_alias> | <const_def>
              | <static_def> | <effect_def> | <spec_def>
              | <net_def> | <kb_def> | <evolve_def> | <agent_def>
              | <train_def>

<function_def> ::= "def" IDENT <opt_generics> "(" <opt_params> ")"
                   <opt_return> <opt_where> <opt_effects> <block>
<async_fn_def> ::= "par" <function_def>

<data_def> ::= "rec" IDENT <opt_generics> <opt_where> "{" <field_list> "}"
<data_def>   ::= "sum" IDENT <opt_generics> <opt_where> "{" <variant_list> "}"
<trait_def>  ::= "sig" IDENT <opt_generics> <opt_supertrait> <opt_where>
                 "{" <trait_items> "}"
<extend_block> ::= "ext" <opt_generics> <type> <opt_on> <opt_where>
                 "{" <extend_items> "}"
<module_def> ::= "ns" IDENT "{" <item_list> "}" | "ns" IDENT ";"
<use_decl>   ::= "bring" <use_path> ";"

<net_def>    ::= "net" IDENT <opt_generics> <opt_supertrait>
                 "{" <layer_list> <opt_forward> "}"
<layer_list> ::= <layer_def> <layer_list> | ε
<layer_def>  ::= "layer" IDENT "(" <arg_list> ")"
<opt_forward> ::= <function_def> | ε

<kb_def>     ::= "kb" IDENT "{" <kb_items> "}"
<kb_items>   ::= <kb_item> <kb_items> | ε
<kb_item>    ::= "fact" IDENT "(" <arg_list> ")" ";"
               | "rule" IDENT "(" <param_list> ")" ":-" <rule_body> ";"
               | "query" IDENT "(" <param_list> ")" ":-" <rule_body> ";"

<evolve_def> ::= "evolve" IDENT "{" <evolve_fields> "}"
<evolve_fields> ::= <evolve_field> <evolve_fields> | ε
<evolve_field>  ::= "genome" ":" <type> ","
                  | "population" ":" <expression> ","
                  | "generations" ":" <expression> ","
                  | <fitness_def>
                  | "select" <strategy> ","
                  | "crossover" <strategy> ","
                  | "mutate" <strategy> ","
                  | "target" "fitness" <cmp_op> <expression> ","
                  | <function_def>

<agent_def>  ::= "agent" IDENT <opt_supertrait> "{" <agent_items> "}"
<agent_items> ::= <agent_item> <agent_items> | ε
<agent_item>  ::= IDENT ":" <type> ","
                | <visibility> <function_def>

<train_def>  ::= "train" IDENT "{" <train_fields> "}"
<train_fields> ::= <train_field> <train_fields> | ε
<train_field>  ::= IDENT ":" <expression> ","
                 | <function_def>

<type> ::= <type_path>
         | "&" <type> | "&" "var" <type>
         | "Box" "<" <type> ">"
         | "Rc" "<" <type> ">"
         | "Arc" "<" <type> ">"
         | "Vec" "<" <type> ">"
         | "Option" "<" <type> ">"
         | "Result" "<" <type> "," <type> ">"
         | "HashMap" "<" <type> "," <type> ">"
         | "HashSet" "<" <type> ">"
         | "Tensor" "<" <type> "," <shape> ">"
         | "Param" "<" <type> "," <shape> ">"
         | "Genome" "<" <type> ">"
         | "Policy" "<" <type> "," <type> ">"
         | "KnowledgeBase" | "LLM"
         | "[" <type> ";" <expression> "]"
         | "&" "[" <type> "]"
         | "(" <type_list_opt> ")"
         | "def" "(" <type_list_opt> ")" <opt_return>
         | "!" | "_" | "String" | "str"

<shape> ::= "[" <int_list> "]" | "_"
```

---

## Appendix B: Dual Syntax Mapping Table

Every Human-mode construct has an Agent-mode equivalent, and both parse to the
same AST — where both exist. **Every row below was checked against the
compiler**; the ones that do not parse are marked, because this table is the
first thing an agent reads to learn the compressed surface.

### B.1 Declaration Keywords

| Human          | Agent | Meaning           |
| -------------- | ----- | ----------------- |
| `fn`           | `f`   | Function          |
| `val`          | `v`   | Immutable binding |
| `var`          | `m`   | Mutable binding   |
| `const`        | `C`   | Constant (uppercase; lowercase `c` is an identifier) |
| `data`         | `D`   | Data declaration  |
| `data (sum)`   | `D`   | Sum type          |
| `trait`        | `T`   | Trait             |
| `extend`       | `xd`  | Extend block      |
| `mod`          | `M`   | Module            |
| `use`          | `u`   | Import            |
| `pub`          | `+`   | Public prefix     |
| `async fn`     | `af`  | Async function    |
| ~~`const fn`~~ | ~~`c f`~~ | **Neither parses.** There are no const functions |
| ~~`pub(crate)`~~ | ~~`~`~~ | **Neither parses.** Visibility is `pub`/`+` or private |

### B.2 AI Constructs

> **Status: lexed, not parsed.** Fifteen of these Greek symbols are real
> tokens (`Ψ` produces `KwPsi`, `Σ` produces `KwSigma`), and **no parser arm
> consumes any of them** — `Ψ Classifier { … }` is `expected item, found
> KwPsi`. `?:` is not even a token. The AI blocks use the **same keyword in
> both modes**: `net`, `layer`, `train`, `kb`, `fact`, `rule`, `agent`,
> `swarm`, `evolve`, `genome`, `fitness`, `select`, `crossover`, `mutate`,
> `population`, `generations`. The table is the design, and Appendix D says
> the same thing at more length.

| Human           | Agent    | Meaning             |
| --------------- | -------- | ------------------- |
| `net`           | `Ψ`      | Neural network      |
| `layer`         | `λ`      | Network layer       |
| `Tensor<T,S>`   | `Φ[T;S]` | Tensor type         |
| `Param<T,S>`    | `Π[T;S]` | Learnable parameter |
| `train`         | `Θ`      | Training block      |
| `grad`          | `∇`      | Gradient            |
| `agent`         | `α`      | Agent               |
| `swarm`         | `Σ`      | Multi-agent swarm   |
| `kb`            | `κ`      | Knowledge base      |
| `fact`          | `⊢`      | Fact assertion      |
| `rule`          | `ρ`      | Inference rule      |
| `query`         | `?:`     | KB query            |
| `evolve`        | `Ω`      | Evolution block     |
| `Genome<T>`     | `Γ[T]`   | Genome type         |
| `fitness`       | `φ`      | Fitness function    |
| `population`    | `η`      | Population size     |
| `generations`   | `∞`      | Generation count    |
| `select`        | `⊳`      | Selection operator  |
| `crossover`     | `χ`      | Crossover operator  |
| `mutate` (evol) | `μ`      | Mutation operator   |
| `target`        | `→`      | Target condition    |
| `Policy<S,A>`   | `Ξ[S,A]` | RL policy           |
| `reward`        | `ψ`      | Reward signal       |
| `LLM`           | `Λ`      | Language model      |
| `KnowledgeBase` | `Κ`      | Knowledge base type |

### B.3 Control Flow

| Human          | Agent      | Meaning       |
| -------------- | ---------- | ------------- |
| `if`           | `?`        | Conditional   |
| `else`         | `:`        | Else branch   |
| `else if`      | `: ?`      | Else-if. The space matters: `:?` is not a token |
| `match`        | `?= expr {` | Pattern match. `? expr {` also parses |
| `for x in y`   | `@ x in y` | For loop. The separator is `in`; `~` does not parse |
| `loop`         | `@@` or `loop` | Infinite loop |
| `while`        | `@w cond`  | While loop. `loop ?` does not parse |
| `return`       | `ret`      | Return        |
| `break`        | `!`        | Break         |
| `continue`     | `>>`       | Continue      |
| `continue`     | `>>`       | Continue      |
| `true`/`false` | `1b`/`0b`  | Booleans      |

### B.4 Type Syntax

| Human             | Agent     | Meaning             |
| ----------------- | --------- | ------------------- |
| `String`          | `s`       | Owned string        |
| `&str`            | `&s`      | String slice        |
| `[T]~`          | `[T]~`    | Growable array      |
| `?T`       | `?T`      | Optional            |
| `Result<T,E>`     | `R[T,E]`  | Result              |
| `^T`          | `^T`      | Heap pointer        |
| `Rc<T>`           | `$T`      | Reference counted   |
| `Arc<T>`          | `@T`      | Atomic ref counted  |
| `{K: V}`    | `{K:V}`   | Hash map            |
| `{K}`      | `{K}`     | Hash set            |
| `&mut T`        | `&!T`     | Exclusive reference |
| `fn(T)->U`      | `f(T)->U` | Function pointer    |
| `T<A>` (generics) | `T[A]`    | Generic parameters  |

### B.5 Tensor Operations

| Human                 | Agent        | Meaning          |
| --------------------- | ------------ | ---------------- |
| `A @ B`               | `A ⊗ B`      | Matrix multiply  |
| `A .* B`              | `A ⊙ B`      | Hadamard product |
| `A.T`                 | `A⊤`         | Transpose        |
| `A.flatten()`         | `A⊥`         | Flatten          |
| `grad(loss, params)`  | `∇(l,p)`     | Gradient         |
| `A \|> f \|> g`       | `A ▸ f ▸ g`  | Pipeline         |
| `dense(in, out, act)` | `δ(i,o,a)`   | Dense layer      |
| `conv2d(ci,co,k)`     | `⊞(ci,co,k)` | Conv layer       |
| `dropout(rate)`       | `∅(r)`       | Dropout layer    |

### B.6 Path and Scope

| Rust        | MAGE         | Meaning        |
| ----------- | ------------ | -------------- |
| `::`        | `.`          | Path separator. `::` does not parse in either MAGE mode |
| `Foo { x }` | `@Foo { x: 1 }` | Struct literal. The `@` goes **before** the name, every field is named, and bare `Foo { … }` is a *map* |
| ~~`crate::`~~ | ~~`~.`~~   | **Does not parse.** There is no module system, so there is no crate root, `super` or `self` path |

### B.7 Attributes

| Human               | Agent       | Meaning             |
| ------------------- | ----------- | ------------------- |
| `#[derive(...)]`    | `@d(...)`   | Derive              |
| `#[test]`           | `@t`        | Test                |
| `#[bench]`          | `@b`        | Benchmark           |
| `#[inline(always)]` | `@i!`       | Inline always       |
| `#[cfg(...)]`       | `@cfg(...)` | Conditional compile |
| `println!("...")`   | `p"..."`    | Print line          |
| `format!("...")`    | `f"..."`    | Format string       |
| `eprintln!("...")`  | `ep"..."`   | Error print         |

### B.8 Shared Syntax (Identical in Both Modes)

- The numeric types that exist (`i8`–`i128`, `u8`–`u128`, `isize`, `usize`,
  `f32`, `f64`). **`f16` and `bf16` are not type names** — the checker reports
  `unresolved type` — though `@precision(f16)` is accepted as an attribute
- Arithmetic, comparison, logical, bitwise operators
- Semicolons, braces, parentheses
- Comments (`//`, `/* */`, `///`, `//!`)
- Effect annotations (`/ io`, `/ gpu`, `/ llm`)
- Contract attributes (`@req`, `@ens`, `@inv`, `@perf`, `@fx`, `@spec`)
- Range operators (`..`, `..=`)
- Try operator (`?` postfix)
- Closures (`|x| expr`, and `fn(x) => expr`)
- ~~`tensor!` literals~~ — **there are no macros**; write a list literal

### B.9 Agent Mode Safety Philosophy

In agent mode, safety constructs are **fully handled by the compiler and SKB** (Safety Knowledge Base). The following constructs are unnecessary in agent mode:

| Human Syntax                   | Agent Mode Handling                              | SKB Rules |
| ------------------------------ | ------------------------------------------------ | --------- |
| `unsafe { ... }`               | Elided — compiler verifies via OWN/BOR/FFI       | AEL-0001  |
| `unsafe fn`                    | Elided — compiler detects from body analysis     | AEL-0002  |
| Lifetime annotations (`'a`)    | Inferred by compiler's LIF rules                 | AEL-0003  |
| `&mut T` explicit annotation   | Inferred — compiler determines mutability        | AEL-0004  |
| `Send` / `Sync` bounds         | Derived automatically from type structure        | AEL-0005  |
| `move` keyword on closures     | Inferred — compiler determines capture mode      | AEL-0006  |
| `Pin<T>` wrapping              | Handled automatically for self-referential types | AEL-0007  |
| `dyn` / `impl` dispatch choice | Compiler selects based on call-site analysis     | AEL-0008  |
| `PhantomData<T>`               | Inserted automatically by compiler               | AEL-0012  |

This design maximizes token efficiency (fewer tokens in the LLM context window) while maintaining full safety guarantees through compiler infrastructure rather than language syntax.

---

## Appendix C: Operator Precedence

From highest to lowest. Left-associative unless noted.

| Prec | Operator(s)                   | Description      | Assoc   |
| ---- | ----------------------------- | ---------------- | ------- |
| 16   | `.` field, `[i]` index        | Access           | Left    |
| 15   | `f()` call, `.m()` method     | Invocation       | Left    |
| 14   | `?`                           | Try/unwrap       | Postfix |
| 13   | `.T`                          | Transpose        | Postfix |
| 12   | `-` `!` `&` `&var` `*` `grad` | Unary prefix     | Right   |
| 11   | `as`                          | Type cast        | Left    |
| 10   | `@` `.*`                      | Matmul, Hadamard | Left    |
| 9    | `*` `/` `%`                   | Multiplicative   | Left    |
| 8    | `+` `-`                       | Additive         | Left    |
| 7    | `<<` `>>`                     | Bit shift        | Left    |
| 6    | `&`                           | Bitwise AND      | Left    |
| 5    | `^`                           | Bitwise XOR      | Left    |
| 4    | `\|`                          | Bitwise OR       | Left    |
| 3    | `==` `!=` `<` `>` `<=` `>=`   | Comparison       | Left    |
| 2    | `&&`                          | Logical AND      | Left    |
| 1    | `\|\|`                        | Logical OR       | Left    |
| 0    | `\|>`                         | Pipeline         | Left    |
| -1   | `=` `+=` `-=` `*=` etc.       | Assignment       | Right   |
| -2   | `return` `break` `yield`      | Control flow     | —       |

---

## Appendix D: Agent Mode Symbol Reference

A complete lexicon of Agent mode symbols, organized by category. This is the "genetic alphabet" of MAGE — each symbol encodes a high-level concept in minimal space.

> **Status: lexed, not parsed.** The Greek and mathematical symbols in D.1,
> D.2 and D.3 are recognised by the lexer — `Ψ` produces `KwPsi`, `Σ` produces
> `KwSigma`, and so on for fifteen of them — and **no parser arm consumes any
> of them**. `Ψ Classifier { … }` is `expected item, found KwPsi`. The agent
> mode that *works* is the one in D.4–D.6: the ASCII declaration sigils (`+f`,
> `v`, `m`, `S`, `E`, `I`, `T`), the control compressions (`?`, `?=`, `@`,
> `@@`, `!`), and the short aliases (`sw`, `topo`, `cons`, `fx`, `hx`, `gd`,
> `df`, `xd`). The AI-construct blocks — `net`, `layer`, `train`, `kb`,
> `agent`, `swarm`, `evolve` — use the **same keyword in both modes**.
>
> The tables below are the design. They are worth keeping, because the
> compression argument is the reason the language has two surfaces at all; but
> nothing in them compiles today.

### D.1 Greek Letters — AI Constructs

| Symbol | Unicode | Human           | Domain     |
| ------ | ------- | --------------- | ---------- |
| `Ψ`    | U+03A8  | `net`           | Neural     |
| `λ`    | U+03BB  | `layer`         | Neural     |
| `Φ`    | U+03A6  | `Tensor`        | Algebra    |
| `Π`    | U+03A0  | `Param`         | Algebra    |
| `Θ`    | U+0398  | `train`         | Learning   |
| `∇`    | U+2207  | `grad`          | Algebra    |
| `α`    | U+03B1  | `agent`         | Agent      |
| `Σ`    | U+03A3  | `swarm`         | Agent      |
| `κ`    | U+03BA  | `kb`            | Symbolic   |
| `ρ`    | U+03C1  | `rule`          | Symbolic   |
| `Ω`    | U+03A9  | `evolve`        | Evolution  |
| `Γ`    | U+0393  | `Genome`        | Evolution  |
| `φ`    | U+03C6  | `fitness`       | Evolution  |
| `χ`    | U+03C7  | `crossover`     | Evolution  |
| `μ`    | U+03BC  | `mutate`        | Evolution  |
| `η`    | U+03B7  | `population`    | Evolution  |
| `Ξ`    | U+039E  | `Policy`        | RL         |
| `ψ`    | U+03C8  | `reward`        | RL         |
| `Λ`    | U+039B  | `LLM`           | Neural     |
| `Κ`    | U+039A  | `KnowledgeBase` | Symbolic   |
| `σ`    | U+03C3  | `softmax`       | Activation |
| `δ`    | U+03B4  | `dense`         | Layer      |

### D.2 Mathematical Operators — Tensor Algebra

| Symbol | Unicode | Human        | Meaning          |
| ------ | ------- | ------------ | ---------------- |
| `⊗`    | U+2297  | `@`          | Matrix multiply  |
| `⊙`    | U+2299  | `.*`         | Hadamard product |
| `⊤`    | U+22A4  | `.T`         | Transpose        |
| `⊥`    | U+22A5  | `.flatten()` | Flatten          |
| `⊢`    | U+22A2  | `fact`       | Fact assertion   |
| `⊞`    | U+229E  | `conv2d`     | Convolution      |
| `∅`    | U+2205  | `dropout`    | Dropout          |

### D.3 Arrows and Flow

| Symbol | Unicode | Human         | Meaning          |
| ------ | ------- | ------------- | ---------------- |
| `→`    | U+2192  | `target`      | Target/goal      |
| `▸`    | U+25B8  | `\|>`         | Pipeline         |
| `⊳`    | U+22B3  | `select`      | Selection        |
| `∞`    | U+221E  | `generations` | Generation count |

### D.4 Declaration Sigils (from Core Language)

| Symbol   | Human            | Meaning            |
| -------- | ---------------- | ------------------ |
| `f`      | `fn`             | Function           |
| `v`      | `val`            | Variable           |
| `m`      | `var`            | Mutable variable   |
| `D`      | `data`           | Data declaration   |
| `D`      | `data (sum)`     | Sum type           |
| `T`      | `trait`          | Trait              |
| `xd`     | `extend`         | Extend block       |
| `M`      | `mod`            | Module             |
| `u`      | `use`            | Import             |
| `+`      | `pub`            | Public             |
| `~`      | `pub(crate)`     | Crate visibility   |
| `?`      | `if`/`match`     | Conditional/match  |
| `:`      | `else`           | Else               |
| `@`      | `for`            | For loop           |
| `ret`    | `return`         | Return             |
| `!`      | `break`          | Break              |
| `>>`     | `continue`       | Continue           |
| `1b`     | `true`         | Boolean true       |
| `0b`     | `false`        | Boolean false      |
| `s`      | `String`       | String type        |
| `&s`     | `&str`         | String slice       |
| `[T]~`   | `[T]~`       | Vector             |
| `?T`     | `?T`    | Optional           |
| `R[T,E]` | `Result<T,E>`  | Result             |
| `^T`     | `^T`       | Heap box           |
| `$T`     | `Rc<T>`        | Ref counted        |
| `@T`     | `Arc<T>`       | Atomic ref counted |
| `{K:V}`  | `{K: V}` | Hash map           |
| `{K}`    | `{K}`   | Hash set           |
| `&!T`    | `&mut T`       | Mutable reference  |
| `.`      | `::`           | Path separator     |
| `~.`     | `crate::`      | Crate root         |
| `@d()`   | `#[derive()]`  | Derive             |
| `@t`     | `#[test]`      | Test               |
| `p""`    | `println!()`   | Print              |
| `f""`    | `format!()`    | Format             |

### D.5 Control Flow & Effect Compressions (Agent Mode)

| Symbol | Human     | Meaning              |
| ------ | --------- | -------------------- |
| `@@`   | `loop`      | Infinite loop        |
| `@w`   | `while`     | While loop           |
| `!`    | `break`     | Break from loop      |
| `>>`   | `continue`  | Continue loop        |
| `ret`  | `return`    | Return value         |
| `yl`   | `yield`     | Yield from generator |
| `fx`   | `effect`    | Effect declaration   |
| `hx`   | `handle`    | Effect handler       |
| `sp`   | `spec`      | Spec/contract block  |
| `xn`   | `extern`    | FFI extern block     |
| `.w`   | `.await`    | Async await          |
| `?=`   | `match`     | Pattern match        |
| `:?`   | `else if`   | Else-if chain        |
| `sw`   | `swarm`     | Multi-agent swarm    |

### D.6 Safety Elision (Agent Mode — Handled by Compiler)

In agent mode, the following constructs have **no syntax** — the compiler's SKB handles them:

| Human Syntax     | Agent Equivalent | Compiler Handling        |
| ---------------- | ---------------- | ------------------------ |
| `unsafe { ... }` | `{ ... }`        | SKB verifies operations  |
| `unsafe fn`      | `f`              | Compiler detects unsafe  |
| `'a` lifetimes   | *(omitted)*      | LIF rules infer all      |
| `Send + Sync`    | *(omitted)*      | CON rules derive bounds  |
| `Pin<T>`         | *(omitted)*      | Compiler wraps as needed |
| `PhantomData<T>` | *(omitted)*      | Compiler inserts marker  |
| `move \|x\|`     | `\|x\|`          | Capture mode inferred    |

### D.7 Swarm Constructs

| Human Field  | Agent Field | Meaning |
| ------------ | ----------- | ------- |
| `swarm`      | `sw`        | Swarm definition |
| `agent:`     | `agent:`    | Agent type in the swarm |
| `size:`      | `size:`     | Number of members |
| `topology:`  | `topo:`     | Communication topology |
| `consensus:` | `cons:`     | Consensus strategy |

Four fields, and **no behaviour blocks**: `dispatch`, `aggregate` and
`on_failure` do not exist — a swarm block declares the group, and the work is
ordinary functions with `map` and `fold` (§9.2). `Σ` is a reserved token that
no parser arm consumes; `sw` is the agent-mode spelling that works.

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

**Human:**
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

**Agent:**
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
