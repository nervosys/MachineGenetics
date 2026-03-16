# Redox Language Formal Specification

**Version**: 0.1.0 (Draft)
**Status**: Pre-implementation specification
**Companion**: See `REDOX_PROPOSAL.md` for design rationale and architecture.

---

## Table of Contents

1. [Notation and Conventions](#1-notation-and-conventions)
2. [Lexical Grammar](#2-lexical-grammar)
3. [Syntactic Grammar (EBNF)](#3-syntactic-grammar-ebnf)
4. [Type System](#4-type-system)
5. [Effect System](#5-effect-system)
6. [Contract System](#6-contract-system)
7. [Ownership and Borrowing](#7-ownership-and-borrowing)
8. [Module System](#8-module-system)
9. [Name Resolution](#9-name-resolution)
10. [Appendix: Full Grammar in BNF](#appendix-a-full-grammar-in-bnf)
11. [Appendix: Keyword and Operator Tables](#appendix-b-keyword-and-operator-tables)
12. [Appendix: Precedence Table](#appendix-c-precedence-table)

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

## 2. Lexical Grammar

### 2.1 Source Encoding

```
source_file = BOM? { token | whitespace | comment }* EOF ;
BOM         = '\u{FEFF}' ;
```

### 2.2 Whitespace and Comments

```
whitespace   = ( ' ' | '\t' | '\n' | '\r' )+ ;
comment      = line_comment | block_comment ;
line_comment = '//' { any_char - '\n' }* '\n' ;
block_comment = '/*' { any_char | block_comment }* '*/' ;  /* nestable */
```

### 2.3 Keywords

All keywords are reserved and cannot be used as identifiers.

```
keyword =
    /* Declarations */
    | 'f' | 'F' | 'm' | 'v' | 'c' | 'S' | 'E' | 'T' | 'I' | 'M' | 'U'
    /* Visibility */
    | '+' (as prefix to declaration keyword)
    /* Control flow */
    | '?' | '@' | 'loop' | 'break' | 'continue' | 'ret' | 'yield'
    /* Boolean */
    | '1b' | '0b'
    /* Special */
    | '_' | '_T' | 'as'
    /* Effect */
    | 'effect' | 'handle'
    /* Contract */
    | 'spec'
    /* FFI */
    | 'extern'
    /* Safety (opt-in) */
    | 'unsafe'  /* only in legacy mode */
    ;
```

### 2.4 Identifiers

```
identifier       = XID_START { XID_CONTINUE }* ;
raw_identifier   = 'r#' identifier ;
XID_START        = /* Unicode XID_Start */ | '_' ;
XID_CONTINUE     = /* Unicode XID_Continue */ | '_' ;
```

### 2.5 Literals

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
print_string         = 'p"' { string_char | '{' expression '}' }* '"' ;
raw_string           = 'r"' { any_char - '"' }* '"'
                     | 'r#"' { any_char }* '"#'
                     | 'r##"' { any_char }* '"##' /* etc. */ ;
string_char          = any_char - ( '"' | '\\' ) | escape_sequence ;
escape_sequence      = '\\' ( 'n' | 'r' | 't' | '\\' | '\'' | '"' | '0'
                     | 'x' HEX_DIGIT HEX_DIGIT
                     | 'u{' HEX_DIGIT{1,6} '}' ) ;

/* Character literals */
char_literal = '\'' ( any_char - ( '\'' | '\\' ) | escape_sequence ) '\'' ;

/* Boolean literals */
bool_literal = '1b' | '0b' ;

/* Byte literals */
byte_literal        = "b'" ( BYTE_CHAR | byte_escape ) "'" ;
byte_string_literal = 'b"' { BYTE_CHAR | byte_escape }* '"' ;
byte_escape         = '\\' ( 'n' | 'r' | 't' | '\\' | '\'' | '"' | '0'
                    | 'x' HEX_DIGIT HEX_DIGIT ) ;
```

### 2.6 Operators and Punctuation

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
SCOPE     = '::' ;  /* only in legacy mode; canonical uses '.' */
```

---

## 3. Syntactic Grammar (EBNF)

### 3.1 Compilation Unit

```
compilation_unit = { item }* ;

item = visibility? item_kind ;

visibility = '+' ;  /* pub; absence = private */

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

### 3.2 Function Definitions

```
function_def = 'f' IDENT [ generic_params ] '(' [ param_list ] ')'
               [ '->' type ] [ where_clause ] [ effect_annotation ]
               block ;

/* Method definition (identical but uses 'm') */
method_def = 'm' IDENT [ generic_params ] '(' self_param [ ',' param_list ] ')'
             [ '->' type ] [ where_clause ] [ effect_annotation ]
             block ;

param_list = param { ',' param }* [ ',' ] ;
param      = IDENT ':' type ;

self_param = '&' '_'          /* &self (shared borrow) */
           | '&!' '_'         /* &mut self (exclusive borrow) */
           | '_'              /* self (owned) */
           ;

generic_params = '[' generic_param { ',' generic_param }* [ ',' ] ']' ;
generic_param  = IDENT [ ':' type_bound_list ] [ '=' type ] ;

type_bound_list = type_bound { '+' type_bound }* ;
type_bound      = type_path
                | IDENT             /* abbreviated trait: Cl, Db, PEq, etc. */
                ;

where_clause = '/' where_predicate { ',' where_predicate }* ;
where_predicate = type ':' type_bound_list ;
```

### 3.3 Struct Definitions

```
struct_def = 'S' IDENT [ generic_params ] [ where_clause ]
             '{' { struct_field }* '}' ;

struct_field = visibility? IDENT ':' type [ ',' ] ;

/* Tuple struct */
tuple_struct_def = 'S' IDENT [ generic_params ] '(' { type { ',' type }* } ')' ';' ;

/* Unit struct */
unit_struct_def = 'S' IDENT ';' ;
```

### 3.4 Enum Definitions

```
enum_def = 'E' IDENT [ generic_params ] [ where_clause ]
           '{' enum_variant { ',' enum_variant }* [ ',' ] '}' ;

enum_variant = IDENT [ '(' type_list ')' ]      /* tuple variant */
             | IDENT [ '{' struct_field_list '}' ] /* struct variant */
             | IDENT [ '=' expression ]           /* discriminant variant */
             ;

type_list = type { ',' type }* [ ',' ] ;
struct_field_list = struct_field { ',' struct_field }* [ ',' ] ;
```

### 3.5 Trait Definitions

```
trait_def = 'T' IDENT [ generic_params ] [ ':' type_bound_list ] [ where_clause ]
            '{' { trait_item }* '}' ;

trait_item = trait_method | trait_type | trait_const ;

trait_method = 'm' IDENT [ generic_params ] '(' [ self_param [ ',' param_list ] ] ')'
               [ '->' type ] [ where_clause ] [ block | ';' ] ;

trait_type  = 'type' IDENT [ ':' type_bound_list ] [ '=' type ] ';' ;
trait_const = 'c' IDENT ':' type [ '=' expression ] ';' ;
```

### 3.6 Impl Blocks

```
impl_block = 'I' [ generic_params ] type [ 'for' type ] [ where_clause ]
             '{' { impl_item }* '}' ;

impl_item = visibility? ( method_def | function_def | type_alias | const_def ) ;
```

### 3.7 Module and Use Declarations

```
module_def = 'M' IDENT ( '{' { item }* '}' | ';' ) ;

use_decl = 'u' use_path ';' ;
use_path = path_segment { '.' path_segment }*
           [ '.' ( '*' | '{' use_tree_list '}' ) ] ;
use_tree_list = use_tree { ',' use_tree }* [ ',' ] ;
use_tree = IDENT [ 'as' IDENT ] ;
```

### 3.8 Type Aliases and Constants

```
type_alias  = 'type' IDENT [ generic_params ] '=' type ';' ;
const_def   = 'c' IDENT ':' type '=' expression ';' ;
static_def  = 'static' IDENT ':' type '=' expression ';' ;
```

### 3.9 Types

The type grammar eliminates `<>` angle brackets in favor of `[]` square brackets, uses sigil prefixes for smart pointers, and removes all lifetime annotations.

```
type = type_path
     | reference_type
     | owned_ptr_type
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
     | ptr_type
     | never_type
     | inferred_type
     | string_type
     | simd_type
     ;

type_path      = IDENT { '.' IDENT }* [ '[' type_args ']' ] ;
type_args      = type { ',' type }* [ ',' ] ;

reference_type = '&' type               /* shared reference */
               | '&!' type              /* exclusive (mutable) reference */
               ;

owned_ptr_type = '^' type ;             /* Box<T> equivalent */
rc_type        = '$' type ;             /* Rc<T> equivalent */
arc_type       = '@' type ;             /* Arc<T> equivalent — NOTE: context-dependent, see §3.14 */

slice_type     = '[' type ']' ;         /* &[T] equivalent */
array_type     = '[' type ';' expression ']' ;  /* [T; N] */
vec_type       = '[' type ']~' ;        /* Vec<T> */
tuple_type     = '(' [ type { ',' type }* [ ',' ] ] ')' ;

fn_type        = 'f' '(' [ type_list ] ')' [ '->' type ] ;
option_type    = '?' type ;             /* Option<T> */
result_type    = 'R' '[' type ',' type ']' ;  /* Result<T, E> */
map_type       = '{' type ':' type '}' ;      /* HashMap<K, V> */

ptr_type       = 'Ptr' '[' type ']' ;   /* *const T / *mut T unified */
never_type     = '!' ;
inferred_type  = '_' ;                  /* compiler infers */
string_type    = 's' ;                  /* String or &str depending on context */
simd_type      = 'Simd' '[' type ',' INT_LITERAL ']' ;  /* Simd[f32, 8] */
```

### 3.10 Expressions

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
prefix_expr = ( '-' | '!' | '&' | '&!' | '*' ) expression ;

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
method_call_expr = expression '.' IDENT [ '[' type_args ']' ] '(' [ arg_list ] ')' ;
arg_list         = expression { ',' expression }* [ ',' ] ;

/* Indexing */
index_expr = expression '[' expression ']' ;

/* Field access */
field_expr = expression '.' IDENT ;
           | expression '.' INT_LITERAL ;  /* tuple field */

/* Struct construction — '@' prefix eliminates block ambiguity */
struct_expr = '@' type_path '{' [ field_init_list ] '}' ;
field_init_list = field_init { ',' field_init }* [ ',' ] ;
field_init = IDENT ':' expression
           | IDENT                      /* shorthand: name matches local */
           ;

/* Tuple construction */
tuple_expr = '(' expression ',' [ expression { ',' expression }* [ ',' ] ] ')' ;

/* Array construction */
array_expr = '[' [ expression { ',' expression }* [ ',' ] ] ']'
           | '[' expression ';' expression ']' ;   /* repeat */

/* Closures — deterministic syntax (no |x| ambiguity) */
closure_expr = 'fn' '(' [ param_list ] ')' '=>' expression
             | 'fn' '(' [ param_list ] ')' '=>' block
             ;

/* Control flow */
if_expr    = '?' expression block [ ':' block ]
           | '?' expression block { ':?' expression block }* [ ':' block ] ;
match_expr = '?' '{' match_arm { ',' match_arm }* [ ',' ] '}' ;
match_arm  = pattern '=>' expression ;

loop_expr     = 'loop' block ;
for_expr      = '@' pattern ':' expression block ;

return_expr   = 'ret' [ expression ] ;
break_expr    = 'break' [ expression ] ;
continue_expr = 'continue' ;

/* Range */
range_expr = 'range' '(' expression ',' expression ')'
           | 'range_incl' '(' expression ',' expression ')' ;

/* Explicit cast — no 'as' ambiguity */
cast_expr = '@cast' '(' expression ',' type ')' ;

/* Struct spread */
spread_expr = '@spread' '(' expression ')' ;

/* Async */
await_expr = expression '.await' ;

/* Try operator */
try_expr = expression '?' ;

/* Assignment */
assign_expr = expression '=' expression ;
```

### 3.11 Statements

```
statement = let_statement
          | expression_statement
          | item
          ;

/* Variable bindings */
let_statement = ( 'v' | 'm' ) pattern [ ':' type ] '=' expression ';' ;
/* v = immutable binding (let), m = mutable binding (let mut) */

expression_statement = expression ';' ;
```

### 3.12 Patterns

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

### 3.13 Blocks

```
block = '{' { statement }* [ expression ] '}' ;
```

The last expression in a block (without trailing `;`) is the block's value (tail expression).

### 3.14 Attributes

Redox uses `@` as the attribute prefix. Attributes are always unambiguous because `@` is not used as an infix operator.

```
attribute = '@' attr_name [ '(' attr_args ')' ]
          | '@' attr_name '!' ;         /* e.g., @i! for inline(always) */

attr_name = IDENT { '.' IDENT }* ;      /* e.g., cfg, d, t, pt, as */
attr_args = attr_arg { ',' attr_arg }* ;
attr_arg  = IDENT | IDENT '=' literal | literal | string_literal ;

/* Common abbreviations */
/* @d(Cl,Db)       → derive(Clone, Debug)           */
/* @r(C)           → repr(C)                        */
/* @t              → test                            */
/* @cfg(os=lx)     → cfg(target_os = "linux")       */
/* @pt(gpu)        → perf::target(gpu)              */
/* @pv(8)          → perf::vectorize(width = 8)     */
/* @pa(4)          → perf::autotune(variants = 4)   */
/* @pnb            → perf::no_bounds_check          */
/* @i!             → inline(always)                 */
/* @mu             → must_use                       */
/* @as("...")      → agent::summary("...")          */
/* @ac("...")      → agent::category("...")         */
/* @ffi("c", ...) → FFI binding directive          */
```

### 3.15 Effect Definitions

```
effect_def = 'effect' IDENT '{' { effect_operation }* '}' ;
effect_operation = 'f' IDENT '(' [ param_list ] ')' [ '->' type ] ';' ;

effect_annotation = effect_name { '+' effect_name }* ;
effect_name       = IDENT ;
```

### 3.16 Spec Definitions

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

## 4. Type System

### 4.1 Overview

Redox's type system is based on Rust's type system with the following modifications:

1. **Lifetime annotations are inferred** — no user-visible lifetime parameters (see §13 of the proposal for the algorithm).
2. **Borrow mode is inferred** — `&T` in the source unifies shared and exclusive references; the compiler chooses (see §13.3).
3. **Dispatch strategy is inferred** — no `dyn Trait` vs `impl Trait` distinction in user code (see §13.5).
4. **Allocation strategy is inferred** — bare `T` may be stack, heap, or arena allocated (see §13.6).
5. **Safety bounds (`Send`, `Sync`, etc.) are moved to the SKB** — not part of the type syntax.
6. **Effect types are first-class** — every function has an effect signature.

### 4.2 Type Judgment Rules

The type system uses a judgment of the form:

$$\Gamma; \Sigma; \Delta \vdash e : \tau \dashv \varepsilon$$

where:
- $\Gamma$ is the type environment (variable → type)
- $\Sigma$ is the SKB context (available safety rules)
- $\Delta$ is the effect environment (available effect handlers)
- $e$ is the expression
- $\tau$ is the type
- $\varepsilon$ is the effect set

### 4.3 Core Typing Rules

#### 4.3.1 Variables and Literals

$$
\frac{x : \tau \in \Gamma}{\Gamma; \Sigma; \Delta \vdash x : \tau \dashv \emptyset} \quad \text{[T-Var]}
$$

$$
\frac{n \text{ is an integer literal of type } \tau}{\Gamma; \Sigma; \Delta \vdash n : \tau \dashv \emptyset} \quad \text{[T-IntLit]}
$$

$$
\frac{s \text{ is a string literal}}{\Gamma; \Sigma; \Delta \vdash s : \text{str} \dashv \emptyset} \quad \text{[T-StrLit]}
$$

#### 4.3.2 Function Application

$$
\frac{\Gamma; \Sigma; \Delta \vdash f : (\tau_1, \ldots, \tau_n) \xrightarrow{\varepsilon_f} \tau_r \quad \Gamma; \Sigma; \Delta \vdash e_i : \tau_i \dashv \varepsilon_i}{\Gamma; \Sigma; \Delta \vdash f(e_1, \ldots, e_n) : \tau_r \dashv \varepsilon_f \cup \bigcup_i \varepsilon_i} \quad \text{[T-App]}
$$

#### 4.3.3 Let Binding

$$
\frac{\Gamma; \Sigma; \Delta \vdash e : \tau \dashv \varepsilon \quad \Gamma' = \Gamma, x : \tau \quad \Gamma'; \Sigma; \Delta \vdash e' : \tau' \dashv \varepsilon'}{\Gamma; \Sigma; \Delta \vdash \text{v } x = e; \; e' : \tau' \dashv \varepsilon \cup \varepsilon'} \quad \text{[T-Let]}
$$

#### 4.3.4 Conditional

$$
\frac{\Gamma; \Sigma; \Delta \vdash e_c : \text{bool} \dashv \varepsilon_c \quad \Gamma; \Sigma; \Delta \vdash e_t : \tau \dashv \varepsilon_t \quad \Gamma; \Sigma; \Delta \vdash e_f : \tau \dashv \varepsilon_f}{\Gamma; \Sigma; \Delta \vdash \text{?} \; e_c \; e_t \; \text{:} \; e_f : \tau \dashv \varepsilon_c \cup \varepsilon_t \cup \varepsilon_f} \quad \text{[T-If]}
$$

#### 4.3.5 References (Inferred Mode)

$$
\frac{\Gamma; \Sigma; \Delta \vdash e : \tau \dashv \varepsilon \quad \text{mode} = \text{InferBorrowMode}(e, \text{uses})}{\Gamma; \Sigma; \Delta \vdash \&e : \&_{\text{mode}} \tau \dashv \varepsilon} \quad \text{[T-Ref]}
$$

#### 4.3.6 Struct Construction

$$
\frac{S \text{ defined with fields } f_i : \tau_i \quad \Gamma; \Sigma; \Delta \vdash e_i : \tau_i \dashv \varepsilon_i}{\Gamma; \Sigma; \Delta \vdash @S \{ f_1: e_1, \ldots, f_n: e_n \} : S \dashv \bigcup_i \varepsilon_i} \quad \text{[T-Struct]}
$$

#### 4.3.7 Match Expression

$$
\frac{\Gamma; \Sigma; \Delta \vdash e : \tau \dashv \varepsilon_e \quad \forall i: \text{pat}_i : \tau \Rightarrow \Gamma_i \quad \Gamma, \Gamma_i; \Sigma; \Delta \vdash e_i : \tau_r \dashv \varepsilon_i}{\Gamma; \Sigma; \Delta \vdash \text{? \{} \text{pat}_1 \Rightarrow e_1, \ldots \text{\}} : \tau_r \dashv \varepsilon_e \cup \bigcup_i \varepsilon_i} \quad \text{[T-Match]}
$$

### 4.4 Subtyping

Redox uses **covariant** subtyping for references and **invariant** subtyping for mutable positions, consistent with Rust. The compiler infers variance from field usage (no `PhantomData` required):

$$
\frac{\tau_1 <: \tau_2}{\&\tau_1 <: \&\tau_2} \quad \text{[Sub-RefCovariant]}
$$

$$
\frac{\tau_1 = \tau_2}{\&!\tau_1 <: \&!\tau_2} \quad \text{[Sub-MutRefInvariant]}
$$

### 4.5 Type Inference Algorithm

Redox uses **bidirectional type checking** combined with Hindley-Milner unification:

1. **Check mode**: When the expected type is known (e.g., function arguments), check the expression against it.
2. **Synth mode**: When no expected type is available (e.g., `v x = expr;`), synthesize the type.
3. **Unification**: For generic functions, use constraint-based unification (Robinson's algorithm) with extensions for effect unification.

```
TypeInference:
  1. Generate fresh type variables for unknowns
  2. Walk the AST, generating equality constraints τ₁ ≡ τ₂
  3. Solve via unification (most general unifier)
  4. Apply substitution to resolve all type variables
  5. Report errors for unsolvable constraints
  6. Effect inference runs in parallel (see §5)
```

### 4.6 Generic Type Resolution

Type parameters use `[T]` syntax and are resolved via monomorphization (by default) or dynamic dispatch (when the compiler chooses — see §13.5):

$$
\frac{f : \forall T. (\tau_1[T]) \rightarrow \tau_2[T] \quad \Gamma \vdash e : \tau_1[\sigma]}{\Gamma \vdash f(e) : \tau_2[\sigma]} \quad \text{[T-GenericInst]}
$$

---

## 5. Effect System

### 5.1 Overview

Every function in Redox has an **effect signature** — a set of effects the function may perform. Effects are algebraic: they can be declared, composed, and handled.

### 5.2 Effect Grammar

```
effect_set = '{' effect_name { ',' effect_name }* '}'
           | 'pure'    /* empty effect set */
           ;

effect_name = 'IO' | 'Async' | 'Alloc' | 'Panic' | 'FFI'
            | 'Net' | 'FS' | 'Env' | 'Time'
            | user_defined_effect
            ;
```

### 5.3 Effect Typing Rules

#### 5.3.1 Pure Functions

$$
\frac{\text{body contains no effect.perform ops}}{\Gamma; \Sigma; \Delta \vdash f : \tau_1 \rightarrow \tau_2 \dashv \emptyset} \quad \text{[E-Pure]}
$$

#### 5.3.2 Effect Subsumption

If a function declares effects $\varepsilon_1$ and is called in a context that handles $\varepsilon_2$:

$$
\frac{\varepsilon_1 \subseteq \varepsilon_2}{\text{call is valid}} \quad \text{[E-Subsume]}
$$

#### 5.3.3 Effect Composition

$$
\frac{f : \tau_1 \xrightarrow{\varepsilon_f} \tau_2 \quad g : \tau_2 \xrightarrow{\varepsilon_g} \tau_3}{g \circ f : \tau_1 \xrightarrow{\varepsilon_f \cup \varepsilon_g} \tau_3} \quad \text{[E-Compose]}
$$

#### 5.3.4 Effect Handling

$$
\frac{\Gamma; \Sigma; \Delta, (\text{eff} \mapsto h) \vdash e : \tau \dashv \varepsilon \cup \{\text{eff}\}}{\Gamma; \Sigma; \Delta \vdash \text{handle } e \text{ with } h : \tau \dashv \varepsilon} \quad \text{[E-Handle]}
$$

### 5.4 Effect Inference

Effects are **inferred bottom-up**: leaf functions have their effects determined by which effect operations they call, and callers accumulate the union. Explicit effect annotations are optional and serve as documentation/contracts.

```
InferEffects(fn):
  1. Collect all effect.perform calls in fn body
  2. For each called function g, recursively InferEffects(g)
  3. fn.effects = union of all performed effects and callee effects
  4. If fn has explicit effect annotation, verify inferred ⊆ declared
  5. If violation: emit structured diagnostic
```

### 5.5 Standard Effects

| Effect  | Operations               | Description                        |
| ------- | ------------------------ | ---------------------------------- |
| `IO`    | read, write, seek, close | File and stream I/O                |
| `Net`   | connect, listen, send    | Network I/O                        |
| `FS`    | open, stat, mkdir, rm    | Filesystem operations              |
| `Async` | spawn, join, select      | Asynchronous task management       |
| `Alloc` | alloc, dealloc, realloc  | Heap memory allocation             |
| `Panic` | panic, catch_panic       | Unwinding / structured panics      |
| `FFI`   | call_foreign             | Foreign function invocation        |
| `Env`   | get_var, set_var         | Environment variable access        |
| `Time`  | now, sleep, timeout      | Clock and timer access             |

---

## 6. Contract System

### 6.1 Overview

Contracts specify **verifiable behavioral properties** of functions and types. They are first-class in the language and can be **checked at compile time** (for decidable predicates), **checked at runtime** (as assertions), or **verified against the SKB** (for known patterns).

### 6.2 Contract Grammar

```
contract_attr = requirement | postcondition | invariant | performance_bound ;

requirement       = '@req' '(' expression [ ',' STRING_LITERAL ] ')' ;
postcondition     = '@ens' '(' expression [ ',' STRING_LITERAL ] ')' ;
invariant         = '@inv' '(' expression [ ',' STRING_LITERAL ] ')' ;
performance_bound = '@perf' '(' metric ':' expression ')' ;

/* In postconditions, 'result' refers to the function's return value */
/* In invariants, 'self' refers to the type's current state */
```

### 6.3 Contract Verification Algorithm

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

### 6.4 Contract Inheritance

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

### 6.5 Spec Blocks

Spec blocks group contracts into reusable specifications:

```
spec Sortable[T: Ord] {
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
f sort(items: [T]~) -> [T]~ { ... }
```

---

## 7. Ownership and Borrowing

### 7.1 Overview

Redox preserves Rust's ownership and borrowing semantics but **infers all annotations**. The rules are enforced by the compiler's inference engine (§13 of the proposal) and verified against the SKB.

### 7.2 Ownership Rules (Informal)

1. **Every value has exactly one owner.**
2. **When the owner goes out of scope, the value is dropped.**
3. **Ownership can be transferred (moved) to another binding.**
4. **Values that implement `Copy` are duplicated instead of moved.**
5. **Values can be borrowed (shared `&` or exclusive `&!`), with the compiler deciding which.**

### 7.3 Borrowing Rules (Informal)

1. **At any point, a value may have either:**
   - Any number of shared borrows (**`&T`**), OR
   - Exactly one exclusive borrow (**`&!T`**), but not both.
2. **A borrow must not outlive the referent.**
3. **The compiler infers borrow mode from usage** — if any code path writes through the reference, it is exclusive; otherwise shared.

### 7.4 Formal Ownership Judgments

$$
\frac{x : \tau \in \Gamma \quad x \notin \text{moved}(\Gamma)}{\Gamma \vdash_{\text{own}} x : \text{Valid}} \quad \text{[Own-Valid]}
$$

$$
\frac{\Gamma \vdash_{\text{own}} x : \text{Valid} \quad \Gamma' = \Gamma[\text{moved} \cup \{x\}]}{\Gamma \vdash_{\text{own}} \text{move}(x) : \text{Valid} \dashv \Gamma'} \quad \text{[Own-Move]}
$$

$$
\frac{\Gamma \vdash_{\text{own}} x : \text{Valid} \quad \tau : \text{Copy}}{\Gamma \vdash_{\text{own}} \text{copy}(x) : \text{Valid} \dashv \Gamma} \quad \text{[Own-Copy]}
$$

### 7.5 Borrow Compatibility

The compiler verifies borrow compatibility at each program point:

$$
\frac{\text{borrows}(\Gamma, x) = \{b_1, \ldots, b_n\} \quad \forall i,j: \text{compatible}(b_i, b_j)}{\Gamma \vdash_{\text{borrow}} x : \text{Valid}} \quad \text{[Borrow-Compat]}
$$

where $\text{compatible}(b_i, b_j)$ holds iff both are shared or $i = j$.

---

## 8. Module System

### 8.1 Module Structure

```
// File: src/lib.rdx (crate root)
+M network;      // declares public module, loads from src/network.rdx or src/network/mod.rdx
M internal;       // private module

// File: src/network.rdx
+f connect(addr: s) -> R[Connection, Error] { ... }
f resolve(host: s) -> R[Addr, Error] { ... }  // private
```

### 8.2 Visibility Rules

| Redox    | Rust Equivalent  | Meaning                     |
| -------- | ---------------- | --------------------------- |
| `+f`     | `pub fn`         | Public function             |
| `f`      | `fn`             | Private function            |
| `+S`     | `pub struct`     | Public struct               |
| `+T`     | `pub trait`      | Public trait                |
| `+M`     | `pub mod`        | Public module               |

No `pub(crate)` or `pub(super)` — these are rarely used by agents and add parsing complexity. If needed, they can be added via grammar extension.

### 8.3 Name Resolution

Name resolution follows Rust's rules with simplified path syntax:

```
use std.col.HashMap;     // Rust: use std::collections::HashMap;
use std.io.{Read, Write};
use crate.network.connect;
use super.utils.helper;
```

Resolution order:
1. Local bindings (innermost scope first)
2. Items in the current module
3. Prelude items
4. Explicit imports

---

## 9. Name Resolution

### 9.1 Scoping Rules

Redox uses **lexical scoping** with the following scope hierarchy:

```
Crate scope
  └─ Module scope
       └─ Function scope
            └─ Block scope
                 └─ Pattern binding scope
```

### 9.2 Path Resolution

```
path = [ 'crate' | 'super' | 'self' ] '.' segment { '.' segment }* ;
segment = IDENT [ '[' type_args ']' ] ;
```

The `.` separator replaces Rust's `::` for brevity and consistency with field access. Disambiguation is unnecessary because module paths always start from a known root (`crate`, `super`, `self`, or an import).

### 9.3 Import Shadowing

Later imports shadow earlier imports in the same scope. This is consistent with Rust behavior.

---

## Appendix A: Full Grammar in BNF

For tooling interoperability, here is the complete grammar in pure BNF (no EBNF extensions). Each rule expands to only terminals and non-terminals with `|` for alternation.

```bnf
<compilation_unit> ::= <item_list>
<item_list>        ::= <item> <item_list> | ε

<item>         ::= <attribute_list> <visibility> <item_kind>
                 | <attribute_list> <item_kind>

<visibility>   ::= "+"

<item_kind>    ::= <function_def> | <struct_def> | <enum_def>
                 | <trait_def> | <impl_block> | <module_def>
                 | <use_decl> | <type_alias> | <const_def>
                 | <static_def> | <effect_def> | <spec_def>

<function_def> ::= "f" IDENT <opt_generics> "(" <opt_params> ")"
                   <opt_return> <opt_where> <opt_effects> <block>

<method_def>   ::= "m" IDENT <opt_generics> "(" <self_param> <opt_more_params> ")"
                   <opt_return> <opt_where> <opt_effects> <block>

<opt_generics> ::= "[" <generic_list> "]" | ε
<generic_list> ::= <generic_param> | <generic_param> "," <generic_list>
<generic_param> ::= IDENT | IDENT ":" <bound_list> | IDENT "=" <type>

<opt_params>   ::= <param_list> | ε
<param_list>   ::= <param> | <param> "," <param_list>
<param>        ::= IDENT ":" <type>

<self_param>   ::= "&" "_" | "&!" "_" | "_"
<opt_more_params> ::= "," <param_list> | ε

<opt_return>   ::= "->" <type> | ε
<opt_where>    ::= "/" <where_list> | ε
<opt_effects>  ::= <effect_annotation> | ε

<where_list>     ::= <where_pred> | <where_pred> "," <where_list>
<where_pred>     ::= <type> ":" <bound_list>
<bound_list>     ::= <type_bound> | <type_bound> "+" <bound_list>
<type_bound>     ::= <type_path> | IDENT

<struct_def>   ::= "S" IDENT <opt_generics> <opt_where> "{" <field_list> "}"
<field_list>   ::= <struct_field> <field_list> | ε
<struct_field>  ::= <visibility> IDENT ":" <type> "," | IDENT ":" <type> ","

<enum_def>     ::= "E" IDENT <opt_generics> <opt_where> "{" <variant_list> "}"
<variant_list> ::= <enum_variant> | <enum_variant> "," <variant_list>
<enum_variant> ::= IDENT | IDENT "(" <type_list> ")" | IDENT "{" <field_list> "}"
                 | IDENT "=" <expression>

<trait_def>    ::= "T" IDENT <opt_generics> <opt_supertrait> <opt_where>
                   "{" <trait_items> "}"
<opt_supertrait> ::= ":" <bound_list> | ε
<trait_items>    ::= <trait_item> <trait_items> | ε
<trait_item>     ::= <method_def> | <function_def>
                   | "type" IDENT <opt_bounds> <opt_default_type> ";"
                   | "c" IDENT ":" <type> <opt_default_val> ";"

<impl_block>   ::= "I" <opt_generics> <type> <opt_for> <opt_where>
                   "{" <impl_items> "}"
<opt_for>      ::= "for" <type> | ε
<impl_items>   ::= <impl_item> <impl_items> | ε
<impl_item>    ::= <visibility> <method_def> | <visibility> <function_def>
                 | <method_def> | <function_def>

<module_def>   ::= "M" IDENT "{" <item_list> "}" | "M" IDENT ";"
<use_decl>     ::= "u" <use_path> ";"
<type_alias>   ::= "type" IDENT <opt_generics> "=" <type> ";"
<const_def>    ::= "c" IDENT ":" <type> "=" <expression> ";"
<static_def>   ::= "static" IDENT ":" <type> "=" <expression> ";"

<effect_def>   ::= "effect" IDENT "{" <effect_ops> "}"
<effect_ops>   ::= <effect_op> <effect_ops> | ε
<effect_op>    ::= "f" IDENT "(" <opt_params> ")" <opt_return> ";"

<spec_def>     ::= "spec" IDENT <opt_generics> "{" <spec_items> "}"
<spec_items>   ::= <spec_item> <spec_items> | ε
<spec_item>    ::= "@req" "(" <expression> ")" ";"
                 | "@ens" "(" <expression> ")" ";"
                 | "@perf" "(" <perf_constraint> ")" ";"
                 | "@fx" "(" <effect_list> ")" ";"
                 | "@inv" "(" <expression> ")" ";"

<type>         ::= <type_path> | "&" <type> | "&!" <type>
                 | "^" <type> | "$" <type>
                 | "[" <type> "]" | "[" <type> "]~" | "[" <type> ";" <expression> "]"
                 | "?" <type> | "R" "[" <type> "," <type> "]"
                 | "{" <type> ":" <type> "}"
                 | "Ptr" "[" <type> "]" | "Simd" "[" <type> "," INT "]"
                 | "(" <type_list_opt> ")"
                 | "f" "(" <type_list_opt> ")" <opt_return>
                 | "!" | "_" | "s"

<type_path>    ::= IDENT <opt_type_args> | IDENT "." <type_path>
<opt_type_args> ::= "[" <type_list> "]" | ε
<type_list>    ::= <type> | <type> "," <type_list>
<type_list_opt> ::= <type_list> | ε

<block>        ::= "{" <stmt_list> <opt_tail_expr> "}"
<stmt_list>    ::= <statement> <stmt_list> | ε
<opt_tail_expr> ::= <expression> | ε

<statement>    ::= <let_stmt> | <expression> ";" | <item>
<let_stmt>     ::= "v" <pattern> <opt_type_annot> "=" <expression> ";"
                 | "m" <pattern> <opt_type_annot> "=" <expression> ";"
<opt_type_annot> ::= ":" <type> | ε

<attribute_list>  ::= <attribute> <attribute_list> | ε
<attribute>       ::= "@" IDENT | "@" IDENT "(" <attr_args> ")"
                    | "@" IDENT "!"
<attr_args>       ::= <attr_arg> | <attr_arg> "," <attr_args>
<attr_arg>        ::= IDENT | IDENT "=" <literal> | <literal> | STRING
```

**Note**: Expression grammar is omitted from pure BNF for brevity (it follows standard recursive-descent precedence climbing — see Appendix C).

---

## Appendix B: Keyword and Operator Tables

### B.1 Declaration Keywords

| Keyword | Meaning          | Rust Equivalent  |
| ------- | ---------------- | ---------------- |
| `f`     | Function         | `fn`             |
| `m`     | Mutable binding  | `let mut`        |
| `v`     | Immutable binding| `let`            |
| `c`     | Constant         | `const`          |
| `S`     | Struct           | `struct`         |
| `E`     | Enum             | `enum`           |
| `T`     | Trait            | `trait`          |
| `I`     | Impl block       | `impl`           |
| `M`     | Module           | `mod`            |
| `U`     | Union            | `union`          |
| `u`     | Use import       | `use`            |
| `+`     | Public (prefix)  | `pub`            |

### B.2 Control Flow Keywords

| Keyword    | Meaning             | Rust Equivalent    |
| ---------- | ------------------- | ------------------ |
| `?`        | If / match          | `if` / `match`     |
| `:`        | Else (after block)  | `else`             |
| `:?`       | Else-if             | `else if`          |
| `@`        | For loop            | `for`              |
| `loop`     | Infinite loop       | `loop`             |
| `break`    | Break               | `break`            |
| `continue` | Continue            | `continue`         |
| `ret`      | Return              | `return`           |
| `yield`    | Yield (generators)  | `yield`            |

### B.3 Type Sigils

| Sigil  | Meaning                | Rust Equivalent |
| ------ | ---------------------- | --------------- |
| `&`    | Reference (shared)     | `&T`            |
| `&!`   | Reference (exclusive)  | `&mut T`        |
| `^`    | Heap pointer           | `Box<T>`        |
| `$`    | Reference counted      | `Rc<T>`         |
| `@`    | Atomic ref counted     | `Arc<T>`        |
| `?`    | Optional               | `Option<T>`     |
| `~`    | Growable (suffix)      | `Vec<T>` suffix |
| `!`    | Never type             | `!`             |
| `_`    | Inferred               | `_`             |

### B.4 Attribute Abbreviations

| Abbreviation | Expansion                 |
| ------------ | ------------------------- |
| `@d(...)`    | `derive(...)`             |
| `@r(...)`    | `repr(...)`               |
| `@t`         | `test`                    |
| `@b`         | `bench`                   |
| `@i!`        | `inline(always)`          |
| `@mu`        | `must_use`                |
| `@cfg(...)`  | `cfg(...)`                |
| `@a(...)`    | `allow(...)`              |
| `@x(...)`    | `deny(...)`               |
| `@pt(...)`   | `perf::target(...)`       |
| `@pv(...)`   | `perf::vectorize(...)`    |
| `@pa(...)`   | `perf::autotune(...)`     |
| `@pnb`       | `perf::no_bounds_check`   |
| `@pp`        | `perf::parameter`         |
| `@as(...)`   | `agent::summary(...)`     |
| `@ac(...)`   | `agent::category(...)`    |
| `@ffi(...)`  | FFI binding directive     |
| `@req(...)`  | Contract precondition     |
| `@ens(...)`  | Contract postcondition    |
| `@inv(...)`  | Contract invariant        |
| `@perf(...)` | Performance bound         |
| `@fx(...)`   | Effect constraint         |
| `@spec(...)` | Spec block reference      |

---

## Appendix C: Precedence Table

Operators are listed from highest to lowest precedence. All operators are left-associative unless noted.

| Prec | Operator(s)                 | Description               | Associativity |
| ---- | --------------------------- | ------------------------- | ------------- |
| 15   | `.` field, `[i]` index      | Field access, indexing    | Left          |
| 14   | `f()` call, `.m()` method   | Function/method call      | Left          |
| 13   | `?`                         | Try / unwrap              | Postfix       |
| 12   | `-` `!` `&` `&!` `*`       | Unary prefix              | Right (unary) |
| 11   | `@cast(e, T)`               | Type cast                 | —             |
| 10   | `*` `/` `%`                 | Multiplication            | Left          |
| 9    | `+` `-`                     | Addition                  | Left          |
| 8    | `<<` `>>`                   | Bit shift                 | Left          |
| 7    | `&`                         | Bitwise AND               | Left          |
| 6    | `^`                         | Bitwise XOR               | Left          |
| 5    | `\|`                        | Bitwise OR                | Left          |
| 4    | `==` `!=` `<` `>` `<=` `>=`| Comparison                | Left (no chain)|
| 3    | `&&`                        | Logical AND               | Left          |
| 2    | `\|\|`                      | Logical OR                | Left          |
| 1    | `=` `+=` `-=` `*=` etc.    | Assignment                | Right         |
| 0    | `ret` `break` `yield`       | Control flow              | —             |

**Note**: Comparison operators do not chain. `a < b < c` is a syntax error; use `a < b && b < c`.

---

*End of Redox Language Formal Specification v0.1.0*
