# Machine Genetic Code (MechGen) Language Specification

**Version**: 1.0.0 (Draft)
**Status**: Pre-implementation specification

---

> *Just as DNA encodes biological life through a compact molecular language,*
> *MechGen encodes intelligent systems through a compact computational language.*
> *It is the genetic code for machines — a language in which AI writes, reasons,*
> *optimizes, and evolves itself.*

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

Machine Genetic Code (MechGen) is a systems programming language designed for the age of artificial intelligence. It fuses the safety and performance of Rust with first-class primitives for neural computation, symbolic reasoning, evolutionary optimization, and multi-agent coordination — all within a dual-syntax system that serves both human programmers and AI agents.

### 1.1 Design Principles

1. **Intelligence is a first-class construct.** Neural networks, knowledge bases, rule engines, and evolutionary algorithms are language-level declarations, not library calls. The compiler understands their structure, verifies their types, and targets optimal hardware.

2. **Dual encoding.** Human mode uses clean C-family keywords that any programmer can read. Agent mode compresses every concept into minimal symbols — Greek letters for AI constructs, mathematical operators for tensor algebra — achieving the density of hexadecimal applied to intelligence.

3. **Safety without ceremony.** Ownership, borrowing, and lifetimes are enforced but fully inferred. No lifetime annotations, no `PhantomData`, no `Pin`. The Safety Knowledge Base (SKB) encodes 9,157 rules that the compiler applies automatically. In agent mode, **all safety constructs are handled by the compiler and SKB** — `unsafe` blocks, lifetime annotations, `Send`/`Sync` bounds, and `Pin<T>` are entirely elided from the language surface, maximizing token efficiency while the compiler maintains full safety guarantees.

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

MechGen supports two interchangeable surface syntaxes that parse to the same AST:

| Mode      | Pragma              | Purpose                              | Density |
| --------- | ------------------- | ------------------------------------ | ------- |
| **Human** | (default)           | Human-readable, C-family keywords    | 1×      |
| **Agent** | `#![syntax(agent)]` | Machine-optimized, symbol-compressed | ~3×     |

A `#![syntax(agent)]` pragma at the top of a `.mg` file selects Agent mode. Human is the default.

Both modes are byte-for-byte round-trippable via `mg fmt --human` and `mg fmt --agent`. The compiler accepts both in the same project.

### 2.1 Human mode Keywords

Human mode uses C-family keywords plus MechGen-unique AI constructs:

**From C/C++/Rust** (familiar to any systems programmer):
`fn`, `let`, `mut`, `const`, `struct`, `enum`, `trait`, `impl`, `mod`, `use`,
`pub`, `if`, `else`, `for`, `in`, `match`, `loop`, `while`, `break`, `continue`,
`return`, `yield`, `true`, `false`, `async`, `await`, `as`, `where`, `self`,
`Self`, `crate`, `super`, `type`, `static`, `extern`, `unsafe`

**MechGen-unique — AI constructs:**
`net`, `layer`, `tensor`, `param`, `train`, `grad`, `agent`, `kb`, `fact`,
`rule`, `query`, `evolve`, `genome`, `mutate`, `fitness`, `select`, `crossover`,
`rl`, `policy`, `reward`, `effect`, `handle`, `spec`

### 2.2 Agent mode Symbols

Agent mode maps every concept to 1-2 characters. Like hexadecimal compresses 4 bits into one symbol, Agent mode compresses high-level concepts into atomic glyphs:

| Concept        | Human          | Agent     | Category       |
| -------------- | -------------- | --------- | -------------- |
| Function       | `fn`           | `f`       | Declaration    |
| Public         | `pub`          | `+`       | Visibility     |
| Variable       | `let`          | `v`       | Declaration    |
| Mutable        | `let mut`      | `m`       | Declaration    |
| Struct         | `struct`       | `S`       | Declaration    |
| Enum           | `enum`         | `E`       | Declaration    |
| Trait          | `trait`        | `T`       | Declaration    |
| Impl           | `impl`         | `I`       | Declaration    |
| Module         | `mod`          | `M`       | Declaration    |
| Use            | `use`          | `u`       | Declaration    |
| Neural net     | `net`          | `Ψ`       | AI — Neural    |
| Layer          | `layer`        | `λ`       | AI — Neural    |
| Tensor         | `Tensor`       | `Φ`       | AI — Algebra   |
| Parameter      | `Param`        | `Π`       | AI — Algebra   |
| Train          | `train`        | `Θ`       | AI — Learning  |
| Gradient       | `grad`         | `∇`       | AI — Algebra   |
| Agent          | `agent`        | `α`       | AI — Agent     |
| Knowledge base | `kb`           | `κ`       | AI — Symbolic  |
| Rule           | `rule`         | `ρ`       | AI — Symbolic  |
| Fact           | `fact`         | `⊢`       | AI — Symbolic  |
| Evolve         | `evolve`       | `Ω`       | AI — Evolution |
| Genome         | `Genome`       | `Γ`       | AI — Evolution |
| Fitness        | `fitness`      | `φ`       | AI — Evolution |
| Policy         | `Policy`       | `Ξ`       | AI — RL        |
| Reward         | `reward`       | `ψ`       | AI — RL        |
| If             | `if`           | `?`       | Control        |
| Else           | `else`         | `:`       | Control        |
| Match          | `match`        | `?=`      | Control        |
| For            | `for`          | `@`       | Control        |
| Loop           | `loop`         | `@@`      | Control        |
| While          | `while`        | `@w`      | Control        |
| Break          | `break`        | `!`       | Control        |
| Continue       | `continue`     | `>>`      | Control        |
| Return         | `return`       | `ret`     | Control        |
| Yield          | `yield`        | `yl`      | Control        |
| Effect         | `effect`       | `fx`      | Effects        |
| Handle         | `handle`       | `hx`      | Effects        |
| Spec           | `spec`         | `sp`      | Contracts      |
| Extern         | `extern`       | `xn`      | FFI            |
| Await          | `.await`       | `.w`      | Async          |
| Unsafe         | `unsafe`       | *(elided)*| Safety→SKB     |
| True / False   | `true`/`false` | `1b`/`0b` | Literal        |
| Matmul         | `@`            | `⊗`       | Tensor op      |
| Hadamard       | `.*`           | `⊙`       | Tensor op      |
| Transpose      | `.T`           | `⊤`       | Tensor op      |
| Flatten        | `.flatten()`   | `⊥`       | Tensor op      |
| String         | `String`       | `s`       | Type           |
| `&str`         | `&str`         | `&s`      | Type           |
| `Vec<T>`       | `Vec<T>`       | `[T]~`    | Type           |
| `Option<T>`    | `Option<T>`    | `?T`      | Type           |
| `Result<T,E>`  | `Result<T,E>`  | `R[T,E]`  | Type           |
| `Box<T>`       | `Box<T>`       | `^T`      | Type           |
| `HashMap<K,V>` | `HashMap<K,V>` | `{K:V}`   | Type           |
| Path separator | `::`           | `.`       | Path           |

See [Appendix D](#appendix-d-Agent-mode-symbol-reference) for the complete symbol table.

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
    | 'fn' | 'let' | 'mut' | 'const' | 'struct' | 'enum' | 'trait'
    | 'impl' | 'mod' | 'union' | 'use' | 'type' | 'static'
    /* Visibility */
    | 'pub'
    /* Control flow */
    | 'if' | 'else' | 'for' | 'in' | 'match' | 'loop' | 'while'
    | 'break' | 'continue' | 'return' | 'yield'
    /* Boolean */
    | 'true' | 'false'
    /* Async */
    | 'async' | 'await'
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
    /* FFI & safety */
    | 'extern' | 'unsafe'
    ;
```

**Agent mode keywords** (mapped to human — see Appendix B):

```
agent_keyword =
    /* Core declarations */
    | 'f' | 'v' | 'm' | 'c' | 'S' | 'E' | 'T' | 'I' | 'M' | 'U' | 'u'
    | '+' | '~'
    /* Neural AI */
    | 'Ψ' | 'λ' | 'Φ' | 'Π' | 'Θ' | '∇'
    /* Agent */
    | 'α'
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
    /* 'unsafe' is NEVER needed in agent mode */
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

item_kind = function_def | struct_def | enum_def | trait_def | impl_block
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
param        = IDENT ':' type ;
self_param   = '&' 'self' | '&' 'mut' 'self' | 'self' ;

generic_params    = '<' generic_param { ',' generic_param }* '>' ;
generic_param     = IDENT [ ':' type_bound_list ] [ '=' type ] ;
type_bound_list   = type_bound { '+' type_bound }* ;

where_clause      = 'where' where_predicate { ',' where_predicate }* ;
where_predicate   = type ':' type_bound_list ;

effect_annotation = '/' effect_name { ',' effect_name }* ;
```

### 4.3 Data Types

```
struct_def = 'struct' IDENT [ generic_params ] [ where_clause ]
             ( '{' { struct_field }* '}' | '(' type_list ')' ';' | ';' ) ;
struct_field = visibility? IDENT ':' type [ ',' ] ;

enum_def = 'enum' IDENT [ generic_params ] [ where_clause ]
           '{' enum_variant { ',' enum_variant }* [ ',' ] '}' ;
enum_variant = IDENT [ '(' type_list ')' | '{' struct_field_list '}' | '=' expression ] ;

trait_def = 'trait' IDENT [ generic_params ] [ ':' type_bound_list ] [ where_clause ]
            '{' { trait_item }* '}' ;
trait_item = 'fn' IDENT [ generic_params ] '(' [ self_param [ ',' param_list ] ] ')'
             [ '->' type ] [ block | ';' ]
           | 'type' IDENT [ ':' type_bound_list ] [ '=' type ] ';'
           | 'const' IDENT ':' type [ '=' expression ] ';' ;

impl_block = 'impl' [ generic_params ] type [ 'for' type ] [ where_clause ]
             '{' { impl_item }* '}' ;
impl_item  = visibility? ( function_def | type_alias | const_def ) ;
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
     | 'Vec' '<' type '>'      | 'Option' '<' type '>' | 'Result' '<' type ',' type '>'
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
if_expr      = 'if' expression block [ 'else' block ] ;
match_expr   = 'match' expression '{' { pattern '=>' expression ',' }* '}' ;
for_expr     = 'for' pattern 'in' expression block ;
loop_expr    = 'loop' block ;
while_expr   = 'while' expression block ;
return_expr  = 'return' [ expression ] ;
await_expr   = expression '.' 'await' ;
try_expr     = expression '?' ;
```

### 4.7 Statements

```
statement = 'let' [ 'mut' ] pattern [ ':' type ] '=' expression ';'
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
/* MechGen-specific: @req, @ens, @inv, @perf, @fx, @spec */
/* Neural: @target(gpu), @precision(f16), @batch(32) */
/* Evolution: @population(1000), @generations(500) */
```

---

## 5. Neural Computation

MechGen treats neural networks as first-class language constructs. A `net` block declares a network architecture; `layer` statements define its topology; `train` blocks define optimization loops. The compiler verifies shape compatibility, selects hardware targets, and generates optimized kernels.

### 5.1 Network Definition

```mg
// Human mode
net Classifier {
    layer dense(784, 256, relu)
    layer dropout(0.3)
    layer dense(256, 128, relu)
    layer dense(128, 10, softmax)
}
```

```mg
// Agent mode
Ψ Classifier {
    λ δ(784, 256, relu)
    λ ∅(0.3)
    λ δ(256, 128, relu)
    λ δ(128, 10, softmax)
}
```

**Grammar:**

```
net_def = 'net' IDENT [ generic_params ] [ ':' type_bound_list ]
          '{' { layer_def }* [ forward_def ] '}' ;

layer_def = 'layer' layer_kind '(' layer_args ')' ;

layer_kind = 'dense' | 'conv2d' | 'conv3d' | 'lstm' | 'gru'
           | 'attention' | 'multihead_attention'
           | 'embedding' | 'layernorm' | 'batchnorm'
           | 'dropout' | 'flatten' | 'reshape'
           | 'residual' | 'pool2d' | IDENT ;   /* extensible */

forward_def = 'fn' 'forward' '(' param_list ')' '->' type block ;
```

### 5.2 Layer Types

| Layer                                        | Parameters                        | Description            |
| -------------------------------------------- | --------------------------------- | ---------------------- |
| `dense(in, out, act)`                        | Input dim, output dim, activation | Fully connected        |
| `conv2d(ch_in, ch_out, kernel, stride, pad)` | Channels, kernel size             | 2D convolution         |
| `conv3d(...)`                                | Same pattern for 3D               | 3D convolution         |
| `lstm(in, hidden)`                           | Input size, hidden size           | Long short-term memory |
| `gru(in, hidden)`                            | Input size, hidden size           | Gated recurrent unit   |
| `attention(dim, heads)`                      | Model dim, number of heads        | Multi-head attention   |
| `embedding(vocab, dim)`                      | Vocabulary size, embedding dim    | Token embedding        |
| `layernorm(dim)`                             | Feature dimension                 | Layer normalization    |
| `batchnorm(features)`                        | Number of features                | Batch normalization    |
| `dropout(rate)`                              | Drop probability                  | Regularization         |
| `flatten()`                                  | —                                 | Flatten to 1D          |
| `residual(block)`                            | Sub-network                       | Skip connection        |
| `pool2d(kind, kernel, stride)`               | max/avg, kernel, stride           | 2D pooling             |

### 5.3 Activation Functions

Built-in activations: `relu`, `sigmoid`, `tanh`, `softmax`, `gelu`, `swish`, `leaky_relu`, `elu`, `silu`, `mish`.

### 5.4 Training Blocks

```mg
train mnist_training {
    model: Classifier,
    data: Dataset::load("mnist"),
    optimizer: Adam { lr: 0.001, betas: (0.9, 0.999) },
    loss: cross_entropy,
    epochs: 100,
    batch_size: 64,

    fn on_epoch(epoch: u32, metrics: &Metrics) {
        println!("Epoch {epoch}: loss={metrics.loss:.4}, acc={metrics.accuracy:.2}%");
    }
}
```

**Grammar:**

```
train_def = 'train' IDENT '{' { train_field }* '}' ;

train_field = 'model' ':' expression ','
            | 'data' ':' expression ','
            | 'optimizer' ':' expression ','
            | 'loss' ':' expression ','
            | 'epochs' ':' expression ','
            | 'batch_size' ':' expression ','
            | 'fn' IDENT '(' param_list ')' block   /* callbacks */
            ;
```

### 5.5 LLM Integration

MechGen provides native types for language model invocation:

```mg
use std::llm::{LLM, Prompt, Response};

pub fn summarize(text: &str) -> String / llm {
    let model = LLM::load("local://llama-3-8b");
    let prompt = Prompt::new("Summarize the following text:\n{text}");
    let response = model.generate(prompt, max_tokens: 256);
    response.text()
}
```

The `/ llm` effect annotation makes LLM usage explicit and handleable.

### 5.6 Autograd

The `grad` keyword computes gradients automatically:

```mg
pub fn train_step(model: &mut Classifier, x: Tensor<f32, [B, 784]>,
                  y: Tensor<i64, [B]>) -> f32 / gpu {
    let logits = model.forward(x);
    let loss = cross_entropy(logits, y);

    // Compute gradients of loss w.r.t. all model parameters
    let grads = grad(loss, model.params());

    // Update parameters
    model.apply_grads(grads, lr: 0.001);
    loss.item()
}
```

**Grammar:**

```
grad_expr = 'grad' '(' expression ',' expression ')' ;
```

The compiler traces the computation graph at compile time through the type system, generating backward passes for all differentiable operations. Non-differentiable operations (comparisons, casts, integer ops) are compile-time errors inside `grad` contexts.

---

## 6. Tensor Algebra

Tensors are first-class types with compile-time shape checking and automatic hardware dispatch.

### 6.1 Tensor Types

```mg
// Statically shaped tensors
let a: Tensor<f32, [3, 224, 224]>;      // 3×224×224 image
let b: Tensor<f64, [1000]>;             // 1000-element vector
let c: Tensor<f16, [B, 512, 512]>;      // batched matrix (B is generic)

// Learnable parameters (tracked for autograd)
let w: Param<f32, [512, 256]>;          // weight matrix
let bias: Param<f32, [256]>;            // bias vector
```

Agent mode:
```mg
v a: Φ[f32; 3, 224, 224]
v w: Π[f32; 512, 256]
```

### 6.2 Tensor Operations

| Operation             | Human                  | Agent    | Description              |
| --------------------- | ---------------------- | -------- | ------------------------ |
| Matrix multiply       | `A @ B`                | `A ⊗ B`  | Shape: [M,K]×[K,N]→[M,N] |
| Element-wise multiply | `A .* B`               | `A ⊙ B`  | Hadamard product         |
| Element-wise add      | `A + B`                | `A + B`  | Broadcast-compatible     |
| Transpose             | `A.T`                  | `A⊤`     | Swap last two dims       |
| Flatten               | `A.flatten()`          | `A⊥`     | Reshape to 1D            |
| Reshape               | `A.reshape([2,3])`     | —        | Arbitrary reshape        |
| Sum                   | `A.sum()`              | —        | Reduce sum               |
| Mean                  | `A.mean(axis: 0)`      | —        | Reduce mean              |
| Gradient              | `grad(loss, w)`        | `∇(l,w)` | Autograd                 |
| Slice                 | `A[0..3, ..]`          | —        | Tensor slicing           |
| Broadcast             | automatic              | —        | Shape broadcasting       |
| Concatenate           | `cat([A, B], axis: 0)` | —        | Join along axis          |
| Stack                 | `stack([A, B])`        | —        | New dimension            |

### 6.3 Shape Checking

The compiler verifies tensor shape compatibility at compile time:

```mg
let a: Tensor<f32, [3, 4]>;
let b: Tensor<f32, [4, 5]>;
let c = a @ b;               // OK: c is Tensor<f32, [3, 5]>

let d: Tensor<f32, [2, 3]>;
let e = a @ d;               // COMPILE ERROR: shape mismatch [3,4] @ [2,3]
```

Shape variables allow generic tensor functions:

```mg
fn linear<const M: usize, const N: usize, const K: usize>(
    x: Tensor<f32, [M, K]>,
    w: Param<f32, [K, N]>,
    b: Param<f32, [N]>,
) -> Tensor<f32, [M, N]> {
    x @ w + b
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

```mg
// Vector literal
let v = tensor![1.0, 2.0, 3.0, 4.0];

// Matrix literal
let m = tensor![
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
];

// Zeros/ones/random constructors
let z = Tensor::<f32, [3, 3]>::zeros();
let o = Tensor::<f32, [256]>::ones();
let r = Tensor::<f32, [64, 784]>::randn();
```

---

## 7. Symbolic Reasoning

MechGen integrates symbolic AI as language-level constructs: knowledge bases with facts and rules, logical inference, and queryable rule engines.

### 7.1 Knowledge Base Definition

```mg
kb TypeRules {
    // Facts — ground truth assertions
    fact numeric("i8");
    fact numeric("i16");
    fact numeric("i32");
    fact numeric("i64");
    fact numeric("f32");
    fact numeric("f64");
    fact unsigned("u8");
    fact unsigned("u16");
    fact unsigned("u32");
    fact unsigned("u64");

    // Rules — logical inference
    rule integer(T) :- numeric(T), !floating(T);
    rule floating(T) :- T == "f32" | T == "f64";
    rule safe_cast(From, To) :- numeric(From), numeric(To), bitwidth(From) <= bitwidth(To);

    // Queries — compile-time or runtime inference
    query can_cast(From, To) :- safe_cast(From, To);
}
```

Agent mode:
```mg
κ TypeRules {
    ⊢ numeric("i32")
    ⊢ numeric("f64")
    ρ integer(T) :- numeric(T), !floating(T)
    ? can_cast(From, To) :- safe_cast(From, To)
}
```

**Grammar:**

```
kb_def = 'kb' IDENT '{' { kb_item }* '}' ;

kb_item = 'fact' IDENT '(' arg_list ')' ';'
        | 'rule' IDENT '(' param_list ')' ':-' rule_body ';'
        | 'query' IDENT '(' param_list ')' ':-' rule_body ';'
        ;

rule_body = rule_term { ',' rule_term }* ;
rule_term = IDENT '(' arg_list ')'
          | '!' rule_term
          | expression ;
```

### 7.2 Runtime Querying

```mg
use std::kb::KnowledgeBase;

pub fn check_types(from: &str, to: &str) -> bool {
    let kb = TypeRules::new();
    kb.query("can_cast", &[from, to]).is_some()
}
```

### 7.3 Integration with the Safety Knowledge Base (SKB)

The SKB from the compiler is itself a `kb` instance with 9,157 rules across:
- Ownership and borrowing (2,100 rules)
- Type safety (1,800 rules)
- Concurrency (1,500 rules)
- FFI safety (1,200 rules)
- Memory layout (1,300 rules)
- API contracts (1,257 rules)

Agents can query the SKB at compile time:

```mg
use std::skb;

pub fn validate_borrow(code: &str) -> Vec<Diagnostic> {
    skb::query()
        .category("borrow")
        .severity("error")
        .check(code)
}
```

---

## 8. Evolutionary Computation

MechGen has first-class support for genetic algorithms, neuroevolution, and evolutionary strategies. The `evolve` block declaratively specifies population, fitness, selection, crossover, and mutation — the compiler generates optimized parallel evolution loops.

### 8.1 Evolve Block

```mg
evolve NeuralArchSearch {
    genome: Vec<LayerGene>,
    population: 200,
    generations: 1000,

    fn fitness(&self) -> f64 / gpu {
        let model = self.genome.build_net();
        model |> train(mnist, epochs: 5) |> evaluate(test_set)
    }

    select tournament(k: 8),
    crossover uniform(rate: 0.7),
    mutate gaussian(sigma: 0.02),
    target fitness > 0.98,

    fn on_generation(gen: u32, best: &Self, stats: &EvolveStats) {
        println!("Gen {gen}: best_fitness={best.fitness():.4}");
    }
}
```

Agent mode:
```mg
Ω NeuralArchSearch {
    Γ: [LayerGene]~,
    η: 200,
    ∞: 1000,

    f φ(&self) -> f64 / gpu {
        v model = self.Γ.build_net()
        model ▸ Θ(mnist, 5) ▸ eval(test_set)
    }

    ⊳ tournament(k: 8),
    χ uniform(r: 0.7),
    μ gaussian(σ: 0.02),
    → φ > 0.98,
}
```

**Grammar:**

```
evolve_def = 'evolve' IDENT '{' { evolve_field }* '}' ;

evolve_field = 'genome' ':' type ','
             | 'population' ':' expression ','
             | 'generations' ':' expression ','
             | fitness_def
             | 'select' selection_strategy ','
             | 'crossover' crossover_strategy ','
             | 'mutate' mutation_strategy ','
             | 'target' 'fitness' comparison expression ','
             | callback_def
             ;

fitness_def = 'fn' 'fitness' '(' '&' 'self' ')' '->' 'f64' [ effect_annotation ] block ;

selection_strategy  = 'tournament' '(' kvp_list ')'
                    | 'roulette' | 'rank' | 'elitist' '(' kvp_list ')' ;
crossover_strategy  = 'uniform' '(' kvp_list ')'
                    | 'single_point' | 'two_point' | 'blend' '(' kvp_list ')' ;
mutation_strategy   = 'gaussian' '(' kvp_list ')'
                    | 'uniform' '(' kvp_list ')' | 'bitflip' '(' kvp_list ')' ;
```

### 8.2 Genome Types

```mg
// A genome is a typed genotype that can be crossed over and mutated.
#[derive(Genome)]
pub struct ArchGenome {
    layers: Vec<LayerGene>,
    learning_rate: f64,
    dropout_rate: f64,
}

#[derive(Gene)]
pub enum LayerGene {
    Dense { units: u32, activation: Activation },
    Conv2d { filters: u32, kernel: u32 },
    Attention { heads: u32, dim: u32 },
    Skip,
}
```

The `#[derive(Genome)]` macro generates `crossover`, `mutate`, and `random` implementations. `#[derive(Gene)]` generates per-variant mutation operators.

### 8.3 Reinforcement Learning

```mg
use std::rl::{Env, Policy, PPO, Trajectory};

pub fn train_agent(env: &mut impl Env) -> Policy<f32, f32> / gpu {
    let mut agent = PPO::new(
        obs_dim: env.observation_space(),
        act_dim: env.action_space(),
        hidden: 256,
        lr: 3e-4,
    );

    for episode in 0..10_000 {
        let trajectory = env.rollout(&agent);
        let metrics = agent.update(&trajectory);

        if episode % 100 == 0 {
            println!("Episode {episode}: reward={metrics.mean_reward:.2}");
        }
    }

    agent.policy()
}
```

### 8.4 Self-Improvement

The combination of evolutionary computation and neural networks enables **recursive self-improvement**: programs that optimize their own architectures, hyperparameters, and strategies:

```mg
// A MechGen program that evolves its own compiler optimization passes.
evolve CompilerOptimizer {
    genome: Vec<OptimizationPass>,
    population: 50,
    generations: 500,

    fn fitness(&self) -> f64 {
        let compiler = Compiler::with_passes(&self.genome);
        let binary = compiler.compile(benchmark_suite);
        let perf = binary.run_benchmarks();
        perf.throughput / perf.binary_size  // multi-objective
    }

    select tournament(k: 4),
    crossover uniform(rate: 0.6),
    mutate gaussian(sigma: 0.05),
    target fitness > baseline * 1.5,
}
```

---

## 9. Agents and Swarms

Agents are autonomous computational entities that combine neural reasoning, symbolic knowledge, and evolutionary adaptation. MechGen's agent system is built on structured effects and capability-based security.

### 9.1 Agent Definition

```mg
agent CodeReviewer {
    brain: LLM,
    kb: KnowledgeBase,
    memory: Vec<Review>,

    fn handle(&mut self, msg: Message<CodeSubmission>) -> Result<Review, AgentError> / agent, llm {
        let rules = self.kb.query("style_rules");
        let analysis = self.brain.analyze(&msg.payload.code, context: &rules);
        let review = Review::from(analysis);
        self.memory.push(review.clone());
        Ok(review)
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::new("llm", CapabilityScope::Instance),
            Capability::new("io", CapabilityScope::Sandboxed),
        ]
    }
}
```

**Grammar:**

```
agent_def = 'agent' IDENT [ ':' type_bound_list ]
            '{' { agent_field | agent_method }* '}' ;

agent_field  = IDENT ':' type ',' ;
agent_method = visibility? function_def ;
```

### 9.2 Swarm Operations

```mg
use std::agent::{Swarm, SwarmConfig, ConsensusStrategy};

pub async fn distributed_review(files: Vec<String>) -> Vec<Review> / agent, io {
    let config = SwarmConfig {
        size: 5,
        consensus: ConsensusStrategy::Majority,
        timeout: Duration::from_secs(30),
    };
    let mut swarm = Swarm::<CodeReviewer>::new(config);

    let reviews: Vec<Review> = swarm.map(files, |agent, file| {
        let code = std::fs::read(&file)?;
        agent.handle(Message::new(CodeSubmission { code }))
    }).await?;

    reviews
}
```

### 9.3 Capability-Based Security

All agent operations are gated by capabilities — fine-grained permissions that can be requested, leased, and revoked:

```mg
use std::agent::{Capability, Region};

pub fn sandboxed_analysis(code: &str) -> Result<Analysis, Error> / agent {
    let cap = Capability::request("analyze")?;
    Region::enter(cap, || {
        // Only analysis operations allowed here.
        // No file I/O, no network, no LLM calls unless explicitly granted.
        parse_and_analyze(code)
    })
}
```

---

## 10. Type System

### 10.1 Overview

MechGen's type system extends Rust's with:

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
\frac{\Gamma \vdash e : \tau \dashv \varepsilon \quad \Gamma, x : \tau \vdash e' : \tau' \dashv \varepsilon'}{\Gamma \vdash \text{let } x = e; \; e' : \tau' \dashv \varepsilon \cup \varepsilon'} \quad \text{[T-Let]}
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
- **Genome type derivation**: crossover/mutate signatures inferred from struct fields

---

## 11. Effect System

### 11.1 Overview

Every function has an effect signature. Effects are algebraic — declared, composed, and handled.

### 11.2 Standard Effects

| Effect       | Operations                   | Description                     |
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

Effects are inferred bottom-up. Explicit annotations are optional documentation.

---

## 12. Contract System

### 12.1 Contract Attributes

```mg
@req(balance >= amount, "sufficient funds")
@ens(result.balance == old.balance - amount, "correct deduction")
@perf(time: O(1))
@fx(pure)
pub fn withdraw(account: &mut Account, amount: u64) -> Receipt {
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
2. **SKB cross-reference** — matching against 9,157 known safety rules
3. **Runtime assertion** — fallback for undecidable predicates

---

## 13. Ownership and Borrowing

MechGen preserves Rust's ownership and borrowing semantics with full inference:

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

MechGen compiles to native code via MLIR and LLVM, with specialized lowering passes:

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

```mg
// Built-in SIMD types
let a: f32x4;     // 128-bit, 4 × f32
let b: f32x8;     // 256-bit, 8 × f32
let c: f64x4;     // 256-bit, 4 × f64
let d: f32x16;    // 512-bit, 16 × f32 (AVX-512)

// SIMD operations
let sum = a + b;
let product = a * b;
let dot = (a * b).sum();
```

---

## Appendix A: Full Grammar in BNF

```bnf
<compilation_unit> ::= <item_list>
<item_list>        ::= <item> <item_list> | ε

<item>     ::= <attribute_list> <visibility> <item_kind>
             | <attribute_list> <item_kind>
<visibility> ::= "pub" | "pub" "(" "crate" ")"

<item_kind> ::= <function_def> | <struct_def> | <enum_def>
              | <trait_def> | <impl_block> | <module_def>
              | <use_decl> | <type_alias> | <const_def>
              | <static_def> | <effect_def> | <spec_def>
              | <net_def> | <kb_def> | <evolve_def> | <agent_def>
              | <train_def>

<function_def> ::= "fn" IDENT <opt_generics> "(" <opt_params> ")"
                   <opt_return> <opt_where> <opt_effects> <block>
<async_fn_def> ::= "async" <function_def>

<struct_def> ::= "struct" IDENT <opt_generics> <opt_where> "{" <field_list> "}"
<enum_def>   ::= "enum" IDENT <opt_generics> <opt_where> "{" <variant_list> "}"
<trait_def>  ::= "trait" IDENT <opt_generics> <opt_supertrait> <opt_where>
                 "{" <trait_items> "}"
<impl_block> ::= "impl" <opt_generics> <type> <opt_for> <opt_where>
                 "{" <impl_items> "}"
<module_def> ::= "mod" IDENT "{" <item_list> "}" | "mod" IDENT ";"
<use_decl>   ::= "use" <use_path> ";"

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
         | "&" <type> | "&" "mut" <type>
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
         | "fn" "(" <type_list_opt> ")" <opt_return>
         | "!" | "_" | "String" | "str"

<shape> ::= "[" <int_list> "]" | "_"
```

---

## Appendix B: Dual Syntax Mapping Table

Every Human-mode construct has a Agent-mode equivalent. Both parse to the same AST.

### B.1 Declaration Keywords

| Human        | Agent | Meaning           |
| ------------ | ----- | ----------------- |
| `fn`         | `f`   | Function          |
| `let`        | `v`   | Immutable binding |
| `let mut`    | `m`   | Mutable binding   |
| `const`      | `c`   | Constant          |
| `struct`     | `S`   | Struct            |
| `enum`       | `E`   | Enum              |
| `trait`      | `T`   | Trait             |
| `impl`       | `I`   | Impl block        |
| `mod`        | `M`   | Module            |
| `use`        | `u`   | Import            |
| `pub`        | `+`   | Public prefix     |
| `async fn`   | `af`  | Async function    |
| `const fn`   | `c f` | Const function    |
| `pub(crate)` | `~`   | Crate-visible     |

### B.2 AI Constructs

| Human           | Agent    | Meaning             |
| --------------- | -------- | ------------------- |
| `net`           | `Ψ`      | Neural network      |
| `layer`         | `λ`      | Network layer       |
| `Tensor<T,S>`   | `Φ[T;S]` | Tensor type         |
| `Param<T,S>`    | `Π[T;S]` | Learnable parameter |
| `train`         | `Θ`      | Training block      |
| `grad`          | `∇`      | Gradient            |
| `agent`         | `α`      | Agent               |
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
| `else if`      | `:?`       | Else-if       |
| `match`        | `? expr {` | Pattern match |
| `for x in y`   | `@ x ~ y`  | For loop      |
| `loop`         | `loop`     | Infinite loop |
| `while`        | `loop ?`   | While loop    |
| `return`       | `ret`      | Return        |
| `true`/`false` | `1b`/`0b`  | Booleans      |

### B.4 Type Syntax

| Human             | Agent     | Meaning             |
| ----------------- | --------- | ------------------- |
| `String`          | `s`       | Owned string        |
| `&str`            | `&s`      | String slice        |
| `Vec<T>`          | `[T]~`    | Growable array      |
| `Option<T>`       | `?T`      | Optional            |
| `Result<T,E>`     | `R[T,E]`  | Result              |
| `Box<T>`          | `^T`      | Heap pointer        |
| `Rc<T>`           | `$T`      | Reference counted   |
| `Arc<T>`          | `@T`      | Atomic ref counted  |
| `HashMap<K,V>`    | `{K:V}`   | Hash map            |
| `HashSet<K>`      | `{K}`     | Hash set            |
| `&mut T`          | `&!T`     | Exclusive reference |
| `fn(T)->U`        | `f(T)->U` | Function pointer    |
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

| Human       | Agent        | Meaning        |
| ----------- | ------------ | -------------- |
| `::`        | `.`          | Path separator |
| `crate::`   | `~.`         | Crate root     |
| `super::`   | `super.`     | Parent module  |
| `self::`    | `self.`      | Current module |
| `Foo { x }` | `Foo @{ x }` | Struct literal |

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

- All numeric types (`i32`, `u64`, `f64`, `f16`, `bf16`, etc.)
- Arithmetic, comparison, logical, bitwise operators
- Semicolons, braces, parentheses
- Comments (`//`, `/* */`, `///`, `//!`)
- Effect annotations (`/ io`, `/ gpu`, `/ llm`)
- Contract attributes (`@req`, `@ens`, `@inv`, `@perf`, `@fx`, `@spec`)
- Range operators (`..`, `..=`)
- Try operator (`?` postfix)
- Closures (`|x| expr`)
- `tensor!` literals

### B.9 Agent Mode Safety Philosophy

In agent mode, safety constructs are **fully handled by the compiler and SKB** (Safety Knowledge Base). The following constructs are unnecessary in agent mode:

| Human Syntax                     | Agent Mode Handling                           | SKB Rules    |
| -------------------------------- | --------------------------------------------- | ------------ |
| `unsafe { ... }`                 | Elided — compiler verifies via OWN/BOR/FFI    | AEL-0001     |
| `unsafe fn`                      | Elided — compiler detects from body analysis  | AEL-0002     |
| Lifetime annotations (`'a`)      | Inferred by compiler's LIF rules              | AEL-0003     |
| `&mut T` explicit annotation     | Inferred — compiler determines mutability      | AEL-0004     |
| `Send` / `Sync` bounds           | Derived automatically from type structure     | AEL-0005     |
| `move` keyword on closures       | Inferred — compiler determines capture mode   | AEL-0006     |
| `Pin<T>` wrapping                | Handled automatically for self-referential types | AEL-0007  |
| `dyn` / `impl` dispatch choice   | Compiler selects based on call-site analysis  | AEL-0008     |
| `PhantomData<T>`                 | Inserted automatically by compiler            | AEL-0012     |

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
| 12   | `-` `!` `&` `&mut` `*` `grad` | Unary prefix     | Right   |
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

A complete lexicon of Agent mode symbols, organized by category. This is the "genetic alphabet" of MechGen — each symbol encodes a high-level concept in minimal space.

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

| Symbol   | Human          | Meaning            |
| -------- | -------------- | ------------------ |
| `f`      | `fn`           | Function           |
| `v`      | `let`          | Variable           |
| `m`      | `let mut`      | Mutable variable   |
| `S`      | `struct`       | Struct             |
| `E`      | `enum`         | Enum               |
| `T`      | `trait`        | Trait              |
| `I`      | `impl`         | Implementation     |
| `M`      | `mod`          | Module             |
| `u`      | `use`          | Import             |
| `+`      | `pub`          | Public             |
| `~`      | `pub(crate)`   | Crate visibility   |
| `?`      | `if`/`match`   | Conditional/match  |
| `:`      | `else`         | Else               |
| `@`      | `for`          | For loop           |
| `ret`    | `return`       | Return             |
| `1b`     | `true`         | Boolean true       |
| `0b`     | `false`        | Boolean false      |
| `s`      | `String`       | String type        |
| `&s`     | `&str`         | String slice       |
| `[T]~`   | `Vec<T>`       | Vector             |
| `?T`     | `Option<T>`    | Optional           |
| `R[T,E]` | `Result<T,E>`  | Result             |
| `^T`     | `Box<T>`       | Heap box           |
| `$T`     | `Rc<T>`        | Ref counted        |
| `@T`     | `Arc<T>`       | Atomic ref counted |
| `{K:V}`  | `HashMap<K,V>` | Hash map           |
| `{K}`    | `HashSet<K>`   | Hash set           |
| `&!T`    | `&mut T`       | Mutable reference  |
| `.`      | `::`           | Path separator     |
| `~.`     | `crate::`      | Crate root         |
| `@d()`   | `#[derive()]`  | Derive             |
| `@t`     | `#[test]`      | Test               |
| `p""   ` | `println!()`   | Print              |
| `f""   ` | `format!()`    | Format             |

### D.5 Control Flow & Effect Compressions (Agent Mode)

| Symbol | Human      | Meaning               |
| ------ | ---------- | --------------------- |
| `@@`   | `loop`     | Infinite loop         |
| `@w`   | `while`    | While loop            |
| `!`    | `break`    | Break from loop       |
| `>>`   | `continue` | Continue loop         |
| `ret`  | `return`   | Return value          |
| `yl`   | `yield`    | Yield from generator  |
| `fx`   | `effect`   | Effect declaration    |
| `hx`   | `handle`   | Effect handler        |
| `sp`   | `spec`     | Spec/contract block   |
| `xn`   | `extern`   | FFI extern block      |
| `.w`   | `.await`   | Async await           |
| `?=`   | `match`    | Pattern match         |
| `:?`   | `else if`  | Else-if chain         |

### D.6 Safety Elision (Agent Mode — Handled by Compiler)

In agent mode, the following constructs have **no syntax** — the compiler's SKB handles them:

| Human Syntax        | Agent Equivalent | Compiler Handling       |
| ------------------- | ---------------- | ----------------------- |
| `unsafe { ... }`    | `{ ... }`        | SKB verifies operations |
| `unsafe fn`         | `f`              | Compiler detects unsafe |
| `'a` lifetimes      | *(omitted)*      | LIF rules infer all     |
| `Send + Sync`       | *(omitted)*      | CON rules derive bounds |
| `Pin<T>`            | *(omitted)*      | Compiler wraps as needed|
| `PhantomData<T>`    | *(omitted)*      | Compiler inserts marker |
| `move \|x\|`       | `\|x\|`          | Capture mode inferred   |

---

## Appendix E: Side-by-Side — Human vs Agent

### E.1 Neural Network (Image Classifier)

**Human:**
```mg
use std::neural::{net, train, Metrics};
use std::tensor::Tensor;

net ImageClassifier {
    layer conv2d(3, 32, 3, 1, 1)
    layer batchnorm(32)
    layer conv2d(32, 64, 3, 1, 1)
    layer flatten()
    layer dense(64 * 7 * 7, 128, relu)
    layer dropout(0.5)
    layer dense(128, 10, softmax)
}

pub fn main() / io, gpu {
    let model = ImageClassifier::new();
    let data = Dataset::load("cifar10");

    train cifar_train {
        model: model,
        data: data,
        optimizer: Adam { lr: 0.001 },
        loss: cross_entropy,
        epochs: 50,
        batch_size: 128,
    }

    let accuracy = model.evaluate(data.test());
    println!("Test accuracy: {accuracy:.2}%");
}
```

**Agent:**
```mg
#![syntax(agent)]
u std.neural.{Ψ, Θ, Metrics}
u std.tensor.Φ

Ψ ImageClassifier {
    λ ⊞(3, 32, 3, 1, 1)
    λ bn(32)
    λ ⊞(32, 64, 3, 1, 1)
    λ ⊥()
    λ δ(64*7*7, 128, relu)
    λ ∅(0.5)
    λ δ(128, 10, σ)
}

+f main() / io, gpu {
    v model = ImageClassifier.new()
    v data = Dataset.load("cifar10")

    Θ cifar_train {
        model: model,
        data: data,
        opt: Adam @{ lr: 0.001 },
        loss: cross_entropy,
        epochs: 50,
        batch: 128,
    }

    v accuracy = model.eval(data.test())
    p"Test accuracy: {accuracy:.2}%"
}
```

### E.2 Evolutionary Neural Architecture Search

**Human:**
```mg
use std::evolve::{Genome, Gene, EvolveStats};
use std::neural::net;

#[derive(Genome)]
pub struct ArchGenome {
    layers: Vec<LayerGene>,
    lr: f64,
    dropout: f64,
}

#[derive(Gene)]
pub enum LayerGene {
    Dense { units: u32, activation: Activation },
    Conv2d { filters: u32, kernel: u32 },
    Attention { heads: u32, dim: u32 },
    Skip,
}

evolve NeuralArchSearch {
    genome: ArchGenome,
    population: 200,
    generations: 500,

    fn fitness(&self) -> f64 / gpu {
        let model = self.genome.build_net();
        let data = Dataset::load("cifar10");
        model |> train_quick(data, epochs: 5) |> evaluate(data.test())
    }

    select tournament(k: 8),
    crossover uniform(rate: 0.7),
    mutate gaussian(sigma: 0.02),
    target fitness > 0.95,

    fn on_generation(gen: u32, best: &Self, stats: &EvolveStats) {
        println!("Gen {gen}: best={best.fitness():.4}, mean={stats.mean:.4}");
    }
}
```

**Agent:**
```mg
#![syntax(agent)]
u std.evolve.{Γ, Gene, EvolveStats}
u std.neural.Ψ

@d(Genome)
+S ArchGenome {
    layers: [LayerGene]~,
    lr: f64,
    dropout: f64,
}

@d(Gene)
+E LayerGene {
    Dense { units: u32, act: Activation },
    Conv2d { filters: u32, kernel: u32 },
    Attention { heads: u32, dim: u32 },
    Skip,
}

Ω NeuralArchSearch {
    Γ: ArchGenome,
    η: 200,
    ∞: 500,

    f φ(&self) -> f64 / gpu {
        v model = self.Γ.build_net()
        v data = Dataset.load("cifar10")
        model ▸ train_quick(data, 5) ▸ eval(data.test())
    }

    ⊳ tournament(k: 8),
    χ uniform(r: 0.7),
    μ gaussian(σ: 0.02),
    → φ > 0.95,
}
```

### E.3 Neurosymbolic Agent (Code Reviewer with KB + LLM)

**Human:**
```mg
use std::agent::{Agent, Swarm, Message, Capability};
use std::llm::{LLM, Prompt};
use std::kb::KnowledgeBase;

kb StyleRules {
    fact max_line_length(120);
    fact max_fn_lines(50);
    fact require_doc_comments(true);
    rule violation(file, line, msg) :- too_long(file, line), max_line_length(max),
                                       line_length(file, line) > max;
}

agent CodeReviewer {
    brain: LLM,
    rules: KnowledgeBase,
    history: Vec<Review>,

    fn handle(&mut self, msg: Message<String>) -> Result<Review, AgentError> / agent, llm {
        let violations = self.rules.query("violation", &[&msg.payload]);
        let analysis = self.brain.generate(
            Prompt::new("Review this code. Known violations: {violations}\n\n{msg.payload}"),
            max_tokens: 512,
        );
        let review = Review { violations, analysis: analysis.text(), score: analysis.score() };
        self.history.push(review.clone());
        Ok(review)
    }
}

pub async fn review_codebase(files: Vec<String>) -> Vec<Review> / agent, llm, io {
    let mut swarm = Swarm::<CodeReviewer>::new(SwarmConfig { size: 4 });
    swarm.map(files, |agent, file| {
        let code = std::fs::read(&file)?;
        agent.handle(Message::new(code))
    }).await
}
```

**Agent:**
```mg
#![syntax(agent)]
u std.agent.{α, Swarm, Message, Capability}
u std.llm.{Λ, Prompt}
u std.kb.Κ

κ StyleRules {
    ⊢ max_line_length(120)
    ⊢ max_fn_lines(50)
    ⊢ require_doc_comments(1b)
    ρ violation(file, line, msg) :- too_long(file, line), max_line_length(max),
                                    line_length(file, line) > max
}

α CodeReviewer {
    brain: Λ,
    rules: Κ,
    history: [Review]~,

    f handle(&!self, msg: Message[s]) -> R[Review, AgentError] / agent, llm {
        v violations = self.rules.query("violation", &[&msg.payload])
        v analysis = self.brain.generate(
            Prompt.new(f"Review this code. Violations: {violations}\n\n{msg.payload}"),
            max_tokens: 512,
        )
        v review = Review @{ violations, analysis: analysis.text(), score: analysis.score() }
        self.history.push(review.clone())
        Ok(review)
    }
}

+af review_codebase(files: [s]~) -> [Review]~ / agent, llm, io {
    m swarm = Swarm[CodeReviewer].new(SwarmConfig @{ size: 4 })
    swarm.map(files, |agent, file| {
        v code = std.fs.read(&file)?
        agent.handle(Message.new(code))
    }).await
}
```

---

*End of Machine Genetic Code (MechGen) Language Specification v1.0.0*
