# Chapter 2: Lexer & Parser Internals

The Redox frontend converts source text into an AST through two stages:
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

Redox keywords are mostly single characters (`f`, `v`, `m`, `S`, `E`, `T`,
`I`, `M`, `u`, `c`). The lexer distinguishes keywords from identifiers using
a lookup table after scanning the full identifier token:

```rust
fn classify_ident(text: &str) -> TokenKind {
    match text {
        "f"        => KwF,
        "v"        => KwV,
        "m"        => KwM,
        "c"        => KwC,
        "S"        => KwS,
        "E"        => KwE,
        "T"        => KwT,
        "I"        => KwI,
        "M"        => KwMod,
        "u"        => KwUse,
        "U"        => KwU,
        "af"       => KwAsyncFn,  // async fn
        "ret"      => KwRet,
        "loop"     => KwLoop,
        "break"    => KwBreak,
        "continue" => KwContinue,
        "yield"    => KwYield,
        "effect"   => KwEffect,
        "handle"   => KwHandle,
        "spec"     => KwSpec,
        "type"     => KwType,
        "static"   => KwStatic,
        "extern"   => KwExtern,
        "unsafe"   => KwUnsafe,
        "_T"       => UnderscoreT,  // Self type
        "_"        => Underscore,
        "1b"       => True,
        "0b"       => False,
        _          => Ident,
    }
}
```

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

Whitespace and comment tokens are produced by the lexer but filtered out
before the parser sees them. They are preserved in the token stream for
`rdx fmt` (the formatter) which needs them for layout preservation.

---

## 2.2 Parser Design

The parser is in `rdx_parser` (prototype: `prototype/src/parser.rs`).

### LL(1) Guarantee

Redox's grammar is deliberately LL(1) — every production can be determined
by looking at exactly one token of lookahead. This is the foundational
design decision that makes Redox agent-friendly.

**Why LL(1) matters for agents:**

- Deterministic: one token → one parse decision → no ambiguity
- Streaming: parse as tokens arrive, no buffering needed
- Error-resilient: easy to synchronize on delimiters after errors
- Simple mental model: agents reason about syntax without backtracking

### Parser Structure

```rust
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    pub fn parse(tokens: Vec<Token>) -> ParseResult<Ast> {
        let mut parser = Parser { tokens, pos: 0, diagnostics: vec![] };
        let items = parser.parse_items()?;
        Ok(Ast { items, diagnostics: parser.diagnostics })
    }
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

| First Token   | Parse Function         | Produces                     |
| ------------- | ---------------------- | ---------------------------- |
| `+` then `f`  | `parse_fn(Pub)`        | Public function              |
| `+` then `af` | `parse_async_fn(Pub)`  | Public async function        |
| `+` then `S`  | `parse_struct(Pub)`    | Public struct                |
| `+` then `E`  | `parse_enum(Pub)`      | Public enum                  |
| `+` then `T`  | `parse_trait(Pub)`     | Public trait                 |
| `+` then `M`  | `parse_module(Pub)`    | Public module                |
| `+` then `v`  | `parse_const(Pub)`     | Public constant              |
| `f`           | `parse_fn(Priv)`       | Private function             |
| `af`          | `parse_async_fn(Priv)` | Private async function       |
| `v`           | `parse_let()`          | Immutable binding            |
| `m`           | `parse_let_mut()`      | Mutable binding              |
| `S`           | `parse_struct(Priv)`   | Private struct               |
| `E`           | `parse_enum(Priv)`     | Private enum                 |
| `T`           | `parse_trait(Priv)`    | Private trait                |
| `I`           | `parse_impl()`         | Impl block                   |
| `M`           | `parse_module(Priv)`   | Private module               |
| `u`           | `parse_use()`          | Use declaration              |
| `@`           | `parse_attribute()`    | Attribute (then re-dispatch) |
| `?`           | `parse_if_or_match()`  | If expression or match       |
| `@` (in expr) | `parse_for_loop()`     | For loop                     |
| `loop`        | `parse_loop()`         | Infinite loop                |
| `ret`         | `parse_return()`       | Return expression            |

### Expression Parsing: Pratt Parser

Expressions use a Pratt parser (precedence climbing) for correct operator
binding:

```rust
fn parse_expr(&mut self, min_bp: u8) -> ParseResult<Expr> {
    let mut lhs = self.parse_prefix()?;

    loop {
        let (l_bp, r_bp) = match self.infix_binding_power(self.peek().kind) {
            Some(bp) => bp,
            None => break,
        };
        if l_bp < min_bp { break; }

        let op = self.advance();
        let rhs = self.parse_expr(r_bp)?;
        lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
    }

    Ok(lhs)
}
```

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

### Error Recovery

When the parser encounters an unexpected token:

1. Emit a diagnostic with the expected vs found tokens
2. **Synchronize** — skip tokens until a recovery point:
   - `;` (statement boundary)
   - `}` (block end)
   - Top-level keyword (`f`, `S`, `E`, `T`, `I`, `M`, `u`, `+`)
3. Resume parsing from the recovery point

This produces partial ASTs for error-tolerant tooling (IDE highlighting,
agent code generation with holes).

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

The lexer and parser are fuzz-tested with arbitrary byte sequences:

```bash
cargo fuzz run lexer_fuzz   # never panics on any input
cargo fuzz run parser_fuzz  # always produces AST or diagnostics
```

This is critical for agent usage — agents may generate malformed source and
the frontend must handle it gracefully.
