/// MechGen LL(1) Lexer — tokenizes MechGen canonical syntax.
///
/// Design: single-pass, no backtracking, every token is unambiguous from
/// its first character. Optimized for streaming (agent consumption).
///
/// Covers all keyword/attribute/type mappings from REDOX_PROPOSAL.md §5.5.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    // ── Declaration keywords ──────────────────────────────────────
    KwF,   // f (function)
    KwAf,  // af (async fn)
    KwUf,  // uf (unsafe fn)
    KwM,   // m (mutable binding)
    KwV,   // v (immutable binding)
    KwC,   // C (const)
    KwS,   // S (struct)
    KwE,   // E (enum)
    KwT,   // T (trait)
    KwI,   // I (impl)
    KwMod, // M (module)
    KwU,   // U (union)
    KwUse, // u (use)
    KwY,   // Y (type alias)
    KwZ,   // Z (static)

    // ── Visibility ────────────────────────────────────────────────
    Plus,     // + (pub prefix, also arithmetic)
    TildePre, // ~ prefix on keyword (pub(crate) — e.g., ~f, ~S)

    // ── Control flow ──────────────────────────────────────────────
    Question,     // ? (if / option type / try operator)
    QuestionEq,   // ?= (match)
    At,           // @ (for loop / attribute / struct literal / arc type)
    AtAt,         // @@ (loop — infinite)
    AtW,          // @w (while)
    KwRet,        // ret (return) — also `^` in expression context
    KwYield,      // yield
    DoubleArrowR, // >> (continue)
    Bang,         // ! (break, also logical NOT / assert)

    // ── Boolean ───────────────────────────────────────────────────
    True,  // 1b
    False, // 0b

    // ── Special identifiers ───────────────────────────────────────
    Underscore,  // _ (self)
    UnderscoreT, // _T (Self)

    // ── Effect / contract / spec ──────────────────────────────────
    KwEffect, // effect
    KwHandle, // handle
    KwSpec,   // spec
    KwAgent,  // agent
    KwSwarm,  // swarm
    KwExtern, // extern
    KwReq,    // @req (precondition — parsed as @ + req ident)
    KwEns,    // @ens (postcondition)
    KwInv,    // @inv (invariant)
    KwFx,     // @fx (effects)
    KwPerf,   // @perf (performance contract)

    // ── Safety (legacy mode) ──────────────────────────────────────
    KwUnsafe, // unsafe

    // ── Placeholder/todo ──────────────────────────────────────────
    Todo,          // ?? (todo!())
    Unimplemented, // ??? (unimplemented!())

    // ── Other keywords ────────────────────────────────────────────
    KwType,           // type
    KwStatic,         // static
    KwFor,            // for (in trait bounds)
    KwLoop,           // loop (legacy — canonical is @@)
    KwBreak,          // break (legacy — canonical is !)
    KwContinue,       // continue (legacy — canonical is >>)
    KwOf,             // of (human mode `for` separator — `each x of list`)
    KwOr,             // or (error-union type: T or E)
    KwElse,           // else (if cond {} else {})
    KwData,           // data (record/sum type with auto-derive)
    KwVal,            // val (immutable binding — human mode)
    KwVar,            // var (mutable binding — human mode)
    KwGuard,          // guard ... else { early-exit }
    KwDefer,          // defer expr (run on scope exit)
    KwExtend,         // extend Type { methods }
    KwIs,             // is (pattern test)
    Pipe,             // |> (pipeline operator)
    KwOk,             // Ok
    KwErr,            // Err
    KwSome,           // Some
    KwNone,           // None
    KwSwarmMapReduce, // swarm_map_reduce
    KwSwarmPipeline,  // swarm_pipeline
    KwSwarmSaga,      // swarm_saga
    KwSwarmFanOut,    // swarm_fan_out
    KwSwarmRace,      // swarm_race
    KwPipeline,       // pipeline
    KwGrammarExt,     // grammar_extension

    // ── AI / Neural keywords ──────────────────────────────────────
    KwNet,     // net
    KwLayer,   // layer
    KwTensor,  // tensor
    KwParam,   // param
    KwTrain,   // train
    KwGrad,    // grad
    KwForward, // forward

    // ── AI / Knowledge Base keywords ─────────────────────────────
    KwKb,    // kb
    KwFact,  // fact
    KwRule,  // rule
    KwQuery, // query

    // ── AI / Evolution keywords ──────────────────────────────────
    KwEvolve,      // evolve
    KwGenome,      // genome
    KwMutate,      // mutate
    KwFitness,     // fitness
    KwSelect,      // select
    KwCrossover,   // crossover
    KwPopulation,  // population
    KwGenerations, // generations

    // ── AI / Reinforcement Learning keywords ─────────────────────
    KwRl,     // rl
    KwPolicy, // policy
    KwReward, // reward

    // ── Agent-mode Greek symbols ────────────────────────────────
    KwPsi,        // Ψ → net
    KwLambda,     // λ → layer
    KwPhi,        // Φ → tensor
    KwPi,         // Π → param
    KwTheta,      // Θ → train
    KwNabla,      // ∇ → grad
    KwAlpha,      // α → agent
    KwSigma,      // Σ → swarm
    KwKappa,      // κ → kb
    KwRho,        // ρ → rule
    KwOmega,      // Ω → evolve
    KwGammaGreek, // Γ → genome
    KwPhiLower,   // φ → fitness
    KwXi,         // Ξ → policy
    KwMu,         // μ → mutate
    KwChi,        // χ → crossover

    // ── Tensor operators ─────────────────────────────────────────
    TensorMatmul,    // ⊗
    TensorHadamard,  // ⊙
    TensorTranspose, // ⊤
    TensorFlatten,   // ⊥
    TensorPipeline,  // ▸

    // ── Literals ──────────────────────────────────────────────────
    IntLiteral,
    FloatLiteral,
    StringLiteral,
    FormatString, // f"..."
    PrintString,  // p"..."
    EprintString, // ep"..."
    CharLiteral,
    ByteLiteral,
    ByteStringLiteral,

    // ── Identifiers ───────────────────────────────────────────────
    Ident,

    // ── Operators ─────────────────────────────────────────────────
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // % (also Cell type prefix)
    Eq,         // ==
    Neq,        // !=
    Lt,         // <
    Gt,         // >
    Le,         // <=
    Ge,         // >=
    And,        // &&
    Or,         // ||
    Not,        // ! (same as Bang — aliased for clarity)
    BitAnd,     // & (also reference)
    BitOr,      // |
    BitXor,     // ^  (also Box type prefix)
    Shl,        // <<
    Shr,        // >>
    Assign,     // =
    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=
    PercentEq,  // %=
    BitAndEq,   // &=
    BitOrEq,    // |=
    BitXorEq,   // ^=
    ShlEq,      // <<=
    ShrEq,      // >>=
    AndNot,     // &! (&mut T — exclusive reference)
    AndTilde,   // &~ (Cow<T>)
    PercentNot, // %! (RefCell<T>)
    HashTilde,  // #~ (RwLock<T>)

    // ── Where clause ──────────────────────────────────────────────
    TildeArrow, // ~> (where)

    // ── Delimiters ────────────────────────────────────────────────
    LParen, // (
    RParen, // )
    LBrace, // {
    RBrace, // }
    LBrack, // [
    RBrack, // ]

    // ── Punctuation ───────────────────────────────────────────────
    Semi,          // ;
    Comma,         // ,
    Dot,           // .
    Colon,         // :
    ColonQuestion, // :? (else-if)
    Arrow,         // ->
    FatArrow,      // =>
    Hash,          // # (also Mutex type prefix)
    DotDot,        // ..
    DotDotEq,      // ..=
    Tilde,         // ~ (vec suffix, also in ~>, &~, #~)
    Dollar,        // $ (Rc type prefix)

    // ── Special ───────────────────────────────────────────────────
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
        Self {
            source,
            bytes: source.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
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

    fn advance_n(&mut self, n: usize) {
        for _ in 0..n {
            self.advance();
        }
    }

    /// Check if the bytes after `@` form `keyword` and are NOT followed by
    /// an alphanumeric or `_` (i.e. it's a complete keyword, not a prefix of
    /// an identifier like `@require`).
    fn match_keyword_after_at(&self, keyword: &[u8]) -> bool {
        let start = self.pos; // self.pos is right after '@'
        if start + keyword.len() > self.bytes.len() {
            return false;
        }
        if &self.bytes[start..start + keyword.len()] != keyword {
            return false;
        }
        // The character after the keyword must NOT be alphanumeric or '_'
        let after = self.bytes.get(start + keyword.len());
        !after.is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
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
            span: Span {
                offset: start,
                len: self.pos - start,
                line: start_line,
                col: start_col,
            },
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

            b'%' if self.peek() == Some(b'!') => {
                self.advance();
                self.make_token(TokenKind::PercentNot, start, start_line, start_col)
            }
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

            b'!' if self.peek() == Some(b'=') && self.peek2() == Some(b'=') => {
                // !== (assert_eq — lexed as != followed by = in parser, but we emit Neq + Assign)
                self.advance();
                self.make_token(TokenKind::Neq, start, start_line, start_col)
            }
            b'!' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::Neq, start, start_line, start_col)
            }
            b'!' => self.make_token(TokenKind::Bang, start, start_line, start_col),

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
            b'&' if self.peek() == Some(b'~') => {
                self.advance();
                self.make_token(TokenKind::AndTilde, start, start_line, start_col)
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
            b'|' if self.peek() == Some(b'>') => {
                self.advance();
                self.make_token(TokenKind::Pipe, start, start_line, start_col)
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

            b'?' if self.peek() == Some(b'?') && self.peek2() == Some(b'?') => {
                self.advance();
                self.advance();
                self.make_token(TokenKind::Unimplemented, start, start_line, start_col)
            }
            b'?' if self.peek() == Some(b'?') => {
                self.advance();
                self.make_token(TokenKind::Todo, start, start_line, start_col)
            }
            b'?' if self.peek() == Some(b'=') => {
                self.advance();
                self.make_token(TokenKind::QuestionEq, start, start_line, start_col)
            }
            b'?' => self.make_token(TokenKind::Question, start, start_line, start_col),
            b'@' if self.peek() == Some(b'@') => {
                self.advance();
                self.make_token(TokenKind::AtAt, start, start_line, start_col)
            }
            b'@' if self.peek() == Some(b'w')
                && !self
                    .bytes
                    .get(self.pos + 1)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') =>
            {
                self.advance();
                self.make_token(TokenKind::AtW, start, start_line, start_col)
            }
            b'@' if self.match_keyword_after_at(b"req") => {
                self.advance_n(3);
                self.make_token(TokenKind::KwReq, start, start_line, start_col)
            }
            b'@' if self.match_keyword_after_at(b"ens") => {
                self.advance_n(3);
                self.make_token(TokenKind::KwEns, start, start_line, start_col)
            }
            b'@' if self.match_keyword_after_at(b"inv") => {
                self.advance_n(3);
                self.make_token(TokenKind::KwInv, start, start_line, start_col)
            }
            b'@' if self.match_keyword_after_at(b"fx") => {
                self.advance_n(2);
                self.make_token(TokenKind::KwFx, start, start_line, start_col)
            }
            b'@' if self.match_keyword_after_at(b"perf") => {
                self.advance_n(4);
                self.make_token(TokenKind::KwPerf, start, start_line, start_col)
            }
            b'@' => self.make_token(TokenKind::At, start, start_line, start_col),
            b'#' if self.peek() == Some(b'~') => {
                self.advance();
                self.make_token(TokenKind::HashTilde, start, start_line, start_col)
            }
            b'#' => self.make_token(TokenKind::Hash, start, start_line, start_col),
            b'~' if self.peek() == Some(b'>') => {
                self.advance();
                self.make_token(TokenKind::TildeArrow, start, start_line, start_col)
            }
            b'~' => self.make_token(TokenKind::Tilde, start, start_line, start_col),
            b'$' => self.make_token(TokenKind::Dollar, start, start_line, start_col),

            // ── Agent-mode Greek symbols & tensor operators (UTF-8) ──
            0xCE => self.lex_greek_ce(start, start_line, start_col),
            0xCF => self.lex_greek_cf(start, start_line, start_col),
            0xE2 => self.lex_utf8_e2(start, start_line, start_col),

            _ => self.make_token(TokenKind::Error, start, start_line, start_col),
        }
    }

    /// Lex Greek symbols starting with UTF-8 lead byte 0xCE.
    fn lex_greek_ce(&mut self, start: usize, start_line: usize, start_col: usize) -> Token {
        let kind = match self.peek() {
            Some(0x93) => {
                self.advance();
                TokenKind::KwGammaGreek
            } // Γ
            Some(0x98) => {
                self.advance();
                TokenKind::KwTheta
            } // Θ
            Some(0x9E) => {
                self.advance();
                TokenKind::KwXi
            } // Ξ
            Some(0xA0) => {
                self.advance();
                TokenKind::KwPi
            } // Π
            Some(0xA3) => {
                self.advance();
                TokenKind::KwSigma
            } // Σ
            Some(0xA6) => {
                self.advance();
                TokenKind::KwPhi
            } // Φ
            Some(0xA8) => {
                self.advance();
                TokenKind::KwPsi
            } // Ψ
            Some(0xA9) => {
                self.advance();
                TokenKind::KwOmega
            } // Ω
            Some(0xB1) => {
                self.advance();
                TokenKind::KwAlpha
            } // α
            Some(0xBA) => {
                self.advance();
                TokenKind::KwKappa
            } // κ
            Some(0xBB) => {
                self.advance();
                TokenKind::KwLambda
            } // λ
            Some(0xBC) => {
                self.advance();
                TokenKind::KwMu
            } // μ
            _ => TokenKind::Error,
        };
        self.make_token(kind, start, start_line, start_col)
    }

    /// Lex Greek symbols starting with UTF-8 lead byte 0xCF.
    fn lex_greek_cf(&mut self, start: usize, start_line: usize, start_col: usize) -> Token {
        let kind = match self.peek() {
            Some(0x81) => {
                self.advance();
                TokenKind::KwRho
            } // ρ
            Some(0x86) => {
                self.advance();
                TokenKind::KwPhiLower
            } // φ
            Some(0x87) => {
                self.advance();
                TokenKind::KwChi
            } // χ
            _ => TokenKind::Error,
        };
        self.make_token(kind, start, start_line, start_col)
    }

    /// Lex ∇, tensor operators, and ▸ starting with UTF-8 lead byte 0xE2.
    fn lex_utf8_e2(&mut self, start: usize, start_line: usize, start_col: usize) -> Token {
        let kind = match self.peek() {
            Some(0x88) => {
                self.advance();
                match self.peek() {
                    Some(0x87) => {
                        self.advance();
                        TokenKind::KwNabla
                    } // ∇ (E2 88 87)
                    _ => TokenKind::Error,
                }
            }
            Some(0x8A) => {
                self.advance();
                match self.peek() {
                    Some(0x97) => {
                        self.advance();
                        TokenKind::TensorMatmul
                    } // ⊗ (E2 8A 97)
                    Some(0x99) => {
                        self.advance();
                        TokenKind::TensorHadamard
                    } // ⊙ (E2 8A 99)
                    Some(0xA4) => {
                        self.advance();
                        TokenKind::TensorTranspose
                    } // ⊤ (E2 8A A4)
                    Some(0xA5) => {
                        self.advance();
                        TokenKind::TensorFlatten
                    } // ⊥ (E2 8A A5)
                    _ => TokenKind::Error,
                }
            }
            Some(0x96) => {
                self.advance();
                match self.peek() {
                    Some(0xB8) => {
                        self.advance();
                        TokenKind::TensorPipeline
                    } // ▸ (E2 96 B8)
                    _ => TokenKind::Error,
                }
            }
            _ => TokenKind::Error,
        };
        self.make_token(kind, start, start_line, start_col)
    }

    fn lex_string(
        &mut self,
        start: usize,
        start_line: usize,
        start_col: usize,
        kind: TokenKind,
    ) -> Token {
        // F-strings (FormatString, PrintString, EprintString) carry
        // `{...}` interpolations whose argument can itself contain
        // `"`, `,`, etc. While inside a top-level `{...}` group, the
        // outer `"` MUST NOT close the string. Track brace depth so
        // those inner tokens are passed through.
        let is_interp = matches!(
            kind,
            TokenKind::FormatString | TokenKind::PrintString | TokenKind::EprintString
        );
        let mut interp_depth: u32 = 0;
        loop {
            match self.advance() {
                Some(b'{') if is_interp => {
                    // `{{` is a literal `{` escape — skip both.
                    if self.peek() == Some(b'{') {
                        self.advance();
                    } else {
                        interp_depth += 1;
                    }
                }
                Some(b'}') if is_interp && interp_depth > 0 => {
                    interp_depth -= 1;
                }
                Some(b'"') if interp_depth == 0 => break,
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
                Some(b'b')
                    if self
                        .peek2()
                        .is_some_and(|c| c == b'0' || c == b'1' || c == b'_') =>
                {
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
                if !self
                    .peek2()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
                {
                    self.advance();
                    let kind = if text == "1" {
                        TokenKind::True
                    } else {
                        TokenKind::False
                    };
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

        // Check for format/print strings: f"...", p"...", ep"..."
        if text == "ep" && self.peek() == Some(b'"') {
            self.advance(); // consume opening quote
            return self.lex_string(start, start_line, start_col, TokenKind::EprintString);
        }
        if (text == "f" || text == "p") && self.peek() == Some(b'"') {
            self.advance(); // consume opening quote
            let kind = if text == "f" {
                TokenKind::FormatString
            } else {
                TokenKind::PrintString
            };
            return self.lex_string(start, start_line, start_col, kind);
        }

        let kind = match text {
            // ── Agent-mode single-char declaration keywords ──────
            "f" => TokenKind::KwF,
            "af" => TokenKind::KwAf,
            "uf" => TokenKind::KwUf,
            "m" => TokenKind::KwM,
            "v" => TokenKind::KwV,
            "C" => TokenKind::KwC,
            "S" => TokenKind::KwS,
            "E" => TokenKind::KwE,
            "T" => TokenKind::KwT,
            "I" => TokenKind::KwI,
            "M" => TokenKind::KwMod,
            "U" => TokenKind::KwU,
            "u" => TokenKind::KwUse,
            "Y" => TokenKind::KwY,
            "Z" => TokenKind::KwZ,
            "s" => TokenKind::Ident, // 's' is a type, not a keyword; parser handles

            // ── Human-mode keywords (Rust-compatible) ─────────────────
            "fn" => TokenKind::KwF,           // function
            "pub" => TokenKind::Plus,         // public — same as agent's +
            "let" => TokenKind::KwV,          // immutable binding (let)
            "mut" => TokenKind::KwM,          // mutable — qualifier on let/&
            "const" => TokenKind::KwC,        // const
            "struct" => TokenKind::KwS,       // struct
            "enum" => TokenKind::KwE,         // enum
            "trait" => TokenKind::KwT,        // trait
            "impl" => TokenKind::KwI,         // impl
            "mod" => TokenKind::KwMod,        // module
            "use" => TokenKind::KwUse,        // import
            "async" => TokenKind::KwAf,       // async
            "if" => TokenKind::Question,      // if
            "match" => TokenKind::QuestionEq, // match
            "for" => TokenKind::At,           // for loop
            "in" => TokenKind::KwOf,          // for separator (for x in list)
            "while" => TokenKind::AtW,        // while
            "else" => TokenKind::KwElse,      // else (if cond {} else {})
            "where" => TokenKind::TildeArrow, // where clause

            // ── Multi-char keywords (shared) ─────────────────────
            "loop" => TokenKind::KwLoop,
            "break" => TokenKind::KwBreak,
            "continue" => TokenKind::KwContinue,
            "return" => TokenKind::KwRet,
            "ret" => TokenKind::KwRet, // agent mode alias
            "yield" => TokenKind::KwYield,
            "yl" => TokenKind::KwYield, // agent mode alias
            "effect" => TokenKind::KwEffect,
            "fx" => TokenKind::KwEffect, // agent mode alias
            "handle" => TokenKind::KwHandle,
            "hx" => TokenKind::KwHandle, // agent mode alias
            "spec" => TokenKind::KwSpec,
            "sp" => TokenKind::KwSpec, // agent mode alias
            "agent" => TokenKind::KwAgent,
            "swarm" => TokenKind::KwSwarm,
            "sw" => TokenKind::KwSwarm, // agent mode alias
            "extern" => TokenKind::KwExtern,
            "xn" => TokenKind::KwExtern, // agent mode alias
            "unsafe" => TokenKind::KwUnsafe,
            "type" => TokenKind::KwType,
            "static" => TokenKind::KwStatic,

            // ── New human-mode keywords ────────────────────────────
            "data" => TokenKind::KwData,
            "val" => TokenKind::KwVal,
            "var" => TokenKind::KwVar,
            "guard" => TokenKind::KwGuard,
            "defer" => TokenKind::KwDefer,
            "extend" => TokenKind::KwExtend,
            "is" => TokenKind::KwIs,
            "or" => TokenKind::KwOr,

            // ── Agent-mode aliases for new keywords ────────────────
            "D" => TokenKind::KwData,
            "gd" => TokenKind::KwGuard,
            "df" => TokenKind::KwDefer,
            "xd" => TokenKind::KwExtend,

            // Built-in result/option variants
            "Ok" => TokenKind::KwOk,
            "Err" => TokenKind::KwErr,
            "Some" => TokenKind::KwSome,
            "None" => TokenKind::KwNone,

            // Swarm orchestration pattern keywords
            "swarm_map_reduce" => TokenKind::KwSwarmMapReduce,
            "swarm_pipeline" => TokenKind::KwSwarmPipeline,
            "swarm_saga" => TokenKind::KwSwarmSaga,
            "swarm_fan_out" => TokenKind::KwSwarmFanOut,
            "swarm_race" => TokenKind::KwSwarmRace,
            "pipeline" => TokenKind::KwPipeline,
            "grammar_extension" => TokenKind::KwGrammarExt,

            // AI / Neural keywords
            "net" => TokenKind::KwNet,
            "layer" => TokenKind::KwLayer,
            "tensor" => TokenKind::KwTensor,
            "param" => TokenKind::KwParam,
            "train" => TokenKind::KwTrain,
            "grad" => TokenKind::KwGrad,
            "forward" => TokenKind::KwForward,

            // AI / Knowledge Base keywords
            "kb" => TokenKind::KwKb,
            "fact" => TokenKind::KwFact,
            "rule" => TokenKind::KwRule,
            "query" => TokenKind::KwQuery,

            // AI / Evolution keywords
            "evolve" => TokenKind::KwEvolve,
            "genome" => TokenKind::KwGenome,
            "mutate" => TokenKind::KwMutate,
            "fitness" => TokenKind::KwFitness,
            "select" => TokenKind::KwSelect,
            "crossover" => TokenKind::KwCrossover,
            "population" => TokenKind::KwPopulation,
            "generations" => TokenKind::KwGenerations,

            // AI / Reinforcement Learning keywords
            "rl" => TokenKind::KwRl,
            "policy" => TokenKind::KwPolicy,
            "reward" => TokenKind::KwReward,

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
    fn test_match_token() {
        let tokens = lex("?= x { 1 => a, _ => b, }");
        assert_eq!(tokens[0].kind, TokenKind::QuestionEq);
    }

    #[test]
    fn test_loop_token() {
        let tokens = lex("@@ { ! }");
        assert_eq!(tokens[0].kind, TokenKind::AtAt);
        assert_eq!(tokens[2].kind, TokenKind::Bang);
    }

    #[test]
    fn test_todo_unimplemented() {
        let tokens = lex("?? ???");
        assert_eq!(tokens[0].kind, TokenKind::Todo);
        assert_eq!(tokens[1].kind, TokenKind::Unimplemented);
    }

    #[test]
    fn test_async_unsafe_fn() {
        let tokens = lex("+af handle() -> R[(), Error]");
        assert_eq!(tokens[0].kind, TokenKind::Plus);
        assert_eq!(tokens[1].kind, TokenKind::KwAf);
    }

    #[test]
    fn test_where_clause() {
        let tokens = lex("~> T: Cl");
        assert_eq!(tokens[0].kind, TokenKind::TildeArrow);
    }

    #[test]
    fn test_type_alias_static() {
        let tokens = lex("Y Alias = i32; Z GLOBAL: u8 = 0;");
        assert_eq!(tokens[0].kind, TokenKind::KwY);
        assert_eq!(tokens[5].kind, TokenKind::KwZ);
    }

    #[test]
    fn test_eprint_string() {
        let tokens = lex(r#"ep"error: {msg}""#);
        assert_eq!(tokens[0].kind, TokenKind::EprintString);
    }

    #[test]
    fn test_smart_pointer_types() {
        // &~ (Cow), %! (RefCell), #~ (RwLock)
        let tokens = lex("&~T %!T #~T");
        assert_eq!(tokens[0].kind, TokenKind::AndTilde);
        assert_eq!(tokens[2].kind, TokenKind::PercentNot);
        assert_eq!(tokens[4].kind, TokenKind::HashTilde);
    }

    #[test]
    fn test_swarm_keywords() {
        let tokens = lex("swarm_map_reduce { }");
        assert_eq!(tokens[0].kind, TokenKind::KwSwarmMapReduce);
    }

    #[test]
    fn test_result_option_variants() {
        let tokens = lex("Ok(x) Err(e) Some(v) None");
        assert_eq!(tokens[0].kind, TokenKind::KwOk);
        assert_eq!(tokens[4].kind, TokenKind::KwErr);
        assert_eq!(tokens[8].kind, TokenKind::KwSome);
        assert_eq!(tokens[12].kind, TokenKind::KwNone);
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

    #[test]
    fn test_contract_tokens_req_ens_inv() {
        let tokens = lex("@req(n > 0) @ens(result > 0) @inv(_.len <= _.cap)");
        assert_eq!(tokens[0].kind, TokenKind::KwReq);
        assert_eq!(tokens[0].text, "@req");
        // @req(n > 0) = tokens 0..=5 (6 tokens), @ens at index 6
        assert_eq!(tokens[6].kind, TokenKind::KwEns);
        assert_eq!(tokens[6].text, "@ens");
        // @ens(result > 0) = tokens 6..=11 (6 tokens), @inv at index 12
        assert_eq!(tokens[12].kind, TokenKind::KwInv);
        assert_eq!(tokens[12].text, "@inv");
    }

    #[test]
    fn test_contract_tokens_fx_perf() {
        let tokens = lex("@fx(io) @perf(O(1))");
        assert_eq!(tokens[0].kind, TokenKind::KwFx);
        assert_eq!(tokens[0].text, "@fx");
        assert_eq!(tokens[4].kind, TokenKind::KwPerf);
        assert_eq!(tokens[4].text, "@perf");
    }

    #[test]
    fn test_at_ident_not_contract() {
        // @require is NOT @req + "uire" — it's @ + Ident("require")
        let tokens = lex("@require");
        assert_eq!(tokens[0].kind, TokenKind::At);
        assert_eq!(tokens[1].kind, TokenKind::Ident);
        assert_eq!(tokens[1].text, "require");
    }

    #[test]
    fn test_agent_keyword() {
        let tokens = lex("agent");
        assert_eq!(tokens[0].kind, TokenKind::KwAgent);
        assert_eq!(tokens[0].text, "agent");
    }

    #[test]
    fn test_ai_neural_keywords() {
        let tokens = lex("net layer tensor param train grad forward");
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Eof)
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwNet,
                TokenKind::KwLayer,
                TokenKind::KwTensor,
                TokenKind::KwParam,
                TokenKind::KwTrain,
                TokenKind::KwGrad,
                TokenKind::KwForward,
            ]
        );
    }

    #[test]
    fn test_ai_kb_keywords() {
        let tokens = lex("kb fact rule query");
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Eof)
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwKb,
                TokenKind::KwFact,
                TokenKind::KwRule,
                TokenKind::KwQuery,
            ]
        );
    }

    #[test]
    fn test_ai_evolve_keywords() {
        let tokens = lex("evolve genome mutate fitness select crossover population generations");
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Eof)
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwEvolve,
                TokenKind::KwGenome,
                TokenKind::KwMutate,
                TokenKind::KwFitness,
                TokenKind::KwSelect,
                TokenKind::KwCrossover,
                TokenKind::KwPopulation,
                TokenKind::KwGenerations,
            ]
        );
    }

    #[test]
    fn test_ai_rl_keywords() {
        let tokens = lex("rl policy reward");
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Eof)
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![TokenKind::KwRl, TokenKind::KwPolicy, TokenKind::KwReward]
        );
    }

    #[test]
    fn test_greek_symbols() {
        let tokens = lex("Ψ λ Φ Π Θ ∇ α κ Ω Γ Ξ μ ρ φ χ");
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Eof)
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::KwPsi,
                TokenKind::KwLambda,
                TokenKind::KwPhi,
                TokenKind::KwPi,
                TokenKind::KwTheta,
                TokenKind::KwNabla,
                TokenKind::KwAlpha,
                TokenKind::KwKappa,
                TokenKind::KwOmega,
                TokenKind::KwGammaGreek,
                TokenKind::KwXi,
                TokenKind::KwMu,
                TokenKind::KwRho,
                TokenKind::KwPhiLower,
                TokenKind::KwChi,
            ]
        );
    }

    #[test]
    fn test_tensor_operators() {
        let tokens = lex("⊗ ⊙ ⊤ ⊥ ▸");
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Eof)
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::TensorMatmul,
                TokenKind::TensorHadamard,
                TokenKind::TensorTranspose,
                TokenKind::TensorFlatten,
                TokenKind::TensorPipeline,
            ]
        );
    }

    #[test]
    fn test_net_def_tokens() {
        let tokens = lex("net MyModel { layer conv1 }");
        let kinds: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Eof)
            .map(|t| t.kind)
            .collect();
        assert_eq!(kinds[0], TokenKind::KwNet);
        assert_eq!(kinds[1], TokenKind::Ident); // MyModel
        assert_eq!(kinds[2], TokenKind::LBrace);
        assert_eq!(kinds[3], TokenKind::KwLayer);
    }
}
