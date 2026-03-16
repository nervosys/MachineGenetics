/// Redox LL(1) Lexer — tokenizes Redox canonical syntax.
///
/// Design: single-pass, no backtracking, every token is unambiguous from
/// its first character. Optimized for streaming (agent consumption).
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    // Declaration keywords
    KwF,   // f (function)
    KwM,   // m (mutable binding / method)
    KwV,   // v (immutable binding)
    KwC,   // c (const)
    KwS,   // S (struct)
    KwE,   // E (enum)
    KwT,   // T (trait)
    KwI,   // I (impl)
    KwMod, // M (module)
    KwU,   // U (union) — context disambiguated from 'u' (use)
    KwUse, // u (use)

    // Visibility
    Plus, // + (pub prefix, also arithmetic)

    // Control flow
    Question,   // ? (if / match / option type / try)
    At,         // @ (for loop / attribute / struct literal / arc type)
    KwLoop,     // loop
    KwBreak,    // break
    KwContinue, // continue
    KwRet,      // ret
    KwYield,    // yield

    // Boolean
    True,  // 1b
    False, // 0b

    // Special identifiers
    Underscore,  // _
    UnderscoreT, // _T (Self)

    // Effect/contract/spec
    KwEffect, // effect
    KwHandle, // handle
    KwSpec,   // spec
    KwExtern, // extern

    // Safety (legacy mode)
    KwUnsafe, // unsafe

    // Other keywords
    KwType,   // type
    KwStatic, // static
    KwFor,    // for (in trait bounds)

    // Literals
    IntLiteral,
    FloatLiteral,
    StringLiteral,
    FormatString, // f"..."
    PrintString,  // p"..."
    CharLiteral,
    ByteLiteral,
    ByteStringLiteral,

    // Identifiers
    Ident,

    // Operators
    Minus,     // -
    Star,      // *
    Slash,     // /
    Percent,   // %
    Eq,        // ==
    Neq,       // !=
    Lt,        // <
    Gt,        // >
    Le,        // <=
    Ge,        // >=
    And,       // &&
    Or,        // ||
    Not,       // !
    BitAnd,    // & (also reference)
    BitOr,     // |
    BitXor,    // ^  (also owned ptr)
    Shl,       // <<
    Shr,       // >>
    Assign,    // =
    PlusEq,    // +=
    MinusEq,   // -=
    StarEq,    // *=
    SlashEq,   // /=
    PercentEq, // %=
    BitAndEq,  // &=
    BitOrEq,   // |=
    BitXorEq,  // ^=
    ShlEq,     // <<=
    ShrEq,     // >>=
    AndNot,    // &! (exclusive reference)

    // Delimiters
    LParen, // (
    RParen, // )
    LBrace, // {
    RBrace, // }
    LBrack, // [
    RBrack, // ]

    // Punctuation
    Semi,          // ;
    Comma,         // ,
    Dot,           // .
    Colon,         // :
    ColonQuestion, // :? (else-if)
    Arrow,         // ->
    FatArrow,      // =>
    Hash,          // #
    DotDot,        // ..
    DotDotEq,      // ..=
    Tilde,         // ~ (vec suffix)
    Dollar,        // $ (Rc type)

    // Special
    Eof,
    Error,
    Whitespace,
    Comment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub offset: usize,
    pub len: usize,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub text: String,
}

pub fn lex(source: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();

    loop {
        let tok = lexer.next_token();
        let is_eof = tok.kind == TokenKind::Eof;
        // Skip whitespace and comments for the parser
        if tok.kind != TokenKind::Whitespace && tok.kind != TokenKind::Comment {
            tokens.push(tok);
        }
        if is_eof {
            break;
        }
    }

    tokens
}

struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, bytes: source.as_bytes(), pos: 0, line: 1, col: 1 }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.bytes.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.bytes.get(self.pos).copied()?;
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(b)
    }

    fn make_token(
        &self,
        kind: TokenKind,
        start: usize,
        start_line: usize,
        start_col: usize,
    ) -> Token {
        Token {
            kind,
            span: Span { offset: start, len: self.pos - start, line: start_line, col: start_col },
            text: self.source[start..self.pos].to_string(),
        }
    }

    fn next_token(&mut self) -> Token {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        let Some(ch) = self.advance() else {
            return self.make_token(TokenKind::Eof, start, start_line, start_col);
        };

        match ch {
            // Whitespace
            b' ' | b'\t' | b'\r' | b'\n' => {
                while let Some(b) = self.peek() {
                    if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.make_token(TokenKind::Whitespace, start, start_line, start_col)
            }

            // Comments
            b'/' if self.peek() == Some(b'/') => {
                self.advance();
                while let Some(b) = self.peek() {
                    if b == b'\n' {
                        break;
                    }
                    self.advance();
                }
                self.make_token(TokenKind::Comment, start, start_line, start_col)
            }
            b'/' if self.peek() == Some(b'*') => {
                self.advance();
                let mut depth = 1u32;
                while depth > 0 {
                    match self.advance() {
                        Some(b'/') if self.peek() == Some(b'*') => {
                            self.advance();
                            depth += 1;
                        }
                        Some(b'*') if self.peek() == Some(b'/') => {
                            self.advance();
                            depth -= 1;
                        }
                        None => break,
                        _ => {}
                    }
                }
                self.make_token(TokenKind::Comment, start, start_line, start_col)
            }

            // String literals
            b'"' => self.lex_string(start, start_line, start_col, TokenKind::StringLiteral),

            // Char literal
            b'\'' => {
                // Read one char (possibly escaped)
                if self.peek() == Some(b'\\') {
                    self.advance();
                    self.advance(); // escape char
                } else {
                    self.advance();
                }
                if self.peek() == Some(b'\'') {
                    self.advance();
                }
                self.make_token(TokenKind::CharLiteral, start, start_line, start_col)
            }

            // Numbers
            b'0'..=b'9' => self.lex_number(start, start_line, start_col, ch),

            // Identifiers and keywords
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                self.lex_ident_or_keyword(start, start_line, start_col, ch)
            }

            // Operators and punctuation
            b'+' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::PlusEq, start, start_line, start_col)
            }
            b'+' => self.make_token(TokenKind::Plus, start, start_line, start_col),

            b'-' if self.peek() == Some(b'>') => {
                self.advance();
                self.make_token(TokenKind::Arrow, start, start_line, start_col)
            }
            b'-' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::MinusEq, start, start_line, start_col)
            }
            b'-' => self.make_token(TokenKind::Minus, start, start_line, start_col),

            b'*' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::StarEq, start, start_line, start_col)
            }
            b'*' => self.make_token(TokenKind::Star, start, start_line, start_col),

            b'/' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::SlashEq, start, start_line, start_col)
            }
            b'/' => self.make_token(TokenKind::Slash, start, start_line, start_col),

            b'%' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::PercentEq, start, start_line, start_col)
            }
            b'%' => self.make_token(TokenKind::Percent, start, start_line, start_col),

            b'=' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::Eq, start, start_line, start_col)
            }
            b'=' if self.peek() == Some(b'>') => {
                self.advance();
                self.make_token(TokenKind::FatArrow, start, start_line, start_col)
            }
            b'=' => self.make_token(TokenKind::Assign, start, start_line, start_col),

            b'!' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::Neq, start, start_line, start_col)
            }
            b'!' => self.make_token(TokenKind::Not, start, start_line, start_col),

            b'<' if self.peek() == Some(b'<') && self.peek2() == Some(b'=') => {
                self.advance();
                self.advance();
                self.make_token(TokenKind::ShlEq, start, start_line, start_col)
            }
            b'<' if self.peek() == Some(b'<') => {
                self.advance();
                self.make_token(TokenKind::Shl, start, start_line, start_col)
            }
            b'<' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::Le, start, start_line, start_col)
            }
            b'<' => self.make_token(TokenKind::Lt, start, start_line, start_col),

            b'>' if self.peek() == Some(b'>') && self.peek2() == Some(b'=') => {
                self.advance();
                self.advance();
                self.make_token(TokenKind::ShrEq, start, start_line, start_col)
            }
            b'>' if self.peek() == Some(b'>') => {
                self.advance();
                self.make_token(TokenKind::Shr, start, start_line, start_col)
            }
            b'>' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::Ge, start, start_line, start_col)
            }
            b'>' => self.make_token(TokenKind::Gt, start, start_line, start_col),

            b'&' if self.peek() == Some(b'&') => {
                self.advance();
                self.make_token(TokenKind::And, start, start_line, start_col)
            }
            b'&' if self.peek() == Some(b'!') => {
                self.advance();
                self.make_token(TokenKind::AndNot, start, start_line, start_col)
            }
            b'&' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::BitAndEq, start, start_line, start_col)
            }
            b'&' => self.make_token(TokenKind::BitAnd, start, start_line, start_col),

            b'|' if self.peek() == Some(b'|') => {
                self.advance();
                self.make_token(TokenKind::Or, start, start_line, start_col)
            }
            b'|' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::BitOrEq, start, start_line, start_col)
            }
            b'|' => self.make_token(TokenKind::BitOr, start, start_line, start_col),

            b'^' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::BitXorEq, start, start_line, start_col)
            }
            b'^' => self.make_token(TokenKind::BitXor, start, start_line, start_col),

            // Delimiters
            b'(' => self.make_token(TokenKind::LParen, start, start_line, start_col),
            b')' => self.make_token(TokenKind::RParen, start, start_line, start_col),
            b'{' => self.make_token(TokenKind::LBrace, start, start_line, start_col),
            b'}' => self.make_token(TokenKind::RBrace, start, start_line, start_col),
            b'[' => self.make_token(TokenKind::LBrack, start, start_line, start_col),
            b']' => self.make_token(TokenKind::RBrack, start, start_line, start_col),

            // Punctuation
            b';' => self.make_token(TokenKind::Semi, start, start_line, start_col),
            b',' => self.make_token(TokenKind::Comma, start, start_line, start_col),
            b'.' if self.peek() == Some(b'.') && self.peek2() == Some(b'=') => {
                self.advance();
                self.advance();
                self.make_token(TokenKind::DotDotEq, start, start_line, start_col)
            }
            b'.' if self.peek() == Some(b'.') => {
                self.advance();
                self.make_token(TokenKind::DotDot, start, start_line, start_col)
            }
            b'.' => self.make_token(TokenKind::Dot, start, start_line, start_col),

            b':' if self.peek() == Some(b'?') => {
                self.advance();
                self.make_token(TokenKind::ColonQuestion, start, start_line, start_col)
            }
            b':' => self.make_token(TokenKind::Colon, start, start_line, start_col),

            b'?' => self.make_token(TokenKind::Question, start, start_line, start_col),
            b'@' => self.make_token(TokenKind::At, start, start_line, start_col),
            b'#' => self.make_token(TokenKind::Hash, start, start_line, start_col),
            b'~' => self.make_token(TokenKind::Tilde, start, start_line, start_col),
            b'$' => self.make_token(TokenKind::Dollar, start, start_line, start_col),

            _ => self.make_token(TokenKind::Error, start, start_line, start_col),
        }
    }

    fn lex_string(
        &mut self,
        start: usize,
        start_line: usize,
        start_col: usize,
        kind: TokenKind,
    ) -> Token {
        loop {
            match self.advance() {
                Some(b'"') => break,
                Some(b'\\') => {
                    self.advance();
                }
                None => break,
                _ => {}
            }
        }
        self.make_token(kind, start, start_line, start_col)
    }

    fn lex_number(
        &mut self,
        start: usize,
        start_line: usize,
        start_col: usize,
        first: u8,
    ) -> Token {
        let mut is_float = false;

        // Check for hex/oct/bin prefix
        if first == b'0' {
            match self.peek() {
                Some(b'x') | Some(b'X') => {
                    self.advance();
                    while let Some(b) = self.peek() {
                        if b.is_ascii_hexdigit() || b == b'_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    // Consume optional suffix
                    self.lex_int_suffix();
                    return self.make_token(TokenKind::IntLiteral, start, start_line, start_col);
                }
                Some(b'o') => {
                    self.advance();
                    while let Some(b) = self.peek() {
                        if (b'0'..=b'7').contains(&b) || b == b'_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.lex_int_suffix();
                    return self.make_token(TokenKind::IntLiteral, start, start_line, start_col);
                }
                Some(b'b') if self.peek2().is_some_and(|c| c == b'0' || c == b'1' || c == b'_') => {
                    self.advance();
                    while let Some(b) = self.peek() {
                        if b == b'0' || b == b'1' || b == b'_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.lex_int_suffix();
                    return self.make_token(TokenKind::IntLiteral, start, start_line, start_col);
                }
                _ => {}
            }
        }

        // Decimal digits
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'_' {
                self.advance();
            } else {
                break;
            }
        }

        // Fractional part
        if self.peek() == Some(b'.') && self.peek2().is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            self.advance(); // consume '.'
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() || b == b'_' {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Exponent
        if let Some(b'e') | Some(b'E') = self.peek() {
            is_float = true;
            self.advance();
            if let Some(b'+') | Some(b'-') = self.peek() {
                self.advance();
            }
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() || b == b'_' {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Suffix
        if is_float {
            self.lex_float_suffix();
            self.make_token(TokenKind::FloatLiteral, start, start_line, start_col)
        } else {
            // Check for 'b' suffix (boolean: 0b, 1b) — but only if this is exactly "0" or "1"
            let text = &self.source[start..self.pos];
            if (text == "0" || text == "1") && self.peek() == Some(b'b') {
                // Check that next char after 'b' is not alphanumeric (not 0b... binary literal)
                if !self.peek2().is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_') {
                    self.advance();
                    let kind = if text == "1" { TokenKind::True } else { TokenKind::False };
                    return self.make_token(kind, start, start_line, start_col);
                }
            }
            self.lex_int_suffix();
            self.make_token(TokenKind::IntLiteral, start, start_line, start_col)
        }
    }

    fn lex_int_suffix(&mut self) {
        // Look for i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
        let remaining = &self.source[self.pos..];
        let suffixes = [
            "i128", "isize", "i64", "i32", "i16", "i8", "u128", "usize", "u64", "u32", "u16", "u8",
        ];
        for s in &suffixes {
            if remaining.starts_with(s) {
                // Make sure next char is not alphanumeric
                let after = self.bytes.get(self.pos + s.len());
                if !after.is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
                    for _ in 0..s.len() {
                        self.advance();
                    }
                    break;
                }
            }
        }
    }

    fn lex_float_suffix(&mut self) {
        let remaining = &self.source[self.pos..];
        for s in &["f64", "f32"] {
            if remaining.starts_with(s) {
                let after = self.bytes.get(self.pos + s.len());
                if !after.is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
                    for _ in 0..s.len() {
                        self.advance();
                    }
                    break;
                }
            }
        }
    }

    fn lex_ident_or_keyword(
        &mut self,
        start: usize,
        start_line: usize,
        start_col: usize,
        _first: u8,
    ) -> Token {
        // Consume rest of identifier
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.advance();
            } else {
                break;
            }
        }

        let text = &self.source[start..self.pos];

        // Check for format/print strings: f"..." and p"..."
        if (text == "f" || text == "p") && self.peek() == Some(b'"') {
            self.advance(); // consume opening quote
            let kind = if text == "f" { TokenKind::FormatString } else { TokenKind::PrintString };
            return self.lex_string(start, start_line, start_col, kind);
        }

        let kind = match text {
            // Single-char declaration keywords
            "f" => TokenKind::KwF,
            "m" => TokenKind::KwM,
            "v" => TokenKind::KwV,
            "c" => TokenKind::KwC,
            "S" => TokenKind::KwS,
            "E" => TokenKind::KwE,
            "T" => TokenKind::KwT,
            "I" => TokenKind::KwI,
            "M" => TokenKind::KwMod,
            "U" => TokenKind::KwU,
            "u" => TokenKind::KwUse,
            "s" => TokenKind::Ident, // 's' is a type, not a keyword; parser handles

            // Multi-char keywords
            "loop" => TokenKind::KwLoop,
            "break" => TokenKind::KwBreak,
            "continue" => TokenKind::KwContinue,
            "ret" => TokenKind::KwRet,
            "yield" => TokenKind::KwYield,
            "effect" => TokenKind::KwEffect,
            "handle" => TokenKind::KwHandle,
            "spec" => TokenKind::KwSpec,
            "extern" => TokenKind::KwExtern,
            "unsafe" => TokenKind::KwUnsafe,
            "type" => TokenKind::KwType,
            "static" => TokenKind::KwStatic,
            "for" => TokenKind::KwFor,

            // Special identifiers
            "_" => TokenKind::Underscore,
            "_T" => TokenKind::UnderscoreT,

            _ => TokenKind::Ident,
        };

        self.make_token(kind, start, start_line, start_col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_def() {
        let tokens = lex("+f add(a: i32, b: i32) -> i32 { a + b }");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(kinds[0], TokenKind::Plus);
        assert_eq!(kinds[1], TokenKind::KwF);
        assert_eq!(kinds[2], TokenKind::Ident); // add
        assert_eq!(kinds[3], TokenKind::LParen);
    }

    #[test]
    fn test_struct_def() {
        let tokens = lex("+S Point { x: f64, y: f64, }");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(kinds[0], TokenKind::Plus);
        assert_eq!(kinds[1], TokenKind::KwS);
        assert_eq!(kinds[2], TokenKind::Ident); // Point
    }

    #[test]
    fn test_bool_literals() {
        let tokens = lex("1b 0b");
        assert_eq!(tokens[0].kind, TokenKind::True);
        assert_eq!(tokens[1].kind, TokenKind::False);
    }

    #[test]
    fn test_operators() {
        let tokens = lex("&! == != <= >= << >> => ->");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(kinds[0], TokenKind::AndNot);
        assert_eq!(kinds[1], TokenKind::Eq);
        assert_eq!(kinds[2], TokenKind::Neq);
        assert_eq!(kinds[3], TokenKind::Le);
        assert_eq!(kinds[4], TokenKind::Ge);
        assert_eq!(kinds[5], TokenKind::Shl);
        assert_eq!(kinds[6], TokenKind::Shr);
        assert_eq!(kinds[7], TokenKind::FatArrow);
        assert_eq!(kinds[8], TokenKind::Arrow);
    }

    #[test]
    fn test_string_literal() {
        let tokens = lex(r#""hello world""#);
        assert_eq!(tokens[0].kind, TokenKind::StringLiteral);
    }

    #[test]
    fn test_format_string() {
        let tokens = lex(r#"f"x = {x}""#);
        assert_eq!(tokens[0].kind, TokenKind::FormatString);
    }

    #[test]
    fn test_generic_brackets() {
        let tokens = lex("Vec[i32]");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(kinds[0], TokenKind::Ident); // Vec
        assert_eq!(kinds[1], TokenKind::LBrack);
        assert_eq!(kinds[2], TokenKind::Ident); // i32
        assert_eq!(kinds[3], TokenKind::RBrack);
    }

    #[test]
    fn test_attribute() {
        let tokens = lex("@d(Cl,Db)");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(kinds[0], TokenKind::At);
        assert_eq!(kinds[1], TokenKind::Ident); // d
    }

    #[test]
    fn test_hex_literal() {
        let tokens = lex("0xFF_AB");
        assert_eq!(tokens[0].kind, TokenKind::IntLiteral);
        assert_eq!(tokens[0].text, "0xFF_AB");
    }
}
