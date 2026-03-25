> **DEPRECATED**: This specification is superseded by [MECHGEN_SPEC.md](MECHGEN_SPEC.md).
> The Redox language has been renamed to **Machine Genetic Code (MechGen)**.
> This file is retained for historical reference only.

# Redox Language Formal Specification

**Version**: 0.2.0 (Draft)
**Status**: Superseded by MECHGEN_SPEC.md v1.0.0
**Companion**: See `REDOX_PROPOSAL.md` for design rationale and architecture.

---

## Table of Contents

1. [Notation and Conventions](#1-notation-and-conventions)
2. [Dual Syntax Modes](#2-dual-syntax-modes)
3. [Lexical Grammar](#3-lexical-grammar)
4. [Syntactic Grammar (EBNF)](#4-syntactic-grammar-ebnf)
5. [Type System](#5-type-system)
6. [Effect System](#6-effect-system)
7. [Contract System](#7-contract-system)
8. [Ownership and Borrowing](#8-ownership-and-borrowing)
9. [Module System](#9-module-system)
10. [Name Resolution](#10-name-resolution)
11. [Appendix: Full Grammar in BNF](#appendix-a-full-grammar-in-bnf)
12. [Appendix: Dual Syntax Mapping Table](#appendix-b-dual-syntax-mapping-table)
13. [Appendix: Precedence Table](#appendix-c-precedence-table)

---

## 1. Notation and Conventions

This specification uses **Extended Backus-Naur Form (EBNF)** with the following meta-syntax:

```
Convention        Meaning
────────────────  ─────────────────────────────────────────
A B               Sequence: A followed by B
A | B             Alternation: A or B
[ A ]             Optional: zero or one A
{ A }             Repetition: zero or more A
( A | B ) C       Grouping
'literal'         Terminal: exact literal string
"literal"         Terminal: exact literal string (alternative quoting)
A+                One or more A  (shorthand for A { A })
A?                Zero or one A  (shorthand for [ A ])
/* comment */     Non-normative comment
UPPER_CASE        Non-terminal defined elsewhere
```

**LL(1) property**: Every production in this grammar is LL(1)-parseable. The lookahead set for each alternative is disjoint. No backtracking is required.

**Unicode**: Redox source files are UTF-8 encoded. The lexer operates on Unicode scalar values (USV). Identifiers follow Unicode UAX #31 with Rust-compatible extensions.

---

## 2. Dual Syntax Modes

Redox supports **two interchangeable surface syntaxes** that parse to the same AST:

| Mode         | Extension | Purpose                              | Example                        |
| ------------ | --------- | ------------------------------------ | ------------------------------ |
| **Human**    | `.rdx`    | Human-readable, C-family style       | `pub fn main() -> i32 { ... }` |
| **Agent**    | `.rdx`    | Machine/agent-optimized, sigil-based | `+f main() -> i32 { ... }`     |

A `#![syntax(agent)]` pragma at the top of a file selects agent mode. **human mode is the default.**

Both modes are byte-for-byte round-trippable via `rdx fmt --human` and `rdx fmt --agent`. The compiler accepts both in the same project — individual files choose their mode.

### 2.1 Design Principle

human mode uses **C-family keywords** wherever a direct C, C++, or Rust analogue exists:

- C keywords: `if`, `else`, `for`, `return`, `struct`, `enum`, `union`, `const`, `true`, `false`
- C++ keywords: `namespace` → `mod`, `template<T>` → `<T>`, `::` path separator
- Rust keywords: `fn`, `let`, `mut`, `pub`, `trait`, `impl`, `match`, `use`, `async`, `yield`
- Redox-unique: `effect`, `handle`, `spec`, `@req`, `@ens`, `@inv`, `@perf`, `@fx`

---

## 3. Lexical Grammar

### 3.1 Source Encoding

```
source_file = BOM? { token | whitespace | comment }* EOF ;
BOM         = '\u{FEFF}' ;
```

### 3.2 Whitespace and Comments

```
whitespace   = ( ' ' | '\t' | '\n' | '\r' )+ ;
comment      = line_comment | block_comment ;
line_comment = '//' { any_char - '\n' }* '\n' ;
block_comment = '/*' { any_char | block_comment }* '*/' ;  /* nestable */
```

### 3.3 Keywords

All keywords are reserved and cannot be used as identifiers.

**human mode keywords:**

```
keyword =
    /* Declarations */
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
    /* Effect */
    | 'effect' | 'handle'
    /* Contract */
    | 'spec'
    /* FFI */
    | 'extern'
    /* Safety (opt-in) */
    | 'unsafe'
    ;
```

**agent mode keywords** (mapped to human equivalents — see Appendix B):

```
compact_keyword =
    | 'f' | 'v' | 'm' | 'c' | 'S' | 'E' | 'T' | 'I' | 'M' | 'U' | 'u'
    | '+' | '?' | '@' | '~' | ':' | ':?'
    | 'ret' | '1b' | '0b'
    | 'loop' | 'break' | 'continue' | 'yield'
    | 'effect' | 'handle' | 'spec' | 'extern' | 'unsafe'
    ;
```

### 3.4 Identifiers

```
identifier       = XID_START { XID_CONTINUE }* ;
raw_identifier   = 'r#' identifier ;
XID_START        = /* Unicode XID_Start */ | '_' ;
XID_CONTINUE     = /* Unicode XID_Continue */ | '_' ;
```

### 3.5 Literals

```
literal = int_literal | float_literal | string_literal | char_literal
        | bool_literal | byte_literal | byte_string_literal ;

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
string_literal       = '"' { string_char }* '"' ;
format_string        = 'f"' { string_char | '{' expression '}' }* '"' ;
print_string         = 'println!' '(' '"' { string_char | '{' expression '}' }* '"' ')' ;
raw_string           = 'r"' { any_char - '"' }* '"'
                     | 'r#"' { any_char }* '"#'
                     | 'r##"' { any_char }* '"##' /* etc. */ ;
string_char          = any_char - ( '"' | '\\' ) | escape_sequence ;
escape_sequence      = '\\' ( 'n' | 'r' | 't' | '\\' | '\'' | '"' | '0'
                     | 'x' HEX_DIGIT HEX_DIGIT
                     | 'u{' HEX_DIGIT{1,6} '}' ) ;

/* Character literals */
char_literal = '\'' ( any_char - ( '\'' | '\\' ) | escape_sequence ) '\'' ;

/* Boolean literals — human mode */
bool_literal = 'true' | 'false' ;

/* Byte literals */
byte_literal        = "b'" ( BYTE_CHAR | byte_escape ) "'" ;
byte_string_literal = 'b"' { BYTE_CHAR | byte_escape }* '"' ;
byte_escape         = '\\' ( 'n' | 'r' | 't' | '\\' | '\'' | '"' | '0'
                    | 'x' HEX_DIGIT HEX_DIGIT ) ;
```

### 3.6 Operators and Punctuation

```
/* Arithmetic */
PLUS    = '+' ;   MINUS  = '-' ;   STAR  = '*' ;   SLASH  = '/' ;
PERCENT = '%' ;

/* Comparison */
EQ = '==' ;  NEQ = '!=' ;  LT = '<' ;  GT = '>' ;  LE = '<=' ;  GE = '>=' ;

/* Logical */
AND = '&&' ;  OR = '||' ;  NOT = '!' ;

/* Bitwise */
BIT_AND = '&' ;  BIT_OR = '|' ;  BIT_XOR = '^' ;  SHL = '<<' ;  SHR = '>>' ;

/* Assignment */
ASSIGN     = '=' ;
PLUS_EQ    = '+=' ;  MINUS_EQ = '-=' ;  STAR_EQ  = '*=' ;
SLASH_EQ   = '/=' ;  PERCENT_EQ = '%=' ;
BIT_AND_EQ = '&=' ;  BIT_OR_EQ  = '|=' ;  BIT_XOR_EQ = '^=' ;
SHL_EQ     = '<<=' ; SHR_EQ     = '>>=' ;

/* Delimiters */
LPAREN = '(' ;   RPAREN = ')' ;
LBRACE = '{' ;   RBRACE = '}' ;
LBRACK = '[' ;   RBRACK = ']' ;

/* Punctuation */
SEMI      = ';' ;   COMMA = ',' ;   DOT   = '.' ;
COLON     = ':' ;   ARROW = '->' ;  FAT_ARROW = '=>' ;
QUESTION  = '?' ;   HASH  = '#' ;   AT    = '@' ;
DOTDOT    = '..' ;  DOTDOTEQ = '..=' ;
SCOPE     = '::' ;  /* human mode path separator */
LT_ANGLE  = '<' ;   GT_ANGLE = '>' ;  /* human mode generic delimiters */
```

---

## 4. Syntactic Grammar (EBNF)

All productions below use **human (C-like) mode**. Agent-mode equivalents are listed in Appendix B.

### 4.1 Compilation Unit

```
compilation_unit = { item }* ;

item = visibility? item_kind ;

visibility = 'pub' ;

item_kind = function_def
          | struct_def
          | enum_def
          | trait_def
          | impl_block
          | module_def
          | use_decl
          | type_alias
          | const_def
          | static_def
          | effect_def
          | spec_def
          | attribute_item
          ;
```

### 4.2 Function Definitions

```
function_def = 'fn' IDENT [ generic_params ] '(' [ param_list ] ')'
               [ '->' type ] [ where_clause ] [ effect_annotation ]
               block ;

/* Async functions */
async_function_def = 'async' 'fn' IDENT [ generic_params ] '(' [ param_list ] ')'
                     [ '->' type ] [ where_clause ] [ effect_annotation ]
                     block ;

param_list = param { ',' param }* [ ',' ] ;
param      = IDENT ':' type ;

self_param = '&' 'self'          /* shared borrow */
           | '&' 'mut' 'self'   /* exclusive borrow */
           | 'self'             /* owned */
           ;

generic_params = '<' generic_param { ',' generic_param }* [ ',' ] '>' ;
generic_param  = IDENT [ ':' type_bound_list ] [ '=' type ] ;

type_bound_list = type_bound { '+' type_bound }* ;
type_bound      = type_path ;

where_clause = 'where' where_predicate { ',' where_predicate }* ;
where_predicate = type ':' type_bound_list ;
```

### 4.3 Struct Definitions

```
struct_def = 'struct' IDENT [ generic_params ] [ where_clause ]
             '{' { struct_field }* '}' ;

struct_field = visibility? IDENT ':' type [ ',' ] ;

/* Tuple struct */
tuple_struct_def = 'struct' IDENT [ generic_params ] '(' { type { ',' type }* } ')' ';' ;

/* Unit struct */
unit_struct_def = 'struct' IDENT ';' ;
```

### 4.4 Enum Definitions

```
enum_def = 'enum' IDENT [ generic_params ] [ where_clause ]
           '{' enum_variant { ',' enum_variant }* [ ',' ] '}' ;

enum_variant = IDENT [ '(' type_list ')' ]      /* tuple variant */
             | IDENT [ '{' struct_field_list '}' ] /* struct variant */
             | IDENT [ '=' expression ]           /* discriminant variant */
             ;

type_list = type { ',' type }* [ ',' ] ;
struct_field_list = struct_field { ',' struct_field }* [ ',' ] ;
```

### 4.5 Trait Definitions

```
trait_def = 'trait' IDENT [ generic_params ] [ ':' type_bound_list ] [ where_clause ]
            '{' { trait_item }* '}' ;

trait_item = trait_method | trait_type | trait_const ;

trait_method = 'fn' IDENT [ generic_params ] '(' [ self_param [ ',' param_list ] ] ')'
               [ '->' type ] [ where_clause ] [ block | ';' ] ;

trait_type  = 'type' IDENT [ ':' type_bound_list ] [ '=' type ] ';' ;
trait_const = 'const' IDENT ':' type [ '=' expression ] ';' ;
```

### 4.6 Impl Blocks

```
impl_block = 'impl' [ generic_params ] type [ 'for' type ] [ where_clause ]
             '{' { impl_item }* '}' ;

impl_item = visibility? ( function_def | type_alias | const_def ) ;
```

### 4.7 Module and Use Declarations

```
module_def = 'mod' IDENT ( '{' { item }* '}' | ';' ) ;

use_decl = 'use' use_path ';' ;
use_path = path_segment { '::' path_segment }*
           [ '::' ( '*' | '{' use_tree_list '}' ) ] ;
use_tree_list = use_tree { ',' use_tree }* [ ',' ] ;
use_tree = IDENT [ 'as' IDENT ] ;
```

### 4.8 Type Aliases and Constants

```
type_alias  = 'type' IDENT [ generic_params ] '=' type ';' ;
const_def   = 'const' IDENT ':' type '=' expression ';' ;
static_def  = 'static' IDENT ':' type '=' expression ';' ;
```

### 4.9 Types

human mode uses `<>` angle brackets for generics and full type names for containers, matching C++/Rust conventions. All lifetime annotations remain inferred.

```
type = type_path
     | reference_type
     | box_type
     | rc_type
     | arc_type
     | slice_type
     | array_type
     | vec_type
     | tuple_type
     | fn_type
     | option_type
     | result_type
     | map_type
     | set_type
     | ptr_type
     | never_type
     | inferred_type
     | string_type
     | simd_type
     ;

type_path      = IDENT { '::' IDENT }* [ '<' type_args '>' ] ;
type_args      = type { ',' type }* [ ',' ] ;

reference_type = '&' type               /* shared reference */
               | '&' 'mut' type         /* exclusive (mutable) reference */
               ;

box_type       = 'Box' '<' type '>' ;       /* heap-allocated */
rc_type        = 'Rc' '<' type '>' ;        /* reference counted */
arc_type       = 'Arc' '<' type '>' ;       /* atomic ref counted */

slice_type     = '&' '[' type ']' ;          /* &[T] slice */
array_type     = '[' type ';' expression ']' ;  /* [T; N] */
vec_type       = 'Vec' '<' type '>' ;        /* growable array */
tuple_type     = '(' [ type { ',' type }* [ ',' ] ] ')' ;

fn_type        = 'fn' '(' [ type_list ] ')' [ '->' type ] ;
option_type    = 'Option' '<' type '>' ;     /* nullable */
result_type    = 'Result' '<' type ',' type '>' ;  /* failable */
map_type       = 'HashMap' '<' type ',' type '>' ; /* hash map */
set_type       = 'HashSet' '<' type '>' ;    /* hash set */

ptr_type       = '*const' type | '*mut' type ;  /* raw pointer */
never_type     = '!' ;
inferred_type  = '_' ;
string_type    = 'String' ;                  /* owned string */
str_type       = 'str' ;                     /* string slice (behind &) */
simd_type      = 'Simd' '<' type ',' INT_LITERAL '>' ;
```

### 4.10 Expressions

```
expression = literal
           | IDENT
           | prefix_expr
           | infix_expr
           | postfix_expr
           | call_expr
           | index_expr
           | field_expr
           | method_call_expr
           | struct_expr
           | tuple_expr
           | array_expr
           | closure_expr
           | if_expr
           | match_expr
           | loop_expr
           | for_expr
           | while_expr
           | block_expr
           | return_expr
           | break_expr
           | continue_expr
           | range_expr
           | cast_expr
           | spread_expr
           | await_expr
           | try_expr
           | assign_expr
           ;

/* Prefix expressions */
prefix_expr = ( '-' | '!' | '&' | '&' 'mut' | '*' ) expression ;

/* Infix expressions — see Appendix C for precedence */
infix_expr = expression binop expression ;
binop = '+' | '-' | '*' | '/' | '%'
      | '==' | '!=' | '<' | '>' | '<=' | '>='
      | '&&' | '||'
      | '&' | '|' | '^' | '<<' | '>>'
      | '+=' | '-=' | '*=' | '/=' | '%='
      | '&=' | '|=' | '^=' | '<<=' | '>>='
      ;

/* Postfix expressions */
postfix_expr = expression '?' ;         /* try/unwrap */

/* Function/method calls */
call_expr        = expression '(' [ arg_list ] ')' ;
method_call_expr = expression '.' IDENT [ '<' type_args '>' ] '(' [ arg_list ] ')' ;
arg_list         = expression { ',' expression }* [ ',' ] ;

/* Indexing */
index_expr = expression '[' expression ']' ;

/* Field access */
field_expr = expression '.' IDENT
           | expression '.' INT_LITERAL ;  /* tuple field */

/* Struct construction */
struct_expr = type_path '{' [ field_init_list ] '}' ;
field_init_list = field_init { ',' field_init }* [ ',' ] ;
field_init = IDENT ':' expression
           | IDENT                      /* shorthand: name matches local */
           ;

/* Tuple construction */
tuple_expr = '(' expression ',' [ expression { ',' expression }* [ ',' ] ] ')' ;

/* Array construction */
array_expr = '[' [ expression { ',' expression }* [ ',' ] ] ']'
           | '[' expression ';' expression ']' ;   /* repeat */

/* Vec construction */
vec_expr = 'vec!' '[' [ expression { ',' expression }* [ ',' ] ] ']'
         | 'Vec' '::' 'new' '(' ')' ;

/* Closures */
closure_expr = '|' [ param_list ] '|' expression
             | '|' [ param_list ] '|' block
             ;

/* Control flow */
if_expr    = 'if' expression block [ 'else' block ]
           | 'if' expression block { 'else' 'if' expression block }* [ 'else' block ] ;
match_expr = 'match' expression '{' match_arm { ',' match_arm }* [ ',' ] '}' ;
match_arm  = pattern '=>' expression ;

loop_expr     = 'loop' block ;
for_expr      = 'for' pattern 'in' expression block ;
while_expr    = 'while' expression block ;

return_expr   = 'return' [ expression ] ;
break_expr    = 'break' [ expression ] ;
continue_expr = 'continue' ;

/* Range */
range_expr = expression '..' expression
           | expression '..=' expression ;

/* Type cast */
cast_expr = expression 'as' type ;

/* Struct spread */
spread_expr = '..' expression ;

/* Async */
await_expr = expression '.' 'await' ;

/* Try operator */
try_expr = expression '?' ;

/* Assignment */
assign_expr = expression '=' expression ;
```

### 4.11 Statements

```
statement = let_statement
          | expression_statement
          | item
          ;

/* Variable bindings */
let_statement = 'let' pattern [ ':' type ] '=' expression ';'
              | 'let' 'mut' pattern [ ':' type ] '=' expression ';' ;

expression_statement = expression ';' ;
```

### 4.12 Patterns

```
pattern = literal_pattern
        | ident_pattern
        | wildcard_pattern
        | tuple_pattern
        | struct_pattern
        | enum_pattern
        | slice_pattern
        | range_pattern
        | or_pattern
        | ref_pattern
        ;

literal_pattern  = literal ;
ident_pattern    = IDENT ;
wildcard_pattern = '_' ;
tuple_pattern    = '(' [ pattern { ',' pattern }* [ ',' ] ] ')' ;
struct_pattern   = type_path '{' [ field_pattern { ',' field_pattern }* [ ',' ] ] '}' ;
field_pattern    = IDENT ':' pattern | IDENT ;
enum_pattern     = type_path '(' [ pattern { ',' pattern }* ] ')' ;
slice_pattern    = '[' [ pattern { ',' pattern }* ] [ '..' ] ']' ;
range_pattern    = literal '..' literal | literal '..=' literal ;
or_pattern       = pattern '|' pattern ;
ref_pattern      = '&' pattern ;
```

### 4.13 Blocks

```
block = '{' { statement }* [ expression ] '}' ;
```

The last expression in a block (without trailing `;`) is the block's value (tail expression).

### 4.14 Attributes

human mode uses `#[...]` for most attributes, matching C++/Rust conventions. Redox-unique attributes retain the `@` prefix for contract, performance, and agent annotations.

```
attribute = '#' '[' attr_name [ '(' attr_args ')' ] ']'
          | '@' attr_name [ '(' attr_args ')' ] ;  /* Redox-specific */

attr_name = IDENT { '::' IDENT }* ;
attr_args = attr_arg { ',' attr_arg }* ;
attr_arg  = IDENT | IDENT '=' literal | literal | string_literal ;

/* Standard attributes — #[...] form */
/* #[derive(Clone, Debug)]     → derive                 */
/* #[repr(C)]                  → repr                   */
/* #[test]                     → test                   */
/* #[bench]                    → bench                  */
/* #[inline(always)]           → inline hint            */
/* #[must_use]                 → must_use               */
/* #[cfg(...)]                 → conditional compilation */
/* #[allow(...)]               → suppress warning       */
/* #[deny(...)]                → error on warning       */

/* Redox-specific attributes — @ form */
/* @pt(gpu)        → perf::target(gpu)              */
/* @pv(8)          → perf::vectorize(width = 8)     */
/* @pa(4)          → perf::autotune(variants = 4)   */
/* @pnb            → perf::no_bounds_check          */
/* @pp             → perf::parameter                */
/* @as("...")      → agent::summary("...")          */
/* @ac("...")      → agent::category("...")         */
/* @ffi("c", ...) → FFI binding directive          */
```

### 4.15 Effect Definitions

```
effect_def = 'effect' IDENT '{' { effect_operation }* '}' ;
effect_operation = 'fn' IDENT '(' [ param_list ] ')' [ '->' type ] ';' ;

effect_annotation = '/' effect_name { '+' effect_name }* ;
effect_name       = IDENT ;
```

Effects are Redox-unique and have no C analogue. The `/` syntax is shared between standard and agent modes.

### 4.16 Spec Definitions

```
spec_def = 'spec' IDENT [ generic_params ] '{' { spec_item }* '}' ;

spec_item = '@req' '(' expression ')' ';'      /* precondition */
          | '@ens' '(' expression ')' ';'       /* postcondition */
          | '@perf' '(' perf_constraint ')' ';' /* performance bound */
          | '@fx' '(' effect_list ')' ';'       /* effect constraint */
          | '@inv' '(' expression ')' ';'       /* invariant  */
          ;

perf_constraint = IDENT ':' expression ;  /* e.g., time: O(n), space: O(1) */
effect_list     = effect_name { ',' effect_name }* ;
```

---

## 5. Type System

### 5.1 Overview

Redox's type system is based on Rust's type system with the following modifications:

1. **Lifetime annotations are inferred** — no user-visible lifetime parameters.
2. **Borrow mode is inferred** — `&T` in the source unifies shared and exclusive references; the compiler chooses.
3. **Dispatch strategy is inferred** — no `dyn Trait` vs `impl Trait` distinction in user code.
4. **Allocation strategy is inferred** — bare `T` may be stack, heap, or arena allocated.
5. **Safety bounds (`Send`, `Sync`, etc.) are moved to the SKB** — not part of the type syntax.
6. **Effect types are first-class** — every function has an effect signature.

### 5.2 Type Judgment Rules

The type system uses a judgment of the form:

$$\Gamma; \Sigma; \Delta \vdash e : \tau \dashv \varepsilon$$

where:
- $\Gamma$ is the type environment (variable → type)
- $\Sigma$ is the SKB context (available safety rules)
- $\Delta$ is the effect environment (available effect handlers)
- $e$ is the expression
- $\tau$ is the type
- $\varepsilon$ is the effect set

### 5.3 Core Typing Rules

#### 5.3.1 Variables and Literals

$$
\frac{x : \tau \in \Gamma}{\Gamma; \Sigma; \Delta \vdash x : \tau \dashv \emptyset} \quad \text{[T-Var]}
$$

$$
\frac{n \text{ is an integer literal of type } \tau}{\Gamma; \Sigma; \Delta \vdash n : \tau \dashv \emptyset} \quad \text{[T-IntLit]}
$$

$$
\frac{s \text{ is a string literal}}{\Gamma; \Sigma; \Delta \vdash s : \text{str} \dashv \emptyset} \quad \text{[T-StrLit]}
$$

#### 5.3.2 Function Application

$$
\frac{\Gamma; \Sigma; \Delta \vdash f : (\tau_1, \ldots, \tau_n) \xrightarrow{\varepsilon_f} \tau_r \quad \Gamma; \Sigma; \Delta \vdash e_i : \tau_i \dashv \varepsilon_i}{\Gamma; \Sigma; \Delta \vdash f(e_1, \ldots, e_n) : \tau_r \dashv \varepsilon_f \cup \bigcup_i \varepsilon_i} \quad \text{[T-App]}
$$

#### 5.3.3 Let Binding

$$
\frac{\Gamma; \Sigma; \Delta \vdash e : \tau \dashv \varepsilon \quad \Gamma' = \Gamma, x : \tau \quad \Gamma'; \Sigma; \Delta \vdash e' : \tau' \dashv \varepsilon'}{\Gamma; \Sigma; \Delta \vdash \text{let } x = e; \; e' : \tau' \dashv \varepsilon \cup \varepsilon'} \quad \text{[T-Let]}
$$

#### 5.3.4 Conditional

$$
\frac{\Gamma; \Sigma; \Delta \vdash e_c : \text{bool} \dashv \varepsilon_c \quad \Gamma; \Sigma; \Delta \vdash e_t : \tau \dashv \varepsilon_t \quad \Gamma; \Sigma; \Delta \vdash e_f : \tau \dashv \varepsilon_f}{\Gamma; \Sigma; \Delta \vdash \text{if} \; e_c \; e_t \; \text{else} \; e_f : \tau \dashv \varepsilon_c \cup \varepsilon_t \cup \varepsilon_f} \quad \text{[T-If]}
$$

#### 5.3.5 References (Inferred Mode)

$$
\frac{\Gamma; \Sigma; \Delta \vdash e : \tau \dashv \varepsilon \quad \text{mode} = \text{InferBorrowMode}(e, \text{uses})}{\Gamma; \Sigma; \Delta \vdash \&e : \&_{\text{mode}} \tau \dashv \varepsilon} \quad \text{[T-Ref]}
$$

#### 5.3.6 Struct Construction

$$
\frac{S \text{ defined with fields } f_i : \tau_i \quad \Gamma; \Sigma; \Delta \vdash e_i : \tau_i \dashv \varepsilon_i}{\Gamma; \Sigma; \Delta \vdash S \{ f_1: e_1, \ldots, f_n: e_n \} : S \dashv \bigcup_i \varepsilon_i} \quad \text{[T-Struct]}
$$

#### 5.3.7 Match Expression

$$
\frac{\Gamma; \Sigma; \Delta \vdash e : \tau \dashv \varepsilon_e \quad \forall i: \text{pat}_i : \tau \Rightarrow \Gamma_i \quad \Gamma, \Gamma_i; \Sigma; \Delta \vdash e_i : \tau_r \dashv \varepsilon_i}{\Gamma; \Sigma; \Delta \vdash \text{match } e \text{ \{} \text{pat}_1 \Rightarrow e_1, \ldots \text{\}} : \tau_r \dashv \varepsilon_e \cup \bigcup_i \varepsilon_i} \quad \text{[T-Match]}
$$

### 5.4 Subtyping

Redox uses **covariant** subtyping for references and **invariant** subtyping for mutable positions, consistent with Rust. The compiler infers variance from field usage (no `PhantomData` required):

$$
\frac{\tau_1 <: \tau_2}{\&\tau_1 <: \&\tau_2} \quad \text{[Sub-RefCovariant]}
$$

$$
\frac{\tau_1 = \tau_2}{\& \text{mut} \; \tau_1 <: \& \text{mut} \; \tau_2} \quad \text{[Sub-MutRefInvariant]}
$$

### 5.5 Type Inference Algorithm

Redox uses **bidirectional type checking** combined with Hindley-Milner unification:

1. **Check mode**: When the expected type is known (e.g., function arguments), check the expression against it.
2. **Synth mode**: When no expected type is available (e.g., `let x = expr;`), synthesize the type.
3. **Unification**: For generic functions, use constraint-based unification (Robinson's algorithm) with extensions for effect unification.

```
TypeInference:
  1. Generate fresh type variables for unknowns
  2. Walk the AST, generating equality constraints τ₁ ≡ τ₂
  3. Solve via unification (most general unifier)
  4. Apply substitution to resolve all type variables
  5. Report errors for unsolvable constraints
  6. Effect inference runs in parallel (see §6)
```

### 5.6 Generic Type Resolution

Type parameters use `<T>` syntax and are resolved via monomorphization (by default) or dynamic dispatch (when the compiler chooses):

$$
\frac{f : \forall T. (\tau_1[T]) \rightarrow \tau_2[T] \quad \Gamma \vdash e : \tau_1[\sigma]}{\Gamma \vdash f(e) : \tau_2[\sigma]} \quad \text{[T-GenericInst]}
$$

---

## 6. Effect System

### 6.1 Overview

Every function in Redox has an **effect signature** — a set of effects the function may perform. Effects are algebraic: they can be declared, composed, and handled.

### 6.2 Effect Grammar

```
effect_set = '{' effect_name { ',' effect_name }* '}'
           | 'pure'    /* empty effect set */
           ;

effect_name = 'IO' | 'Async' | 'Alloc' | 'Panic' | 'FFI'
            | 'Net' | 'FS' | 'Env' | 'Time'
            | user_defined_effect
            ;
```

### 6.3 Effect Typing Rules

#### 6.3.1 Pure Functions

$$
\frac{\text{body contains no effect.perform ops}}{\Gamma; \Sigma; \Delta \vdash f : \tau_1 \rightarrow \tau_2 \dashv \emptyset} \quad \text{[E-Pure]}
$$

#### 6.3.2 Effect Subsumption

If a function declares effects $\varepsilon_1$ and is called in a context that handles $\varepsilon_2$:

$$
\frac{\varepsilon_1 \subseteq \varepsilon_2}{\text{call is valid}} \quad \text{[E-Subsume]}
$$

#### 6.3.3 Effect Composition

$$
\frac{f : \tau_1 \xrightarrow{\varepsilon_f} \tau_2 \quad g : \tau_2 \xrightarrow{\varepsilon_g} \tau_3}{g \circ f : \tau_1 \xrightarrow{\varepsilon_f \cup \varepsilon_g} \tau_3} \quad \text{[E-Compose]}
$$

#### 6.3.4 Effect Handling

$$
\frac{\Gamma; \Sigma; \Delta, (\text{eff} \mapsto h) \vdash e : \tau \dashv \varepsilon \cup \{\text{eff}\}}{\Gamma; \Sigma; \Delta \vdash \text{handle } e \text{ with } h : \tau \dashv \varepsilon} \quad \text{[E-Handle]}
$$

### 6.4 Effect Inference

Effects are **inferred bottom-up**: leaf functions have their effects determined by which effect operations they call, and callers accumulate the union. Explicit effect annotations are optional and serve as documentation/contracts.

```
InferEffects(fn):
  1. Collect all effect.perform calls in fn body
  2. For each called function g, recursively InferEffects(g)
  3. fn.effects = union of all performed effects and callee effects
  4. If fn has explicit effect annotation, verify inferred ⊆ declared
  5. If violation: emit structured diagnostic
```

### 6.5 Standard Effects

| Effect  | Operations               | Description                   |
| ------- | ------------------------ | ----------------------------- |
| `IO`    | read, write, seek, close | File and stream I/O           |
| `Net`   | connect, listen, send    | Network I/O                   |
| `FS`    | open, stat, mkdir, rm    | Filesystem operations         |
| `Async` | spawn, join, select      | Asynchronous task management  |
| `Alloc` | alloc, dealloc, realloc  | Heap memory allocation        |
| `Panic` | panic, catch_panic       | Unwinding / structured panics |
| `FFI`   | call_foreign             | Foreign function invocation   |
| `Env`   | get_var, set_var         | Environment variable access   |
| `Time`  | now, sleep, timeout      | Clock and timer access        |

---

## 7. Contract System

### 7.1 Overview

Contracts specify **verifiable behavioral properties** of functions and types. They are first-class in the language and can be **checked at compile time** (for decidable predicates), **checked at runtime** (as assertions), or **verified against the SKB** (for known patterns).

### 7.2 Contract Grammar

```
contract_attr = requirement | postcondition | invariant | performance_bound ;

requirement       = '@req' '(' expression [ ',' STRING_LITERAL ] ')' ;
postcondition     = '@ens' '(' expression [ ',' STRING_LITERAL ] ')' ;
invariant         = '@inv' '(' expression [ ',' STRING_LITERAL ] ')' ;
performance_bound = '@perf' '(' metric ':' expression ')' ;

/* In postconditions, 'result' refers to the function's return value */
/* In invariants, 'self' refers to the type's current state */
```

### 7.3 Contract Verification Algorithm

```
VerifyContract(fn, contracts):
  Input:
    fn       — function definition (AST + MIR)
    contracts — list of @req, @ens, @inv, @perf annotations

  Phase 1: Static Verification (decidable predicates)
    for each contract c in contracts:
      if c.expression is_decidable:
        result = SMT_solve(c.expression, fn.body)
        if result == Proved:
          mark c as verified
        elif result == Disproved:
          emit error("contract violation", c, counterexample)
        else: /* Unknown */
          mark c as runtime_check

  Phase 2: SKB Cross-Reference
    for each contract c not yet verified:
      matching_rules = SKB.query(c.pattern, fn.context)
      for rule in matching_rules:
        if rule.implies(c):
          mark c as verified_by_skb(rule.id)

  Phase 3: Runtime Instrumentation
    for each contract c still unverified:
      if safety_mode >= "warnings":
        insert runtime assertion for c at appropriate program point
      elif safety_mode == "none":
        skip (contract is documentation only)

  Output:
    VerificationResult {
      proved:        Vec<ContractId>,     // statically verified
      skb_verified:  Vec<(ContractId, RuleId)>,  // verified by SKB
      runtime_check: Vec<ContractId>,     // will be checked at runtime
      violated:      Vec<(ContractId, Counterexample)>,  // proven false
    }
```

### 7.4 Contract Inheritance

Contracts follow the **Liskov Substitution Principle**:

$$
\text{If } S <: T \text{ then:}
$$
$$
\text{Pre}(T) \Rightarrow \text{Pre}(S) \quad \text{(contravariant preconditions)}
$$
$$
\text{Post}(S) \Rightarrow \text{Post}(T) \quad \text{(covariant postconditions)}
$$
$$
\text{Inv}(S) \Rightarrow \text{Inv}(T) \quad \text{(covariant invariants)}
$$

In practice: a subtype may **weaken** preconditions (accept more inputs) and **strengthen** postconditions (guarantee more outputs).

### 7.5 Spec Blocks

Spec blocks group contracts into reusable specifications:

```redox
spec Sortable<T: Ord> {
    @req(items.len() > 0, "non-empty input");
    @ens(result.is_sorted(), "output is sorted");
    @ens(result.len() == items.len(), "preserves length");
    @ens(result.is_permutation_of(items), "preserves elements");
    @perf(time: O(n * log(n)));
    @perf(space: O(1));  // in-place
    @fx(pure);
}

// A function satisfies a spec:
@spec(Sortable)
fn sort(items: Vec<T>) -> Vec<T> { ... }
```

---

## 8. Ownership and Borrowing

### 8.1 Overview

Redox preserves Rust's ownership and borrowing semantics but **infers all annotations**. The rules are enforced by the compiler's inference engine and verified against the SKB.

### 8.2 Ownership Rules (Informal)

1. **Every value has exactly one owner.**
2. **When the owner goes out of scope, the value is dropped.**
3. **Ownership can be transferred (moved) to another binding.**
4. **Values that implement `Copy` are duplicated instead of moved.**
5. **Values can be borrowed (shared `&` or exclusive `&mut`), with the compiler deciding which.**

### 8.3 Borrowing Rules (Informal)

1. **At any point, a value may have either:**
   - Any number of shared borrows (`&T`), OR
   - Exactly one exclusive borrow (`&mut T`), but not both.
2. **A borrow must not outlive the referent.**
3. **The compiler infers borrow mode from usage** — if any code path writes through the reference, it is exclusive; otherwise shared.

### 8.4 Formal Ownership Judgments

$$
\frac{x : \tau \in \Gamma \quad x \notin \text{moved}(\Gamma)}{\Gamma \vdash_{\text{own}} x : \text{Valid}} \quad \text{[Own-Valid]}
$$

$$
\frac{\Gamma \vdash_{\text{own}} x : \text{Valid} \quad \Gamma' = \Gamma[\text{moved} \cup \{x\}]}{\Gamma \vdash_{\text{own}} \text{move}(x) : \text{Valid} \dashv \Gamma'} \quad \text{[Own-Move]}
$$

$$
\frac{\Gamma \vdash_{\text{own}} x : \text{Valid} \quad \tau : \text{Copy}}{\Gamma \vdash_{\text{own}} \text{copy}(x) : \text{Valid} \dashv \Gamma} \quad \text{[Own-Copy]}
$$

### 8.5 Borrow Compatibility

The compiler verifies borrow compatibility at each program point:

$$
\frac{\text{borrows}(\Gamma, x) = \{b_1, \ldots, b_n\} \quad \forall i,j: \text{compatible}(b_i, b_j)}{\Gamma \vdash_{\text{borrow}} x : \text{Valid}} \quad \text{[Borrow-Compat]}
$$

where $\text{compatible}(b_i, b_j)$ holds iff both are shared or $i = j$.

---

## 9. Module System

### 9.1 Module Structure

```redox
// File: src/lib.rdx (crate root)
pub mod network;      // declares public module, loads from src/network.rdx or src/network/mod.rdx
mod internal;          // private module

// File: src/network.rdx
pub fn connect(addr: String) -> Result<Connection, Error> { ... }
fn resolve(host: String) -> Result<Addr, Error> { ... }  // private
```

### 9.2 Visibility Rules

| Redox        | Rust Equivalent | Meaning          |
| ------------ | --------------- | ---------------- |
| `pub fn`     | `pub fn`        | Public function  |
| `fn`         | `fn`            | Private function |
| `pub struct` | `pub struct`    | Public struct    |
| `pub trait`  | `pub trait`     | Public trait     |
| `pub mod`    | `pub mod`       | Public module    |

No `pub(crate)` or `pub(super)` — these are rarely used by agents and add parsing complexity. If needed, they can be added via grammar extension.

### 9.3 Name Resolution

Name resolution follows Rust's rules with standard `::` path syntax:

```redox
use std::collections::HashMap;
use std::io::{Read, Write};
use crate::network::connect;
use super::utils::helper;
```

Resolution order:
1. Local bindings (innermost scope first)
2. Items in the current module
3. Prelude items
4. Explicit imports

---

## 10. Name Resolution

### 10.1 Scoping Rules

Redox uses **lexical scoping** with the following scope hierarchy:

```
Crate scope
  └─ Module scope
       └─ Function scope
            └─ Block scope
                 └─ Pattern binding scope
```

### 10.2 Path Resolution

```
path = [ 'crate' | 'super' | 'self' ] '::' segment { '::' segment }* ;
segment = IDENT [ '<' type_args '>' ] ;
```

The `::` separator matches C++ and Rust conventions. Module paths always start from a known root (`crate`, `super`, `self`, or an import).

### 10.3 Import Shadowing

Later imports shadow earlier imports in the same scope. This is consistent with Rust behavior.

---

## Appendix A: Full Grammar in BNF

For tooling interoperability, here is the complete grammar in pure BNF (no EBNF extensions). Each rule expands to only terminals and non-terminals with `|` for alternation.

```bnf
<compilation_unit> ::= <item_list>
<item_list>        ::= <item> <item_list> | ε

<item>         ::= <attribute_list> <visibility> <item_kind>
                 | <attribute_list> <item_kind>

<visibility>   ::= "pub"

<item_kind>    ::= <function_def> | <struct_def> | <enum_def>
                 | <trait_def> | <impl_block> | <module_def>
                 | <use_decl> | <type_alias> | <const_def>
                 | <static_def> | <effect_def> | <spec_def>

<function_def> ::= "fn" IDENT <opt_generics> "(" <opt_params> ")"
                   <opt_return> <opt_where> <opt_effects> <block>

<opt_generics> ::= "<" <generic_list> ">" | ε
<generic_list> ::= <generic_param> | <generic_param> "," <generic_list>
<generic_param> ::= IDENT | IDENT ":" <bound_list> | IDENT "=" <type>

<opt_params>   ::= <param_list> | ε
<param_list>   ::= <param> | <param> "," <param_list>
<param>        ::= IDENT ":" <type>

<self_param>   ::= "&" "self" | "&" "mut" "self" | "self"
<opt_more_params> ::= "," <param_list> | ε

<opt_return>   ::= "->" <type> | ε
<opt_where>    ::= "where" <where_list> | ε
<opt_effects>  ::= <effect_annotation> | ε

<where_list>     ::= <where_pred> | <where_pred> "," <where_list>
<where_pred>     ::= <type> ":" <bound_list>
<bound_list>     ::= <type_bound> | <type_bound> "+" <bound_list>
<type_bound>     ::= <type_path> | IDENT

<struct_def>   ::= "struct" IDENT <opt_generics> <opt_where> "{" <field_list> "}"
<field_list>   ::= <struct_field> <field_list> | ε
<struct_field>  ::= <visibility> IDENT ":" <type> "," | IDENT ":" <type> ","

<enum_def>     ::= "enum" IDENT <opt_generics> <opt_where> "{" <variant_list> "}"
<variant_list> ::= <enum_variant> | <enum_variant> "," <variant_list>
<enum_variant> ::= IDENT | IDENT "(" <type_list> ")" | IDENT "{" <field_list> "}"
                 | IDENT "=" <expression>

<trait_def>    ::= "trait" IDENT <opt_generics> <opt_supertrait> <opt_where>
                   "{" <trait_items> "}"
<opt_supertrait> ::= ":" <bound_list> | ε
<trait_items>    ::= <trait_item> <trait_items> | ε
<trait_item>     ::= <function_def>
                   | "type" IDENT <opt_bounds> <opt_default_type> ";"
                   | "const" IDENT ":" <type> <opt_default_val> ";"

<impl_block>   ::= "impl" <opt_generics> <type> <opt_for> <opt_where>
                   "{" <impl_items> "}"
<opt_for>      ::= "for" <type> | ε
<impl_items>   ::= <impl_item> <impl_items> | ε
<impl_item>    ::= <visibility> <function_def> | <function_def>

<module_def>   ::= "mod" IDENT "{" <item_list> "}" | "mod" IDENT ";"
<use_decl>     ::= "use" <use_path> ";"
<type_alias>   ::= "type" IDENT <opt_generics> "=" <type> ";"
<const_def>    ::= "const" IDENT ":" <type> "=" <expression> ";"
<static_def>   ::= "static" IDENT ":" <type> "=" <expression> ";"

<effect_def>   ::= "effect" IDENT "{" <effect_ops> "}"
<effect_ops>   ::= <effect_op> <effect_ops> | ε
<effect_op>    ::= "fn" IDENT "(" <opt_params> ")" <opt_return> ";"

<spec_def>     ::= "spec" IDENT <opt_generics> "{" <spec_items> "}"
<spec_items>   ::= <spec_item> <spec_items> | ε
<spec_item>    ::= "@req" "(" <expression> ")" ";"
                 | "@ens" "(" <expression> ")" ";"
                 | "@perf" "(" <perf_constraint> ")" ";"
                 | "@fx" "(" <effect_list> ")" ";"
                 | "@inv" "(" <expression> ")" ";"

<type>         ::= <type_path> | "&" <type> | "&" "mut" <type>
                 | "Box" "<" <type> ">" | "Rc" "<" <type> ">" | "Arc" "<" <type> ">"
                 | "&" "[" <type> "]" | "[" <type> "]" | "[" <type> ";" <expression> "]"
                 | "Vec" "<" <type> ">"
                 | "Option" "<" <type> ">" | "Result" "<" <type> "," <type> ">"
                 | "HashMap" "<" <type> "," <type> ">" | "HashSet" "<" <type> ">"
                 | "*const" <type> | "*mut" <type>
                 | "Simd" "<" <type> "," INT ">"
                 | "(" <type_list_opt> ")"
                 | "fn" "(" <type_list_opt> ")" <opt_return>
                 | "!" | "_" | "String" | "str"

<type_path>    ::= IDENT <opt_type_args> | IDENT "::" <type_path>
<opt_type_args> ::= "<" <type_list> ">" | ε
<type_list>    ::= <type> | <type> "," <type_list>
<type_list_opt> ::= <type_list> | ε

<block>        ::= "{" <stmt_list> <opt_tail_expr> "}"
<stmt_list>    ::= <statement> <stmt_list> | ε
<opt_tail_expr> ::= <expression> | ε

<statement>    ::= <let_stmt> | <expression> ";" | <item>
<let_stmt>     ::= "let" <pattern> <opt_type_annot> "=" <expression> ";"
                 | "let" "mut" <pattern> <opt_type_annot> "=" <expression> ";"
<opt_type_annot> ::= ":" <type> | ε

<attribute_list>  ::= <attribute> <attribute_list> | ε
<attribute>       ::= "#" "[" IDENT "]" | "#" "[" IDENT "(" <attr_args> ")" "]"
                    | "@" IDENT | "@" IDENT "(" <attr_args> ")"
<attr_args>       ::= <attr_arg> | <attr_arg> "," <attr_args>
<attr_arg>        ::= IDENT | IDENT "=" <literal> | <literal> | STRING
```

**Note**: Expression grammar is omitted from pure BNF for brevity (it follows standard recursive-descent precedence climbing — see Appendix C).

---

## Appendix B: Dual Syntax Mapping Table

Every human-mode construct has an agent-mode equivalent. The compiler desugars both to the same AST.

### B.1 Declaration Keywords

| Human   | Agent | Meaning           |
| ---------- | ------- | ----------------- |
| `fn`       | `f`     | Function          |
| `let`      | `v`     | Immutable binding |
| `let mut`  | `m`     | Mutable binding   |
| `const`    | `c`     | Constant          |
| `struct`   | `S`     | Struct            |
| `enum`     | `E`     | Enum              |
| `trait`    | `T`     | Trait             |
| `impl`     | `I`     | Impl block        |
| `mod`      | `M`     | Module            |
| `union`    | `U`     | Union             |
| `use`      | `u`     | Use import        |
| `pub`      | `+`     | Public (prefix)   |
| `async fn` | `af`    | Async function    |
| `const fn` | `c f`   | Const function    |

### B.2 Control Flow Keywords

| Human     | Agent    | Meaning                    |
| ------------ | ---------- | -------------------------- |
| `if`         | `?`        | Conditional                |
| `else`       | `:`        | Else branch                |
| `else if`    | `:?`       | Else-if                    |
| `match`      | `? expr {` | Pattern match              |
| `for x in y` | `@ x ~ y`  | For loop                   |
| `loop`       | `loop`     | Infinite loop              |
| `while`      | —          | While loop (standard only) |
| `break`      | `break`    | Break                      |
| `continue`   | `continue` | Continue                   |
| `return`     | `ret`      | Return                     |
| `yield`      | `yield`    | Yield                      |
| `true`       | `1b`       | Boolean true               |
| `false`      | `0b`       | Boolean false              |

### B.3 Type Syntax

| Human              | Agent      | Meaning             |
| --------------------- | ------------ | ------------------- |
| `&T`                  | `&T`         | Shared reference    |
| `&mut T`              | `&!T`        | Exclusive reference |
| `Box<T>`              | `^T`         | Heap pointer        |
| `Rc<T>`               | `$T`         | Reference counted   |
| `Arc<T>`              | `@T`         | Atomic ref counted  |
| `Vec<T>`              | `[T]~`       | Growable array      |
| `Option<T>`           | `?T`         | Optional            |
| `Result<T, E>`        | `R[T, E]`    | Result              |
| `HashMap<K, V>`       | `{K: V}`     | Hash map            |
| `HashSet<K>`          | `{K}`        | Hash set            |
| `String`              | `s`          | Owned string        |
| `&str`                | `&s`         | String slice        |
| `fn(T) -> U`          | `f(T) -> U`  | Function pointer    |
| `*const T` / `*mut T` | `Ptr[T]`     | Raw pointer         |
| `Simd<T, N>`          | `Simd[T, N]` | SIMD type           |
| `T<A>` (generics)     | `T[A]`       | Generic parameters  |
| `!`                   | `!`          | Never type          |
| `_`                   | `_`          | Inferred type       |

### B.4 Path and Scope

| Human  | Agent  | Meaning        |
| --------- | -------- | -------------- |
| `::`      | `.`      | Path separator |
| `crate::` | `~.`     | Crate root     |
| `super::` | `super.` | Parent module  |
| `self::`  | `self.`  | Current module |

### B.5 Syntax Constructs

| Human              | Agent                | Meaning        |
| --------------------- | ---------------------- | -------------- |
| `\|x\| expr`          | `fn(x) => expr`        | Closure        |
| `Type { field: val }` | `Type @{ field: val }` | Struct literal |
| `e as T`              | `@cast(e, T)`          | Type cast      |
| `..e`                 | `@spread(e)`           | Struct spread  |
| `where T: Clone`      | `/ T: Clone`           | Where clause   |
| `&self`               | `&_`                   | Shared self    |
| `&mut self`           | `&!_`                  | Mutable self   |
| `self`                | `_`                    | Owned self     |

### B.6 Attributes

| Human            | Agent     | Meaning       |
| ------------------- | ----------- | ------------- |
| `#[derive(...)]`    | `@d(...)`   | Derive        |
| `#[repr(...)]`      | `@r(...)`   | Repr          |
| `#[test]`           | `@t`        | Test          |
| `#[bench]`          | `@b`        | Bench         |
| `#[inline(always)]` | `@i!`       | Inline always |
| `#[must_use]`       | `@mu`       | Must use      |
| `#[cfg(...)]`       | `@cfg(...)` | Cfg           |
| `#[allow(...)]`     | `@a(...)`   | Allow         |
| `#[deny(...)]`      | `@x(...)`   | Deny          |

### B.7 Output Macros

| Human           | Agent   | Meaning       |
| ------------------ | --------- | ------------- |
| `println!("...")`  | `p"..."`  | Print line    |
| `format!("...")`   | `f"..."`  | Format string |
| `eprintln!("...")` | `ep"..."` | Error print   |

### B.8 Shared Syntax (Identical in Both Modes)

The following are identical in standard and agent modes:
- All numeric types (`i32`, `u64`, `f64`, etc.)
- All arithmetic, comparison, logical, and bitwise operators
- Semicolons, braces, parentheses
- Comments (`//`, `/* */`, `///`, `//!`)
- `loop`, `break`, `continue`, `yield`
- Effect annotations (`/ io`, `/ net`, `/ pure`)
- Contract attributes (`@req`, `@ens`, `@inv`, `@perf`, `@fx`, `@spec`)
- Performance annotations (`@pt`, `@pv`, `@pa`, `@pnb`, `@pp`)
- Agent annotations (`@as`, `@ac`)
- `effect`, `handle`, `spec` keywords
- `unsafe`, `extern` keywords
- Range operators (`..`, `..=`)
- Try operator (`?` postfix)
- Await (`.await`)

---

## Appendix C: Precedence Table

Operators are listed from highest to lowest precedence. All operators are left-associative unless noted.

| Prec | Operator(s)                 | Description            | Associativity   |
| ---- | --------------------------- | ---------------------- | --------------- |
| 15   | `.` field, `[i]` index      | Field access, indexing | Left            |
| 14   | `f()` call, `.m()` method   | Function/method call   | Left            |
| 13   | `?`                         | Try / unwrap           | Postfix         |
| 12   | `-` `!` `&` `&mut` `*`      | Unary prefix           | Right (unary)   |
| 11   | `as`                        | Type cast              | Left            |
| 10   | `*` `/` `%`                 | Multiplication         | Left            |
| 9    | `+` `-`                     | Addition               | Left            |
| 8    | `<<` `>>`                   | Bit shift              | Left            |
| 7    | `&`                         | Bitwise AND            | Left            |
| 6    | `^`                         | Bitwise XOR            | Left            |
| 5    | `\|`                        | Bitwise OR             | Left            |
| 4    | `==` `!=` `<` `>` `<=` `>=` | Comparison             | Left (no chain) |
| 3    | `&&`                        | Logical AND            | Left            |
| 2    | `\|\|`                      | Logical OR             | Left            |
| 1    | `=` `+=` `-=` `*=` etc.     | Assignment             | Right           |
| 0    | `return` `break` `yield`    | Control flow           | —               |

**Note**: Comparison operators do not chain. `a < b < c` is a syntax error; use `a < b && b < c`.

---

*End of Redox Language Formal Specification v0.2.0*
