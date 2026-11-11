# Chapter 2: Lexer & Parser Internals

> **Checked against the code 2026-08-25, and mostly right.** `Token`, `Span`
> and the `Lexer` struct match `prototype/src/lexer.rs` exactly, and the Pratt
> parser is real (`parse_expr_bp`, precedence climbing). Three things were
> not: the `Parser` struct and `parse`'s signature, which named a
> `ParseResult<Ast>` where neither `ParseResult` nor `Ast` exists; and the
> parser's "Error Recovery" section, which described synchronization the
> parser does not do. All three are corrected below and marked where they
> were.


The MAGE frontend converts source text into an AST through two stages:
tokenization (lexer) and parsing. Both are designed for LL(1) operation —
no backtracking, no ambiguity, every decision resolved by looking at the
current token.

---

## 2.1 Lexer Design

The lexer is in `rdx_lexer` (prototype: `prototype/src/lexer.rs`).

### Single-Pass, No Backtracking

The lexer processes source text byte-by-byte in a single forward pass. Every
token is unambiguous from its first character (or first two characters for
two-byte operators).

```rust
struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}
```

### Token Structure

```rust
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub text: String,
}

pub struct Span {
    pub offset: usize,
    pub len: usize,
    pub line: usize,
    pub col: usize,
}
```

Every token carries its exact source location for diagnostics and IDE
features.

### TokenKind Categories

| Category             | Tokens                                                                       | Examples                                      |
| -------------------- | ---------------------------------------------------------------------------- | --------------------------------------------- |
| Declaration keywords | `KwF`, `KwV`, `KwM`, `KwS`, `KwE`, `KwT`, `KwI`, `KwMod`, `KwUse`            | `f`, `v`, `m`, `S`, `E`, `T`, `I`, `M`, `u`   |
| Control flow         | `Question`, `At`, `Colon`, `KwLoop`, `KwBreak`, `KwRet`                      | `?`, `@`, `:`, `loop`, `break`, `ret`         |
| Visibility           | `Plus`                                                                       | `+` (prefix for pub)                          |
| Booleans             | `True`, `False`                                                              | `1b`, `0b`                                    |
| Operators            | `Eq`, `Neq`, `Lt`, `Star`, `And`, `Or`, `Not`, `AndNot`                      | `==`, `!=`, `<`, `*`, `&&`, `\|\|`, `!`, `&!` |
| Type modifiers       | `Tilde`, `Dollar`, `BitXor`, `At`, `Question`                                | `~`, `$`, `^`, `@`, `?`                       |
| Delimiters           | `LParen`/`RParen`, `LBrace`/`RBrace`, `LBrack`/`RBrack`                      | `()`, `{}`, `[]`                              |
| Literals             | `IntLiteral`, `FloatLiteral`, `StringLiteral`, `FormatString`, `PrintString` | `42`, `3.14`, `"hi"`, `f"..."`, `p"..."`      |
| Special              | `Eof`, `Error`, `Whitespace`, `Comment`                                      | end-of-file, invalid char                     |

### Keyword vs Identifier Disambiguation

MAGE keywords are mostly single characters (`f`, `v`, `m`, `S`, `E`, `T`,
`I`, `M`, `u`, `C`). The lexer distinguishes keywords from identifiers using
a lookup table after scanning the full identifier token:

```rust
pub const KEYWORDS: &[(&str, TokenKind)] = &[
    // Agent-mode single-char declaration keywords
    ("f", TokenKind::KwF), ("af", TokenKind::KwAf), ("uf", TokenKind::KwUf),
    ("m", TokenKind::KwM), ("v", TokenKind::KwV), ("C", TokenKind::KwC),
    ("S", TokenKind::KwS), ("E", TokenKind::KwE), ("T", TokenKind::KwT),
    // …
    // Human-mode keywords (Rust-compatible); some map to symbol tokens
    ("fn", TokenKind::KwF), ("pub", TokenKind::Plus), ("let", TokenKind::KwLet),
    ("const", TokenKind::KwC), ("if", TokenKind::Question), ("for", TokenKind::At),
    // …
    ("_", TokenKind::Underscore), ("_T", TokenKind::UnderscoreT),
];

pub fn keyword_kind(text: &str) -> Option<TokenKind> {
    KEYWORDS.iter().find(|(s, _)| *s == text).map(|(_, k)| *k)
}
```

`keyword_kind` is called from the identifier path. A name that is not in the
table is an `Ident`. The table is a list of pairs, not a `match`, so one
token kind can have several spellings: `af` and `async` are both `KwAf`, and
`C` and `const` are both `KwC`, the const declaration. `af` is the async
function keyword, not a separate `KwAsyncFn` token.

### Boolean Literals: `1b` / `0b`

The `b` suffix disambiguates booleans from integers:

- `1b` → `TokenKind::True`
- `0b` → `TokenKind::False`
- `1` → `TokenKind::IntLiteral`
- `0b1010` — binary literal starts with `0b` but has digits past just `b`

The lexer checks: if the token is exactly `1b` or `0b` (no trailing digits),
it's a boolean. Otherwise it's a numeric literal.

### String Sugar

Three string prefixes are recognized at the lexer level:

- `"hello"` → `StringLiteral` (plain string)
- `f"value: {x}"` → `FormatString` (interpolation, like `format!`)
- `p"hello {name}"` → `PrintString` (print to stdout, like `println!`)

The lexer scans the opening prefix and then processes the string body,
tracking `{...}` nesting depth for interpolation expressions.

### Error Recovery

When the lexer encounters an invalid byte:

1. Emit a `TokenKind::Error` token with the offending span
2. Advance past the bad byte
3. Continue lexing

This ensures the parser always gets a complete token stream, even for
malformed input. The error tokens are reported as diagnostics but don't
stop compilation.

### Whitespace Handling

`lex` (`prototype/src/lexer.rs`) drops whitespace and comment tokens before
it returns the token vector, so neither the parser nor any later pass sees
them. The formatter is `mage-parse --fmt-compact`, which calls
`fmt::format_agent` on the parsed AST. It does not read the token stream.

---

## 2.2 Parser Design

The parser is in `rdx_parser` (prototype: `prototype/src/parser.rs`).

### LL(1) Guarantee

MAGE's grammar is deliberately LL(1) — every production can be determined
by looking at exactly one token of lookahead. This is the foundational
design decision that makes MAGE agent-friendly.

**Why LL(1) matters for agents:**

- Deterministic: one token → one parse decision → no ambiguity
- Streaming: parse as tokens arrive, no buffering needed
- Error-resilient: easy to synchronize on delimiters after errors
- Simple mental model: agents reason about syntax without backtracking

### Parser Structure

```rust
// `Parser` is private: `parse` is the only way in. It borrows the token
// slice rather than taking ownership.
struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    /// `block` macros seen so far, by name (resolved at their use site).
    blocks: HashMap<String, BlockDef>,
}

pub fn parse(tokens: &[Token]) -> Result<Module, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_module()
}

pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}
```

### Token Consumption

```rust
impl Parser {
    /// Look at current token without consuming.
    fn peek(&self) -> &Token { ... }

    /// Consume and return current token.
    fn advance(&mut self) -> Token { ... }

    /// Consume if current token matches, else error.
    fn expect(&mut self, expected: TokenKind) -> Result<Token, Diagnostic> { ... }

    /// Consume if current token matches, return bool.
    fn eat(&mut self, kind: TokenKind) -> bool { ... }
}
```

### Parsing Decision Table

The parser's top-level item dispatch is a direct mapping from the current
token to the parse function:

| First Token                  | Parse Function                                 | Produces                             |
| ---------------------------- | ---------------------------------------------- | ------------------------------------ |
| `+`                          | `parse_item_body` consumes it, then `parse_item_kind` | Public item of any kind below |
| `f`                          | `parse_function_def(false, false, …)`          | Function                             |
| `af`                         | `parse_function_def(true, false, …)`           | Async function                       |
| `uf`                         | `parse_function_def(false, true, …)`           | Function, `is_unsafe` set            |
| `S`                          | `parse_struct_def`                             | Struct                               |
| `E`                          | `parse_enum_def`                               | Enum                                 |
| `T`                          | `parse_trait_def`                              | Trait                                |
| `I`                          | `parse_impl_block`                             | Impl block                           |
| `M`                          | `parse_module_def`                             | Module                               |
| `u`                          | `parse_use_decl`                               | Use declaration                      |
| `type`, `Y`                  | `parse_type_alias`                             | Type alias                           |
| `C`, `const`                 | `parse_const_def`                              | Constant                             |
| `Z`, `static`                | `parse_static_def`                             | Static (retired, see parser)         |
| `effect`                     | `parse_effect_def`                             | Effect block                         |
| `spec`                       | `parse_spec_def`                               | Spec                                 |
| `D`, `data`                  | `parse_data_def`                               | Data                                 |
| `@`                          | `parse_attribute`, looped in `parse_item_body` | Attribute, then re-dispatch          |
| `v`, `m`, `val`, `var`       | `parse_let_stmt`, when `is_let_statement`      | Binding statement in a block         |
| `guard`                      | `parse_guard_stmt`                             | Guard statement                      |
| `defer`                      | `parse_defer_stmt`                             | Defer statement                      |
| `?` (expression position)    | `parse_prefix_expr`, inline                    | If expression or match               |
| `@` (expression position)    | `parse_prefix_expr`, inline                    | For loop                             |
| `@@` or `loop`               | `parse_prefix_expr`, inline                    | Infinite loop                        |
| `ret`                        | `parse_prefix_expr`, inline                    | Return expression                    |

Statements are parsed by `parse_block_body_body`. Expressions go from
`parse_expr` to `parse_expr_bp`, which calls `parse_prefix_expr` for the
first operand. `parse_item`, `parse_block` and `parse_expr` are wrappers
around the `_body` functions that run under `within`.

### Expression Parsing: Pratt Parser

Expressions use a Pratt parser (precedence climbing) for correct operator
binding:

```rust
fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
    let mut lhs = self.parse_prefix_expr()?;

    loop {
        // Postfix forms (`?`, `.field`, `[index]`, `as`, `is`, …) // …

        // Infix operators: (operator, left binding power, right binding power)
        let (op, l_bp, r_bp) = match self.peek() {
            TokenKind::Or => ("||", 3, 4),
            TokenKind::And => ("&&", 5, 6),
            TokenKind::Eq => ("==", 7, 8),
            // …
            TokenKind::Plus => ("+", 19, 20),
            TokenKind::Minus => ("-", 19, 20),
            TokenKind::Star => ("*", 21, 22),
            // …
            _ => break,
        };

        if l_bp < min_bp {
            break;
        }
        // …
        self.advance();

        let rhs = self.parse_expr_bp(r_bp)?;
        lhs = Expr::Binary {
            op: op.to_string(),
            left: Box::new(lhs),
            right: Box::new(rhs),
        };
    }

    Ok(lhs)
}
```

The operator is stored as a `String` in `ast::Expr::Binary { op, left, right }`.
Pipelines (`|>`) and assignments (`=`, `+=`, …) are handled in the same loop
by their own arms before this construction. The table of binding powers is
in `parser.rs`, at `parse_expr_bp`, and it runs further than the table below.
The first operand of a prefix operator is parsed by `parse_unary_operand`.

Binding powers (higher = tighter):

| Precedence | Operators                    | Associativity |
| ---------- | ---------------------------- | ------------- |
| 1          | `=` `+=` `-=` `*=` `/=` `%=` | Right         |
| 2          | `\|\|`                       | Left          |
| 3          | `&&`                         | Left          |
| 4          | `==` `!=` `<` `>` `<=` `>=`  | Left          |
| 5          | `\|`                         | Left          |
| 6          | `^`                          | Left          |
| 7          | `&`                          | Left          |
| 8          | `<<` `>>`                    | Left          |
| 9          | `+` `-`                      | Left          |
| 10         | `*` `/` `%`                  | Left          |
| 11         | `!` `-` (unary) `&` `&!` `*` | Prefix        |
| 12         | `.` function call `[index]`  | Postfix       |

### Error Recovery — the parser has none

**Corrected 2026-08-25.** This section described synchronizing on `;`, `}` or a
top-level keyword and resuming, producing partial ASTs for error-tolerant
tooling. **The parser does none of that.** `parse` returns
`Result<Module, ParseError>`: the first unexpected token aborts, and `Parser`
has no diagnostics list to accumulate into. Verified by running a file with two
syntax errors — only the first is reported, and `--check` exits 1 without
reaching name resolution.

The asymmetry with the lexer above is real and worth understanding, because it
is the reason a parse error is the pipeline's one hard stop (Chapter 1 §1.1):

| | On bad input |
|---|---|
| **Lexer** | emits `TokenKind::Error` in place, keeps going, hands the parser a complete stream. `run_check` counts those tokens as errors and continues |
| **Parser** | returns the first `ParseError` and stops. Every later pass takes an `ast::Module`, and there is no partial one to give them |

Recovery would be worth having for exactly the reasons the old text gave — IDE
highlighting, agent generation with holes — and building it means giving
`Parser` a diagnostics list and a resync routine, then deciding what a
half-parsed `Module` means to the eight passes downstream. That is a design
question, not an oversight, and nothing here has answered it.

### Attribute Parsing

Attributes start with `@` and are parsed before the item they decorate:

```
@d(Debug, Clone)       →  derive attribute
@test                  →  test attribute
@bench                 →  bench attribute
@cfg(target_os: "linux")  →  cfg attribute
@i                     →  inline attribute
@i(always)             →  inline(always) attribute
```

The parser collects attributes into a `Vec<Attribute>` attached to the
subsequent item AST node.

---

## 2.3 Testing the Frontend

### Lexer Tests

```rust
#[test]
fn test_lex_function() {
    let tokens = lex("+f greet(name: &s) -> s { f\"hello {name}\" }");
    assert_eq!(tokens[0].kind, TokenKind::Plus);
    assert_eq!(tokens[1].kind, TokenKind::KwF);
    assert_eq!(tokens[2].kind, TokenKind::Ident);
    assert_eq!(tokens[2].text, "greet");
    // ...
}
```

### Parser Tests

```rust
#[test]
fn test_parse_struct() {
    let ast = parse("+S Point { x: f64, y: f64 }");
    match &ast.items[0] {
        Item::Struct(s) => {
            assert!(s.is_pub);
            assert_eq!(s.name, "Point");
            assert_eq!(s.fields.len(), 2);
        }
        _ => panic!("expected struct"),
    }
}
```

### Fuzz Testing

The fuzzer is in the crate, not a separate `cargo fuzz` target.
`prototype/src/fuzz.rs` is a `#[cfg(test)]` module (`fuzz_tests`) with a
deterministic xorshift PRNG, so a failing seed reproduces. Its contract is
that no input, however malformed, may panic the compiler. Each test runs the
full front end (`lexer::lex`, `parser::parse`, `resolve`, `types::check`,
`effects::infer_effects`) over generated input. The tests are ordinary
`#[test]` functions, for example `pipeline_never_panics_on_random_bytes` and
`pipeline_never_panics_on_mutated_valid_programs`. They run with:

```bash
cargo test fuzz
```

This is critical for agent usage — agents may generate malformed source and
the frontend must handle it gracefully.
