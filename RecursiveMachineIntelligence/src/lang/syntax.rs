//! RMIL text syntax — human-readable surface syntax for debugging and authoring.
//!
//! Provides [`parse`] to convert text into an [`Expr`] AST, and [`pretty`] to
//! render an `Expr` back to text. Round-trips are supported:
//!
//! ```
//! use rmi::lang::syntax;
//! use rmi::lang::{Expr, Op};
//!
//! let text = "relu >> linear >> softmax";
//! let expr = syntax::parse(text).unwrap();
//! let rendered = syntax::pretty(&expr);
//! assert_eq!(rendered, text);
//! ```
//!
//! ## Grammar (informal)
//!
//! ```text
//! program  = pipeline
//! pipeline = parallel (">>" parallel)*
//! parallel = atom ("|" atom)*
//! atom     = "if" pipeline "then" pipeline "else" pipeline
//!          | "let" IDENT "=" pipeline "in" pipeline
//!          | "\\" "(" params ")" "->" pipeline
//!          | "{" (pipeline ";")* pipeline? "}"
//!          | call
//! call     = primary ("(" args ")")?
//! primary  = IDENT | INT | FLOAT | "true" | "false" | "nil" | "$" IDENT
//!          | "(" pipeline ")"
//! args     = pipeline ("," pipeline)*
//! params   = (IDENT ":" type) ("," IDENT ":" type)*
//! type     = "f32" | "f64" | "i64" | "bool" | ...
//! ```

use crate::lang::expr::{Expr, Val};
use crate::lang::op::Op;
use crate::lang::sym::SymbolTable;
use crate::lang::ty::{Dtype, Ty};

// ── Error ────────────────────────────────────────────────────────────────────

/// Parse error with position information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Human-readable error message.
    pub message: String,
    /// Byte offset in the source text.
    pub offset: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

// ── Token ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String), // relu, linear, x, ...
    Int(i64),      // 42, -7
    Float(f64),    // 3.14, -0.5
    True,          // true
    False,         // false
    Nil,           // nil
    Dollar,        // $
    Arrow,         // ->
    FatArrow,      // >>
    Pipe,          // |
    LParen,        // (
    RParen,        // )
    LBrace,        // {
    RBrace,        // }
    Comma,         // ,
    Colon,         // :
    Semi,          // ;
    Eq,            // =
    Backslash,     // \ (lambda)
    If,            // if
    Then,          // then
    Else,          // else
    Let,           // let
    In,            // in
    Eof,
}

// ── Lexer ────────────────────────────────────────────────────────────────────

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            // Skip line comments
            if self.pos + 1 < self.src.len()
                && self.src[self.pos] == b'/'
                && self.src[self.pos + 1] == b'/'
            {
                while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_ws_and_comments();

        if self.pos >= self.src.len() {
            return Ok(Token::Eof);
        }

        let start = self.pos;
        let ch = self.src[self.pos];

        match ch {
            b'(' => {
                self.pos += 1;
                Ok(Token::LParen)
            }
            b')' => {
                self.pos += 1;
                Ok(Token::RParen)
            }
            b'{' => {
                self.pos += 1;
                Ok(Token::LBrace)
            }
            b'}' => {
                self.pos += 1;
                Ok(Token::RBrace)
            }
            b',' => {
                self.pos += 1;
                Ok(Token::Comma)
            }
            b':' => {
                self.pos += 1;
                Ok(Token::Colon)
            }
            b';' => {
                self.pos += 1;
                Ok(Token::Semi)
            }
            b'=' => {
                self.pos += 1;
                Ok(Token::Eq)
            }
            b'$' => {
                self.pos += 1;
                Ok(Token::Dollar)
            }
            b'\\' => {
                self.pos += 1;
                Ok(Token::Backslash)
            }
            b'|' => {
                self.pos += 1;
                Ok(Token::Pipe)
            }
            b'>' => {
                if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'>' {
                    self.pos += 2;
                    Ok(Token::FatArrow)
                } else {
                    Err(ParseError {
                        message: "expected '>>'".into(),
                        offset: start,
                    })
                }
            }
            b'-' => {
                if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'>' {
                    self.pos += 2;
                    Ok(Token::Arrow)
                } else if self.pos + 1 < self.src.len() && self.src[self.pos + 1].is_ascii_digit() {
                    self.lex_number()
                } else {
                    Err(ParseError {
                        message: "unexpected '-'".into(),
                        offset: start,
                    })
                }
            }
            b'0'..=b'9' => self.lex_number(),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_ident(),
            _ => Err(ParseError {
                message: format!("unexpected character '{}'", ch as char),
                offset: start,
            }),
        }
    }

    fn lex_number(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        if self.pos < self.src.len() && self.src[self.pos] == b'-' {
            self.pos += 1;
        }
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let is_float = self.pos < self.src.len()
            && self.src[self.pos] == b'.'
            && self.pos + 1 < self.src.len()
            && self.src[self.pos + 1].is_ascii_digit();
        if is_float {
            self.pos += 1; // skip '.'
            while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            // Optional exponent
            if self.pos < self.src.len()
                && (self.src[self.pos] == b'e' || self.src[self.pos] == b'E')
            {
                self.pos += 1;
                if self.pos < self.src.len()
                    && (self.src[self.pos] == b'+' || self.src[self.pos] == b'-')
                {
                    self.pos += 1;
                }
                while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
            }
            let s = std::str::from_utf8(&self.src[start..self.pos])
                .expect("numeric literal is valid UTF-8");
            let v: f64 = s.parse().map_err(|_| ParseError {
                message: format!("invalid float: {s}"),
                offset: start,
            })?;
            Ok(Token::Float(v))
        } else {
            let s = std::str::from_utf8(&self.src[start..self.pos])
                .expect("integer literal is valid UTF-8");
            let v: i64 = s.parse().map_err(|_| ParseError {
                message: format!("invalid integer: {s}"),
                offset: start,
            })?;
            Ok(Token::Int(v))
        }
    }

    fn lex_ident(&mut self) -> Result<Token, ParseError> {
        let start = self.pos;
        while self.pos < self.src.len()
            && (self.src[self.pos].is_ascii_alphanumeric() || self.src[self.pos] == b'_')
        {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).expect("identifier is valid UTF-8");
        Ok(match s {
            "true" => Token::True,
            "false" => Token::False,
            "nil" => Token::Nil,
            "if" => Token::If,
            "then" => Token::Then,
            "else" => Token::Else,
            "let" => Token::Let,
            "in" => Token::In,
            _ => Token::Ident(s.to_string()),
        })
    }
}

// ── Op from name ─────────────────────────────────────────────────────────────

/// Look up an opcode by its text name.
pub fn op_from_name(name: &str) -> Option<Op> {
    Op::ALL.iter().find(|op| op.name() == name).copied()
}

// ── Parser ───────────────────────────────────────────────────────────────────

struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    symbols: SymbolTable,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(src);
        let current = lexer.next_token()?;
        Ok(Self {
            lexer,
            current,
            symbols: SymbolTable::new(),
        })
    }

    fn advance(&mut self) -> Result<Token, ParseError> {
        let prev = std::mem::replace(&mut self.current, Token::Eof);
        self.current = self.lexer.next_token()?;
        Ok(prev)
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        if std::mem::discriminant(&self.current) == std::mem::discriminant(expected) {
            self.advance()?;
            Ok(())
        } else {
            Err(ParseError {
                message: format!("expected {:?}, got {:?}", expected, self.current),
                offset: self.lexer.pos,
            })
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.advance()? {
            Token::Ident(s) => Ok(s),
            other => Err(ParseError {
                message: format!("expected identifier, got {:?}", other),
                offset: self.lexer.pos,
            }),
        }
    }

    // ── Grammar productions ──────────────────────────────────────────────

    /// program = pipeline EOF
    fn parse_program(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_pipeline()?;
        if self.current != Token::Eof {
            return Err(ParseError {
                message: format!("expected end of input, got {:?}", self.current),
                offset: self.lexer.pos,
            });
        }
        Ok(expr)
    }

    /// pipeline = parallel (">>" parallel)*
    fn parse_pipeline(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_parallel()?;
        while self.current == Token::FatArrow {
            self.advance()?;
            let right = self.parse_parallel()?;
            left = Expr::Seq(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// parallel = atom ("|" atom)*
    fn parse_parallel(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_atom()?;
        while self.current == Token::Pipe {
            self.advance()?;
            let right = self.parse_atom()?;
            left = Expr::Par(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// atom = if/let/lambda/block/call
    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        match &self.current {
            Token::If => self.parse_if(),
            Token::Let => self.parse_let(),
            Token::Backslash => self.parse_lambda(),
            Token::LBrace => self.parse_block(),
            _ => self.parse_call(),
        }
    }

    /// if pipeline then pipeline else pipeline
    fn parse_if(&mut self) -> Result<Expr, ParseError> {
        self.advance()?; // consume 'if'
        let pred = self.parse_pipeline()?;
        self.expect(&Token::Then)?;
        let yes = self.parse_pipeline()?;
        self.expect(&Token::Else)?;
        let no = self.parse_pipeline()?;
        Ok(Expr::Cond {
            pred: Box::new(pred),
            yes: Box::new(yes),
            no: Box::new(no),
        })
    }

    /// let IDENT = pipeline in pipeline
    fn parse_let(&mut self) -> Result<Expr, ParseError> {
        self.advance()?; // consume 'let'
        let name_str = self.expect_ident()?;
        self.expect(&Token::Eq)?;
        let val = self.parse_pipeline()?;
        self.expect(&Token::In)?;
        let body = self.parse_pipeline()?;
        let sym = self.symbols.intern(&name_str);
        Ok(Expr::Let {
            name: sym,
            val: Box::new(val),
            body: Box::new(body),
        })
    }

    /// \(params) -> pipeline
    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        self.advance()?; // consume '\'
        self.expect(&Token::LParen)?;
        let mut params = Vec::new();
        if self.current != Token::RParen {
            loop {
                let name_str = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let ty = self.parse_type()?;
                let sym = self.symbols.intern(&name_str);
                params.push((sym, ty));
                if self.current != Token::Comma {
                    break;
                }
                self.advance()?;
            }
        }
        self.expect(&Token::RParen)?;
        self.expect(&Token::Arrow)?;
        let body = self.parse_pipeline()?;
        Ok(Expr::Lam {
            params,
            body: Box::new(body),
        })
    }

    /// { pipeline ; pipeline ; ... }
    fn parse_block(&mut self) -> Result<Expr, ParseError> {
        self.advance()?; // consume '{'
        let mut exprs = Vec::new();
        while self.current != Token::RBrace && self.current != Token::Eof {
            exprs.push(self.parse_pipeline()?);
            if self.current == Token::Semi {
                self.advance()?;
            }
        }
        self.expect(&Token::RBrace)?;
        if exprs.len() == 1 {
            Ok(exprs
                .into_iter()
                .next()
                .expect("block has at least one expr after parsing"))
        } else {
            Ok(Expr::Block(exprs))
        }
    }

    /// call = primary ( "(" args ")" )?
    fn parse_call(&mut self) -> Result<Expr, ParseError> {
        let primary = self.parse_primary()?;

        // If this is an Ident that resolved to an op and we see '(',
        // parse as op application with explicit args
        if self.current == Token::LParen {
            if let Expr::App(op, _) = &primary {
                let op = *op;
                self.advance()?; // consume '('
                let args = self.parse_args()?;
                self.expect(&Token::RParen)?;
                return Ok(Expr::App(op, args));
            }
            // Function call: expr(args)
            self.advance()?;
            let args = self.parse_args()?;
            self.expect(&Token::RParen)?;
            return Ok(Expr::Call(Box::new(primary), args));
        }

        Ok(primary)
    }

    /// primary = IDENT | INT | FLOAT | true | false | nil | $IDENT | (pipeline)
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match &self.current {
            Token::Ident(name) => {
                let name = name.clone();
                self.advance()?;
                // Try opcode lookup first
                if let Some(op) = op_from_name(&name) {
                    Ok(Expr::op1(op))
                } else {
                    // It's a symbol reference
                    let sym = self.symbols.intern(&name);
                    Ok(Expr::Ref(sym))
                }
            }
            Token::Int(v) => {
                let v = *v;
                self.advance()?;
                Ok(Expr::int(v))
            }
            Token::Float(v) => {
                let v = *v as f32;
                self.advance()?;
                Ok(Expr::float(v))
            }
            Token::True => {
                self.advance()?;
                Ok(Expr::boolean(true))
            }
            Token::False => {
                self.advance()?;
                Ok(Expr::boolean(false))
            }
            Token::Nil => {
                self.advance()?;
                Ok(Expr::Lit(Val::Nil))
            }
            Token::Dollar => {
                self.advance()?; // consume '$'
                let name = self.expect_ident()?;
                let sym = self.symbols.intern(&name);
                Ok(Expr::Ref(sym))
            }
            Token::LParen => {
                self.advance()?;
                let expr = self.parse_pipeline()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            _ => Err(ParseError {
                message: format!("unexpected token {:?}", self.current),
                offset: self.lexer.pos,
            }),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.current != Token::RParen {
            args.push(self.parse_pipeline()?);
            while self.current == Token::Comma {
                self.advance()?;
                args.push(self.parse_pipeline()?);
            }
        }
        Ok(args)
    }

    fn parse_type(&mut self) -> Result<Ty, ParseError> {
        let name = self.expect_ident()?;
        match name.as_str() {
            "f32" => Ok(Ty::f32()),
            "f64" => Ok(Ty::f64()),
            "i64" => Ok(Ty::i64()),
            "bool" => Ok(Ty::bool()),
            "void" => Ok(Ty::Void),
            "sym" => Ok(Ty::Sym),
            _ => Err(ParseError {
                message: format!("unknown type: {name}"),
                offset: self.lexer.pos,
            }),
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Parse RMIL text syntax into an expression AST.
///
/// Returns the expression and the symbol table built during parsing.
///
/// # Examples
///
/// ```
/// use rmi::lang::syntax;
///
/// let (expr, _syms) = syntax::parse_with_symbols("relu >> linear >> softmax").unwrap();
/// assert_eq!(expr.node_count(), 5); // 3 ops + 2 Seq nodes
/// ```
pub fn parse_with_symbols(src: &str) -> Result<(Expr, SymbolTable), ParseError> {
    let mut parser = Parser::new(src)?;
    let expr = parser.parse_program()?;
    Ok((expr, parser.symbols))
}

/// Parse RMIL text syntax into an expression AST (discarding the symbol table).
///
/// # Examples
///
/// ```
/// use rmi::lang::syntax;
///
/// let expr = syntax::parse("add(3, 4)").unwrap();
/// ```
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    parse_with_symbols(src).map(|(expr, _)| expr)
}

// ── Pretty printer ───────────────────────────────────────────────────────────

/// Render an RMIL expression as human-readable text.
///
/// This is the inverse of [`parse`]. Attempts to produce output that
/// round-trips through the parser.
pub fn pretty(expr: &Expr) -> String {
    let mut buf = String::new();
    pretty_expr(expr, &mut buf, Prec::Top);
    buf
}

/// Render with a symbol table for resolving `Ref` names.
pub fn pretty_with_symbols(expr: &Expr, symbols: &SymbolTable) -> String {
    let mut buf = String::new();
    pretty_expr_sym(expr, &mut buf, Prec::Top, Some(symbols));
    buf
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Top,      // lowest — no parens needed
    Pipeline, // >> level
    Parallel, // | level
    Atom,     // highest — always parenthesise composites
}

fn pretty_expr(expr: &Expr, buf: &mut String, prec: Prec) {
    pretty_expr_sym(expr, buf, prec, None);
}

fn pretty_expr_sym(expr: &Expr, buf: &mut String, prec: Prec, symbols: Option<&SymbolTable>) {
    match expr {
        Expr::Lit(val) => pretty_val(val, buf),

        Expr::Ref(sym) => {
            if let Some(st) = symbols {
                if let Some(name) = st.try_resolve(*sym) {
                    if !name.is_empty() {
                        buf.push('$');
                        buf.push_str(name);
                        return;
                    }
                }
            }
            buf.push_str(&format!("${}", sym.0));
        }

        Expr::App(op, args) if args.is_empty() => {
            buf.push_str(op.name());
        }

        Expr::App(op, args) => {
            buf.push_str(op.name());
            buf.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                pretty_expr_sym(arg, buf, Prec::Top, symbols);
            }
            buf.push(')');
        }

        Expr::Seq(a, b) => {
            if prec > Prec::Pipeline {
                buf.push('(');
            }
            pretty_expr_sym(a, buf, Prec::Pipeline, symbols);
            buf.push_str(" >> ");
            pretty_expr_sym(b, buf, Prec::Pipeline, symbols);
            if prec > Prec::Pipeline {
                buf.push(')');
            }
        }

        Expr::Par(a, b) => {
            if prec > Prec::Parallel {
                buf.push('(');
            }
            pretty_expr_sym(a, buf, Prec::Parallel, symbols);
            buf.push_str(" | ");
            pretty_expr_sym(b, buf, Prec::Parallel, symbols);
            if prec > Prec::Parallel {
                buf.push(')');
            }
        }

        Expr::Cond { pred, yes, no } => {
            buf.push_str("if ");
            pretty_expr_sym(pred, buf, Prec::Top, symbols);
            buf.push_str(" then ");
            pretty_expr_sym(yes, buf, Prec::Top, symbols);
            buf.push_str(" else ");
            pretty_expr_sym(no, buf, Prec::Top, symbols);
        }

        Expr::Let { name, val, body } => {
            buf.push_str("let ");
            if let Some(st) = symbols {
                if let Some(n) = st.try_resolve(*name) {
                    buf.push_str(n);
                } else {
                    buf.push_str(&format!("_{}", name.0));
                }
            } else {
                buf.push_str(&format!("_{}", name.0));
            }
            buf.push_str(" = ");
            pretty_expr_sym(val, buf, Prec::Top, symbols);
            buf.push_str(" in ");
            pretty_expr_sym(body, buf, Prec::Top, symbols);
        }

        Expr::Lam { params, body } => {
            buf.push_str("\\(");
            for (i, (sym, ty)) in params.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                if let Some(st) = symbols {
                    if let Some(n) = st.try_resolve(*sym) {
                        buf.push_str(n);
                    } else {
                        buf.push_str(&format!("_{}", sym.0));
                    }
                } else {
                    buf.push_str(&format!("_{}", sym.0));
                }
                buf.push_str(": ");
                pretty_ty(ty, buf);
            }
            buf.push_str(") -> ");
            pretty_expr_sym(body, buf, Prec::Top, symbols);
        }

        Expr::Call(func, args) => {
            pretty_expr_sym(func, buf, Prec::Atom, symbols);
            buf.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                pretty_expr_sym(arg, buf, Prec::Top, symbols);
            }
            buf.push(')');
        }

        Expr::Block(exprs) => {
            buf.push_str("{ ");
            for (i, e) in exprs.iter().enumerate() {
                if i > 0 {
                    buf.push_str("; ");
                }
                pretty_expr_sym(e, buf, Prec::Top, symbols);
            }
            buf.push_str(" }");
        }
    }
}

fn pretty_val(val: &Val, buf: &mut String) {
    match val {
        Val::Nil => buf.push_str("nil"),
        Val::Bool(true) => buf.push_str("true"),
        Val::Bool(false) => buf.push_str("false"),
        Val::I64(v) => buf.push_str(&v.to_string()),
        Val::F32(bits) => {
            let v = f32::from_bits(*bits);
            if v.fract() == 0.0 && v.is_finite() {
                buf.push_str(&format!("{v:.1}"));
            } else {
                buf.push_str(&format!("{v}"));
            }
        }
        Val::F64(bits) => {
            let v = f64::from_bits(*bits);
            if v.fract() == 0.0 && v.is_finite() {
                buf.push_str(&format!("{v:.1}"));
            } else {
                buf.push_str(&format!("{v}"));
            }
        }
        Val::Sym(s) => buf.push_str(&format!("${}", s.0)),
        Val::Tensor { dtype, shape, .. } => {
            buf.push_str(&format!("tensor<{:?}, {:?}>", dtype, shape));
        }
        Val::Tuple(vs) => {
            buf.push('(');
            for (i, v) in vs.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                pretty_val(v, buf);
            }
            buf.push(')');
        }
    }
}

fn pretty_ty(ty: &Ty, buf: &mut String) {
    match ty {
        Ty::Void => buf.push_str("void"),
        Ty::Scalar(Dtype::F32) => buf.push_str("f32"),
        Ty::Scalar(Dtype::F64) => buf.push_str("f64"),
        Ty::Scalar(Dtype::I64) => buf.push_str("i64"),
        Ty::Scalar(Dtype::Bool) => buf.push_str("bool"),
        Ty::Scalar(d) => buf.push_str(&format!("{d:?}").to_lowercase()),
        Ty::Tensor(d, shape) => {
            buf.push_str(&format!("tensor<{d:?}"));
            for s in shape {
                buf.push_str(&format!(", {s}"));
            }
            buf.push('>');
        }
        Ty::Sym => buf.push_str("sym"),
        _ => buf.push_str(&format!("{ty}")),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Op;

    #[test]
    fn parse_single_op() {
        let expr = parse("relu").unwrap();
        assert_eq!(expr, Expr::op1(Op::RELU));
    }

    #[test]
    fn parse_pipeline() {
        let expr = parse("relu >> linear >> softmax").unwrap();
        // relu >> (linear >> softmax) — left-associative
        match &expr {
            Expr::Seq(_, _) => {} // ok, it's a Seq
            other => panic!("expected Seq, got {:?}", other),
        }
        assert_eq!(expr.node_count(), 5); // 3 ops + 2 Seq
    }

    #[test]
    fn parse_parallel() {
        let expr = parse("relu | sigmoid").unwrap();
        assert!(matches!(expr, Expr::Par(_, _)));
    }

    #[test]
    fn parse_op_with_args() {
        let expr = parse("add(3, 4)").unwrap();
        match &expr {
            Expr::App(op, args) => {
                assert_eq!(*op, Op::ADD);
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected App, got {:?}", other),
        }
    }

    #[test]
    fn parse_nested_ops() {
        let expr = parse("add(mul(2, 3), 4)").unwrap();
        match &expr {
            Expr::App(op, args) => {
                assert_eq!(*op, Op::ADD);
                assert_eq!(args.len(), 2);
                assert!(matches!(&args[0], Expr::App(op, _) if *op == Op::MUL));
            }
            other => panic!("expected App, got {:?}", other),
        }
    }

    #[test]
    fn parse_literals() {
        assert_eq!(parse("42").unwrap(), Expr::int(42));
        assert_eq!(parse("-7").unwrap(), Expr::int(-7));
        assert_eq!(parse("true").unwrap(), Expr::boolean(true));
        assert_eq!(parse("false").unwrap(), Expr::boolean(false));
        assert_eq!(parse("nil").unwrap(), Expr::Lit(Val::Nil));
    }

    #[test]
    fn parse_float() {
        let expr = parse("3.15").unwrap();
        match &expr {
            Expr::Lit(Val::F32(bits)) => {
                let v = f32::from_bits(*bits);
                assert!((v - 3.15).abs() < 0.01);
            }
            other => panic!("expected float literal, got {:?}", other),
        }
    }

    #[test]
    fn parse_conditional() {
        let expr = parse("if true then relu else gelu").unwrap();
        assert!(matches!(expr, Expr::Cond { .. }));
    }

    #[test]
    fn parse_let_binding() {
        let expr = parse("let x = 42 in add($x, 1)").unwrap();
        assert!(matches!(expr, Expr::Let { .. }));
    }

    #[test]
    fn parse_lambda() {
        let expr = parse("\\(x: f32) -> add($x, 1.0)").unwrap();
        match &expr {
            Expr::Lam { params, .. } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].1, Ty::f32());
            }
            other => panic!("expected Lam, got {:?}", other),
        }
    }

    #[test]
    fn parse_block() {
        let expr = parse("{ relu; linear; softmax }").unwrap();
        match &expr {
            Expr::Block(exprs) => assert_eq!(exprs.len(), 3),
            other => panic!("expected Block, got {:?}", other),
        }
    }

    #[test]
    fn parse_symbol_ref() {
        let (expr, syms) = parse_with_symbols("$my_var").unwrap();
        match &expr {
            Expr::Ref(s) => {
                let name = syms.resolve(*s);
                assert_eq!(name, "my_var");
            }
            other => panic!("expected Ref, got {:?}", other),
        }
    }

    #[test]
    fn parse_parens_grouping() {
        let expr = parse("(relu >> linear) | softmax").unwrap();
        match &expr {
            Expr::Par(left, _) => {
                assert!(matches!(**left, Expr::Seq(_, _)));
            }
            other => panic!("expected Par, got {:?}", other),
        }
    }

    #[test]
    fn parse_complex_pipeline() {
        let expr =
            parse("layer_norm >> attn >> drop >> layer_norm >> linear >> gelu >> linear >> drop")
                .unwrap();
        assert!(expr.node_count() > 8); // 8 ops + 7 Seq nodes
    }

    #[test]
    fn parse_comments() {
        let expr = parse("// this is a comment\nrelu >> linear").unwrap();
        assert!(matches!(expr, Expr::Seq(_, _)));
    }

    #[test]
    fn pretty_roundtrip_pipeline() {
        let text = "relu >> linear >> softmax";
        let expr = parse(text).unwrap();
        let rendered = pretty(&expr);
        assert_eq!(rendered, text);
    }

    #[test]
    fn pretty_roundtrip_parallel() {
        let text = "relu | sigmoid";
        let expr = parse(text).unwrap();
        let rendered = pretty(&expr);
        assert_eq!(rendered, text);
    }

    #[test]
    fn pretty_roundtrip_op_with_args() {
        let text = "add(3, 4)";
        let expr = parse(text).unwrap();
        let rendered = pretty(&expr);
        assert_eq!(rendered, text);
    }

    #[test]
    fn pretty_roundtrip_literals() {
        assert_eq!(pretty(&parse("42").unwrap()), "42");
        assert_eq!(pretty(&parse("true").unwrap()), "true");
        assert_eq!(pretty(&parse("nil").unwrap()), "nil");
    }

    #[test]
    fn pretty_roundtrip_conditional() {
        let text = "if true then relu else gelu";
        let expr = parse(text).unwrap();
        let rendered = pretty(&expr);
        assert_eq!(rendered, text);
    }

    #[test]
    fn pretty_block() {
        let text = "{ relu; linear; softmax }";
        let expr = parse(text).unwrap();
        let rendered = pretty(&expr);
        assert_eq!(rendered, text);
    }

    #[test]
    fn op_from_name_roundtrip() {
        for &op in Op::ALL {
            let name = op.name();
            if name != "?" {
                assert_eq!(op_from_name(name), Some(op), "failed for op {name}");
            }
        }
    }

    #[test]
    fn parse_error_on_invalid() {
        assert!(parse("@@@").is_err());
    }

    #[test]
    fn parse_error_on_incomplete() {
        assert!(parse("add(3,").is_err());
    }

    #[test]
    fn parse_mixed_pipeline_and_parallel() {
        // | binds tighter than >>
        let expr = parse("relu | sigmoid >> linear").unwrap();
        // Should be (relu | sigmoid) >> linear
        assert!(matches!(expr, Expr::Seq(_, _)));
    }

    #[test]
    fn pretty_nested_ops() {
        let text = "add(mul(2, 3), 4)";
        let expr = parse(text).unwrap();
        let rendered = pretty(&expr);
        assert_eq!(rendered, text);
    }

    #[test]
    fn pretty_let_with_symbols() {
        let (expr, syms) = parse_with_symbols("let x = 42 in add($x, 1)").unwrap();
        let rendered = pretty_with_symbols(&expr, &syms);
        assert_eq!(rendered, "let x = 42 in add($x, 1)");
    }

    #[test]
    fn pretty_lambda_with_symbols() {
        let (expr, syms) = parse_with_symbols("\\(x: f32) -> add($x, 1.0)").unwrap();
        let rendered = pretty_with_symbols(&expr, &syms);
        assert_eq!(rendered, "\\(x: f32) -> add($x, 1.0)");
    }
}
