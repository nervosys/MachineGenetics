/// Redox LL(1) Parser — recursive descent, zero backtracking.
///
/// Parses the Redox canonical syntax into an AST.
/// Every decision point uses a single token of lookahead.
use crate::ast::*;
use crate::lexer::{Token, TokenKind};

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

pub fn parse(tokens: &[Token]) -> Result<Module, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_module()
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> TokenKind {
        self.tokens.get(self.pos).map_or(TokenKind::Eof, |t| t.kind)
    }

    fn peek_text(&self) -> &str {
        self.tokens.get(self.pos).map_or("", |t| t.text.as_str())
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, kind: TokenKind) -> Result<&Token, ParseError> {
        if self.peek() == kind {
            Ok(self.advance())
        } else {
            let tok = self.current();
            Err(ParseError {
                line: tok.span.line,
                col: tok.span.col,
                message: format!("expected {:?}, found {:?} '{}'", kind, tok.kind, tok.text),
            })
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        // Single-char keywords (f, m, v, c, S, E, T, I, M, U, u) can appear as
        // identifiers in contexts like generic params, field names, etc.
        match self.peek() {
            TokenKind::Ident
            | TokenKind::KwF
            | TokenKind::KwM
            | TokenKind::KwV
            | TokenKind::KwC
            | TokenKind::KwS
            | TokenKind::KwE
            | TokenKind::KwT
            | TokenKind::KwI
            | TokenKind::KwMod
            | TokenKind::KwU
            | TokenKind::KwUse => Ok(self.advance().text.clone()),
            _ => {
                let tok = self.current();
                Err(ParseError {
                    line: tok.span.line,
                    col: tok.span.col,
                    message: format!("expected identifier, found {:?} '{}'", tok.kind, tok.text),
                })
            }
        }
    }

    fn error(&self, message: &str) -> ParseError {
        let tok = self.current();
        ParseError { line: tok.span.line, col: tok.span.col, message: message.to_string() }
    }

    // ── Module ──────────────────────────────────────────────

    fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut items = Vec::new();
        while self.peek() != TokenKind::Eof {
            items.push(self.parse_item()?);
        }
        Ok(Module { items })
    }

    // ── Item ────────────────────────────────────────────────

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let mut attributes = Vec::new();
        while self.peek() == TokenKind::At {
            attributes.push(self.parse_attribute()?);
        }

        let (visibility, kind) = match self.peek() {
            TokenKind::Plus => {
                self.advance();
                (Visibility::Public, self.parse_item_kind()?)
            }
            _ => (Visibility::Private, self.parse_item_kind()?),
        };

        Ok(Item { visibility, attributes, kind })
    }

    fn parse_item_kind(&mut self) -> Result<ItemKind, ParseError> {
        match self.peek() {
            TokenKind::KwF => self.parse_function_def().map(ItemKind::Function),
            TokenKind::KwS => self.parse_struct_def().map(ItemKind::Struct),
            TokenKind::KwE => self.parse_enum_def().map(ItemKind::Enum),
            TokenKind::KwT => self.parse_trait_def().map(ItemKind::Trait),
            TokenKind::KwI => self.parse_impl_block().map(ItemKind::Impl),
            TokenKind::KwMod => self.parse_module_def().map(ItemKind::Module),
            TokenKind::KwUse => self.parse_use_decl().map(ItemKind::Use),
            TokenKind::KwType => self.parse_type_alias().map(ItemKind::TypeAlias),
            TokenKind::KwC => self.parse_const_def().map(ItemKind::Const),
            TokenKind::KwEffect => self.parse_effect_def().map(ItemKind::Effect),
            TokenKind::KwSpec => self.parse_spec_def().map(ItemKind::Spec),
            _ => Err(self.error(&format!("expected item, found {:?}", self.peek()))),
        }
    }

    // ── Attribute ───────────────────────────────────────────

    fn parse_attribute(&mut self) -> Result<Attribute, ParseError> {
        self.expect(TokenKind::At)?;
        let name = self.expect_ident()?;

        let mut bang = false;
        if self.peek() == TokenKind::Bang {
            self.advance();
            bang = true;
        }

        let mut args = Vec::new();
        if self.peek() == TokenKind::LParen {
            self.advance();
            while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                let tok = self.advance();
                if tok.kind != TokenKind::Comma {
                    args.push(tok.text.clone());
                }
            }
            self.expect(TokenKind::RParen)?;
        }

        Ok(Attribute { name, args, bang })
    }

    // ── Function ────────────────────────────────────────────

    fn parse_function_def(&mut self) -> Result<FunctionDef, ParseError> {
        self.expect(TokenKind::KwF)?;
        let name = self.expect_ident()?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen)?;

        let return_type = if self.peek() == TokenKind::Arrow {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;

        Ok(FunctionDef { name, generics, params, return_type, effects: Vec::new(), body })
    }

    // ── Struct ──────────────────────────────────────────────

    fn parse_struct_def(&mut self) -> Result<StructDef, ParseError> {
        self.expect(TokenKind::KwS)?;
        let name = self.expect_ident()?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            let vis = if self.peek() == TokenKind::Plus {
                self.advance();
                Visibility::Public
            } else {
                Visibility::Private
            };
            let field_name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            if self.peek() == TokenKind::Comma {
                self.advance();
            }
            fields.push(StructField { visibility: vis, name: field_name, ty });
        }
        self.expect(TokenKind::RBrace)?;

        Ok(StructDef { name, generics, fields })
    }

    // ── Enum ────────────────────────────────────────────────

    fn parse_enum_def(&mut self) -> Result<EnumDef, ParseError> {
        self.expect(TokenKind::KwE)?;
        let name = self.expect_ident()?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            let variant_name = self.expect_ident()?;
            let kind = match self.peek() {
                TokenKind::LParen => {
                    self.advance();
                    let mut types = Vec::new();
                    while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                        types.push(self.parse_type()?);
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    VariantKind::Tuple(types)
                }
                TokenKind::LBrace => {
                    self.advance();
                    let mut fields = Vec::new();
                    while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
                        let fname = self.expect_ident()?;
                        self.expect(TokenKind::Colon)?;
                        let ty = self.parse_type()?;
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                        fields.push(StructField {
                            visibility: Visibility::Private,
                            name: fname,
                            ty,
                        });
                    }
                    self.expect(TokenKind::RBrace)?;
                    VariantKind::Struct(fields)
                }
                _ => VariantKind::Unit,
            };
            if self.peek() == TokenKind::Comma {
                self.advance();
            }
            variants.push(EnumVariant { name: variant_name, kind });
        }
        self.expect(TokenKind::RBrace)?;

        Ok(EnumDef { name, generics, variants })
    }

    // ── Trait ────────────────────────────────────────────────

    fn parse_trait_def(&mut self) -> Result<TraitDef, ParseError> {
        self.expect(TokenKind::KwT)?;
        let name = self.expect_ident()?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        let mut super_traits = Vec::new();
        if self.peek() == TokenKind::Colon {
            self.advance();
            loop {
                super_traits.push(self.expect_ident()?);
                if self.peek() == TokenKind::Plus {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            items.push(self.parse_item()?);
        }
        self.expect(TokenKind::RBrace)?;

        Ok(TraitDef { name, generics, super_traits, items })
    }

    // ── Impl ────────────────────────────────────────────────

    fn parse_impl_block(&mut self) -> Result<ImplBlock, ParseError> {
        self.expect(TokenKind::KwI)?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        let self_type = self.parse_type()?;

        let trait_path = if self.peek() == TokenKind::KwFor {
            self.advance();
            let _actual_type = self.parse_type()?;
            // The "self_type" was actually the trait path
            if let Type::Path { segments, .. } = &self_type { Some(segments.clone()) } else { None }
        } else {
            None
        };

        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            items.push(self.parse_item()?);
        }
        self.expect(TokenKind::RBrace)?;

        Ok(ImplBlock { generics, self_type, trait_path, items })
    }

    // ── Module Def ──────────────────────────────────────────

    fn parse_module_def(&mut self) -> Result<ModuleDef, ParseError> {
        self.expect(TokenKind::KwMod)?;
        let name = self.expect_ident()?;

        if self.peek() == TokenKind::Semi {
            self.advance();
            Ok(ModuleDef { name, items: None })
        } else {
            self.expect(TokenKind::LBrace)?;
            let mut items = Vec::new();
            while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
                items.push(self.parse_item()?);
            }
            self.expect(TokenKind::RBrace)?;
            Ok(ModuleDef { name, items: Some(items) })
        }
    }

    // ── Use ─────────────────────────────────────────────────

    fn parse_use_decl(&mut self) -> Result<UseDef, ParseError> {
        self.expect(TokenKind::KwUse)?;
        let mut path = vec![self.expect_ident()?];

        while self.peek() == TokenKind::Dot {
            self.advance();
            if self.peek() == TokenKind::Star {
                self.advance();
                self.expect(TokenKind::Semi)?;
                return Ok(UseDef { path, alias: None, glob: true, group: Vec::new() });
            }
            if self.peek() == TokenKind::LBrace {
                self.advance();
                let mut group = Vec::new();
                while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
                    let name = self.expect_ident()?;
                    let alias = if self.peek_text() == "as" {
                        self.advance();
                        Some(self.expect_ident()?)
                    } else {
                        None
                    };
                    group.push(UseDef { path: vec![name], alias, glob: false, group: Vec::new() });
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBrace)?;
                self.expect(TokenKind::Semi)?;
                return Ok(UseDef { path, alias: None, glob: false, group });
            }
            path.push(self.expect_ident()?);
        }

        self.expect(TokenKind::Semi)?;
        Ok(UseDef { path, alias: None, glob: false, group: Vec::new() })
    }

    // ── Type Alias ──────────────────────────────────────────

    fn parse_type_alias(&mut self) -> Result<TypeAlias, ParseError> {
        self.expect(TokenKind::KwType)?;
        let name = self.expect_ident()?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::Assign)?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Semi)?;

        Ok(TypeAlias { name, generics, ty })
    }

    // ── Const ───────────────────────────────────────────────

    fn parse_const_def(&mut self) -> Result<ConstDef, ParseError> {
        self.expect(TokenKind::KwC)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Assign)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semi)?;

        Ok(ConstDef { name, ty, value })
    }

    // ── Effect ──────────────────────────────────────────────

    fn parse_effect_def(&mut self) -> Result<EffectDef, ParseError> {
        self.expect(TokenKind::KwEffect)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut operations = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            self.expect(TokenKind::KwF)?;
            let op_name = self.expect_ident()?;
            self.expect(TokenKind::LParen)?;
            let params = self.parse_param_list()?;
            self.expect(TokenKind::RParen)?;
            let return_type = if self.peek() == TokenKind::Arrow {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            self.expect(TokenKind::Semi)?;
            operations.push(EffectOp { name: op_name, params, return_type });
        }

        self.expect(TokenKind::RBrace)?;
        Ok(EffectDef { name, operations })
    }

    // ── Spec ────────────────────────────────────────────────

    fn parse_spec_def(&mut self) -> Result<SpecDef, ParseError> {
        self.expect(TokenKind::KwSpec)?;
        let name = self.expect_ident()?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;
        let items = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            // For now, skip spec items — they require expression parsing in attributes
            let tok = self.advance();
            if tok.kind == TokenKind::Semi {
                continue;
            }
            // Simplified: just collect text until semicolon
        }
        self.expect(TokenKind::RBrace)?;

        Ok(SpecDef { name, generics, items })
    }

    // ── Generic Params ──────────────────────────────────────

    fn parse_generic_params(&mut self) -> Result<Vec<GenericParam>, ParseError> {
        self.expect(TokenKind::LBrack)?;
        let mut params = Vec::new();

        while self.peek() != TokenKind::RBrack && self.peek() != TokenKind::Eof {
            let name = self.expect_ident()?;
            let mut bounds = Vec::new();

            if self.peek() == TokenKind::Colon {
                self.advance();
                loop {
                    bounds.push(self.expect_ident()?);
                    if self.peek() == TokenKind::Plus {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }

            let default = if self.peek() == TokenKind::Assign {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };

            params.push(GenericParam { name, bounds, default });

            if self.peek() == TokenKind::Comma {
                self.advance();
            }
        }

        self.expect(TokenKind::RBrack)?;
        Ok(params)
    }

    // ── Param List ──────────────────────────────────────────

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();

        while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });

            if self.peek() == TokenKind::Comma {
                self.advance();
            }
        }

        Ok(params)
    }

    // ── Type ────────────────────────────────────────────────

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        match self.peek() {
            // &T or &!T
            TokenKind::BitAnd => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Reference { mutable: false, inner: Box::new(inner) })
            }
            TokenKind::AndNot => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Reference { mutable: true, inner: Box::new(inner) })
            }

            // ^T (owned ptr)
            TokenKind::BitXor => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::OwnedPtr { inner: Box::new(inner) })
            }

            // $T (Rc)
            TokenKind::Dollar => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Rc { inner: Box::new(inner) })
            }

            // ?T (Option)
            TokenKind::Question => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Option { inner: Box::new(inner) })
            }

            // ! (never)
            TokenKind::Bang => {
                self.advance();
                Ok(Type::Never)
            }

            // _ (inferred)
            TokenKind::Underscore => {
                self.advance();
                Ok(Type::Inferred)
            }

            // _T (Self)
            TokenKind::UnderscoreT => {
                self.advance();
                Ok(Type::SelfType)
            }

            // [T], [T; N], [T]~
            TokenKind::LBrack => {
                self.advance();
                let inner = self.parse_type()?;

                if self.peek() == TokenKind::Semi {
                    // [T; N]
                    self.advance();
                    let size = self.parse_expr()?;
                    self.expect(TokenKind::RBrack)?;
                    Ok(Type::Array { inner: Box::new(inner), size: Box::new(size) })
                } else {
                    self.expect(TokenKind::RBrack)?;
                    if self.peek() == TokenKind::Tilde {
                        self.advance();
                        Ok(Type::Vec { inner: Box::new(inner) })
                    } else {
                        Ok(Type::Slice { inner: Box::new(inner) })
                    }
                }
            }

            // {K: V} (map)
            TokenKind::LBrace => {
                self.advance();
                let key = self.parse_type()?;
                self.expect(TokenKind::Colon)?;
                let value = self.parse_type()?;
                self.expect(TokenKind::RBrace)?;
                Ok(Type::Map { key: Box::new(key), value: Box::new(value) })
            }

            // (T, T, ...) tuple
            TokenKind::LParen => {
                self.advance();
                let mut elements = Vec::new();
                while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                    elements.push(self.parse_type()?);
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(Type::Tuple { elements })
            }

            // Named types: Ident, R[T, E], Ptr[T], Simd[T, N]
            // Single-char keywords (T, S, E, etc.) can also be type names
            TokenKind::Ident
            | TokenKind::KwS
            | TokenKind::KwE
            | TokenKind::KwT
            | TokenKind::KwI
            | TokenKind::KwM
            | TokenKind::KwU
            | TokenKind::KwC => {
                let name = self.advance().text.clone();

                match name.as_str() {
                    "R" if self.peek() == TokenKind::LBrack => {
                        self.advance();
                        let ok = self.parse_type()?;
                        self.expect(TokenKind::Comma)?;
                        let err = self.parse_type()?;
                        self.expect(TokenKind::RBrack)?;
                        Ok(Type::Result { ok: Box::new(ok), err: Box::new(err) })
                    }
                    "Ptr" if self.peek() == TokenKind::LBrack => {
                        self.advance();
                        let inner = self.parse_type()?;
                        self.expect(TokenKind::RBrack)?;
                        Ok(Type::Ptr { inner: Box::new(inner) })
                    }
                    "Simd" if self.peek() == TokenKind::LBrack => {
                        self.advance();
                        let inner = self.parse_type()?;
                        self.expect(TokenKind::Comma)?;
                        let width_tok = self.expect(TokenKind::IntLiteral)?;
                        let width: u64 = width_tok.text.parse().unwrap_or(0);
                        self.expect(TokenKind::RBrack)?;
                        Ok(Type::Simd { inner: Box::new(inner), width })
                    }
                    "s" => Ok(Type::StringType),
                    _ => {
                        let mut segments = vec![name];
                        // Check for dotted path: Foo.Bar.Baz
                        while self.peek() == TokenKind::Dot {
                            // Peek ahead to check if this is a type path or field access
                            if self.tokens.get(self.pos + 1).is_some_and(|t| {
                                t.kind == TokenKind::Ident
                                    && t.text.chars().next().is_some_and(|c| c.is_uppercase())
                            }) {
                                self.advance(); // consume dot
                                segments.push(self.advance().text.clone());
                            } else {
                                break;
                            }
                        }

                        let type_args = if self.peek() == TokenKind::LBrack {
                            self.advance();
                            let mut args = Vec::new();
                            while self.peek() != TokenKind::RBrack && self.peek() != TokenKind::Eof
                            {
                                args.push(self.parse_type()?);
                                if self.peek() == TokenKind::Comma {
                                    self.advance();
                                }
                            }
                            self.expect(TokenKind::RBrack)?;
                            args
                        } else {
                            Vec::new()
                        };

                        Ok(Type::Path { segments, type_args })
                    }
                }
            }

            // Single-char type keywords used as identifiers
            TokenKind::KwF if self.peek() == TokenKind::LParen => {
                // fn type: f(T, T) -> T
                self.advance();
                self.advance(); // (
                let mut params = Vec::new();
                while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                    params.push(self.parse_type()?);
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RParen)?;
                let ret = if self.peek() == TokenKind::Arrow {
                    self.advance();
                    Some(Box::new(self.parse_type()?))
                } else {
                    None
                };
                Ok(Type::Fn { params, ret })
            }

            _ => Err(self.error(&format!("expected type, found {:?}", self.peek()))),
        }
    }

    // ── Block ───────────────────────────────────────────────

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        let mut tail_expr = None;

        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            // Try to parse a statement
            match self.peek() {
                TokenKind::KwV | TokenKind::KwM if self.is_let_statement() => {
                    stmts.push(self.parse_let_stmt()?);
                }
                _ => {
                    let expr = self.parse_expr()?;
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                        stmts.push(Stmt::Expr { expr });
                    } else if self.peek() == TokenKind::RBrace {
                        tail_expr = Some(Box::new(expr));
                    } else {
                        stmts.push(Stmt::Expr { expr });
                    }
                }
            }
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Block { stmts, tail_expr })
    }

    fn is_let_statement(&self) -> bool {
        // v or m followed by an identifier
        matches!(self.peek(), TokenKind::KwV | TokenKind::KwM)
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.kind == TokenKind::Ident || t.kind == TokenKind::Underscore)
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let mutable = self.peek() == TokenKind::KwM;
        self.advance(); // consume v or m

        let pattern = self.parse_pattern()?;

        let ty = if self.peek() == TokenKind::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(TokenKind::Assign)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semi)?;

        Ok(Stmt::Let { mutable, pattern, ty, value })
    }

    // ── Expression (Pratt Parsing) ──────────────────────────

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_prefix_expr()?;

        loop {
            // Postfix: ?, .field, .method(), [index], ()
            match self.peek() {
                TokenKind::Question => {
                    self.advance();
                    lhs = Expr::Try { expr: Box::new(lhs) };
                    continue;
                }
                TokenKind::Dot => {
                    self.advance();
                    let field = self.expect_ident()?;
                    if self.peek() == TokenKind::LParen {
                        let type_args = Vec::new();
                        self.advance();
                        let mut args = Vec::new();
                        while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                            args.push(self.parse_expr()?);
                            if self.peek() == TokenKind::Comma {
                                self.advance();
                            }
                        }
                        self.expect(TokenKind::RParen)?;
                        lhs = Expr::MethodCall {
                            receiver: Box::new(lhs),
                            method: field,
                            type_args,
                            args,
                        };
                    } else {
                        lhs = Expr::FieldAccess { object: Box::new(lhs), field };
                    }
                    continue;
                }
                TokenKind::LBrack => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(TokenKind::RBrack)?;
                    lhs = Expr::Index { object: Box::new(lhs), index: Box::new(index) };
                    continue;
                }
                TokenKind::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                        args.push(self.parse_expr()?);
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    lhs = Expr::Call { func: Box::new(lhs), args };
                    continue;
                }
                _ => {}
            }

            // Infix operators
            let (op, l_bp, r_bp) = match self.peek() {
                TokenKind::Assign => ("=", 1, 2),
                TokenKind::PlusEq => ("+=", 1, 2),
                TokenKind::MinusEq => ("-=", 1, 2),
                TokenKind::StarEq => ("*=", 1, 2),
                TokenKind::SlashEq => ("/=", 1, 2),
                TokenKind::PercentEq => ("%=", 1, 2),
                TokenKind::Or => ("||", 3, 4),
                TokenKind::And => ("&&", 5, 6),
                TokenKind::Eq => ("==", 7, 8),
                TokenKind::Neq => ("!=", 7, 8),
                TokenKind::Lt => ("<", 9, 10),
                TokenKind::Gt => (">", 9, 10),
                TokenKind::Le => ("<=", 9, 10),
                TokenKind::Ge => (">=", 9, 10),
                TokenKind::BitOr => ("|", 11, 12),
                TokenKind::BitXor => ("^", 13, 14),
                TokenKind::BitAnd => ("&", 15, 16),
                TokenKind::Shl => ("<<", 17, 18),
                TokenKind::Shr => (">>", 17, 18),
                TokenKind::Plus => ("+", 19, 20),
                TokenKind::Minus => ("-", 19, 20),
                TokenKind::Star => ("*", 21, 22),
                TokenKind::Slash => ("/", 21, 22),
                TokenKind::Percent => ("%", 21, 22),
                _ => break,
            };

            if l_bp < min_bp {
                break;
            }

            self.advance();

            // Handle assignment
            if op == "=" {
                let rhs = self.parse_expr_bp(r_bp)?;
                lhs = Expr::Assign { target: Box::new(lhs), value: Box::new(rhs) };
                continue;
            }

            let rhs = self.parse_expr_bp(r_bp)?;
            lhs = Expr::Binary { op: op.to_string(), left: Box::new(lhs), right: Box::new(rhs) };
        }

        Ok(lhs)
    }

    fn parse_prefix_expr(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            // Unary operators
            TokenKind::Minus | TokenKind::Bang | TokenKind::Star => {
                let tok = self.advance();
                let op = tok.text.clone();
                let operand = self.parse_prefix_expr()?;
                Ok(Expr::Unary { op, operand: Box::new(operand) })
            }
            TokenKind::BitAnd => {
                let tok = self.advance();
                let op = tok.text.clone();
                let operand = self.parse_prefix_expr()?;
                Ok(Expr::Unary { op, operand: Box::new(operand) })
            }

            // Return
            TokenKind::KwRet => {
                self.advance();
                if self.peek() == TokenKind::Semi || self.peek() == TokenKind::RBrace {
                    Ok(Expr::Return { value: None })
                } else {
                    let val = self.parse_expr()?;
                    Ok(Expr::Return { value: Some(Box::new(val)) })
                }
            }

            // Break
            TokenKind::KwBreak => {
                self.advance();
                if self.peek() == TokenKind::Semi || self.peek() == TokenKind::RBrace {
                    Ok(Expr::Break { value: None })
                } else {
                    let val = self.parse_expr()?;
                    Ok(Expr::Break { value: Some(Box::new(val)) })
                }
            }

            // Continue
            TokenKind::KwContinue => {
                self.advance();
                Ok(Expr::Continue)
            }

            // If: ? expr { ... } : { ... } or ? { match_arm, ... }
            TokenKind::Question => {
                self.advance();
                if self.peek() == TokenKind::LBrace {
                    // Match expression: ? { pat => expr, ... }
                    self.advance();
                    let mut arms = Vec::new();
                    while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
                        let pattern = self.parse_pattern()?;
                        self.expect(TokenKind::FatArrow)?;
                        let body = self.parse_expr()?;
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                        arms.push(MatchArm { pattern, body });
                    }
                    self.expect(TokenKind::RBrace)?;
                    Ok(Expr::Match { arms })
                } else {
                    // If expression: ? cond { ... } : { ... }
                    let cond = self.parse_expr()?;
                    let then_block = self.parse_block()?;
                    let else_block = if self.peek() == TokenKind::Colon {
                        self.advance();
                        Some(self.parse_block()?)
                    } else {
                        None
                    };
                    Ok(Expr::If { cond: Box::new(cond), then_block, else_block })
                }
            }

            // For loop: @ pattern : iter { ... }
            TokenKind::At => {
                self.advance();
                // Check if this is a struct literal (@Type { ... }) or for loop
                // For loop has pattern : iterable { body }
                let pattern = self.parse_pattern()?;
                self.expect(TokenKind::Colon)?;
                let iter = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Expr::For { pattern, iter: Box::new(iter), body })
            }

            // Loop
            TokenKind::KwLoop => {
                self.advance();
                let body = self.parse_block()?;
                Ok(Expr::Loop { body })
            }

            // Block expression
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Expr::Block { block })
            }

            // Closure: fn(params) => expr
            TokenKind::KwF
                if matches!(
                    self.tokens.get(self.pos + 1).map(|t| t.kind),
                    Some(TokenKind::LParen)
                ) =>
            {
                // Only treat as closure if not followed by identifier (which would be a function def)
                self.advance(); // fn
                self.advance(); // (
                let params = self.parse_param_list()?;
                self.expect(TokenKind::RParen)?;
                self.expect(TokenKind::FatArrow)?;
                let body = self.parse_expr()?;
                Ok(Expr::Closure { params, body: Box::new(body) })
            }

            // Array literal
            TokenKind::LBrack => {
                self.advance();
                if self.peek() == TokenKind::RBrack {
                    self.advance();
                    return Ok(Expr::ArrayLit { elements: Vec::new() });
                }
                let first = self.parse_expr()?;
                if self.peek() == TokenKind::Semi {
                    // Array repeat: [expr; count]
                    self.advance();
                    let count = self.parse_expr()?;
                    self.expect(TokenKind::RBrack)?;
                    Ok(Expr::ArrayRepeat { value: Box::new(first), count: Box::new(count) })
                } else {
                    let mut elements = vec![first];
                    while self.peek() == TokenKind::Comma {
                        self.advance();
                        if self.peek() == TokenKind::RBrack {
                            break;
                        }
                        elements.push(self.parse_expr()?);
                    }
                    self.expect(TokenKind::RBrack)?;
                    Ok(Expr::ArrayLit { elements })
                }
            }

            // Tuple/Paren
            TokenKind::LParen => {
                self.advance();
                if self.peek() == TokenKind::RParen {
                    self.advance();
                    return Ok(Expr::TupleLit { elements: Vec::new() });
                }
                let first = self.parse_expr()?;
                if self.peek() == TokenKind::Comma {
                    // Tuple
                    let mut elements = vec![first];
                    while self.peek() == TokenKind::Comma {
                        self.advance();
                        if self.peek() == TokenKind::RParen {
                            break;
                        }
                        elements.push(self.parse_expr()?);
                    }
                    self.expect(TokenKind::RParen)?;
                    Ok(Expr::TupleLit { elements })
                } else {
                    // Parenthesized expression
                    self.expect(TokenKind::RParen)?;
                    Ok(first)
                }
            }

            // Literals
            TokenKind::IntLiteral | TokenKind::FloatLiteral => {
                let tok = self.advance();
                let kind = if tok.kind == TokenKind::IntLiteral {
                    LiteralKind::Int
                } else {
                    LiteralKind::Float
                };
                Ok(Expr::Literal { value: tok.text.clone(), kind })
            }
            TokenKind::StringLiteral | TokenKind::FormatString | TokenKind::PrintString => {
                let tok = self.advance();
                let kind = match tok.kind {
                    TokenKind::FormatString => LiteralKind::FormatString,
                    _ => LiteralKind::String,
                };
                Ok(Expr::Literal { value: tok.text.clone(), kind })
            }
            TokenKind::CharLiteral => {
                let tok = self.advance();
                Ok(Expr::Literal { value: tok.text.clone(), kind: LiteralKind::Char })
            }
            TokenKind::True | TokenKind::False => {
                let tok = self.advance();
                Ok(Expr::Literal { value: tok.text.clone(), kind: LiteralKind::Bool })
            }

            // Identifiers
            TokenKind::Ident => {
                let tok = self.advance();
                Ok(Expr::Ident { name: tok.text.clone() })
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(Expr::Ident { name: "_".to_string() })
            }
            TokenKind::UnderscoreT => {
                self.advance();
                Ok(Expr::Ident { name: "_T".to_string() })
            }

            _ => Err(self.error(&format!(
                "expected expression, found {:?} '{}'",
                self.peek(),
                self.peek_text()
            ))),
        }
    }

    // ── Pattern ─────────────────────────────────────────────

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        match self.peek() {
            TokenKind::Underscore => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            TokenKind::Ident => {
                let name = self.advance().text.clone();
                Ok(Pattern::Ident { name })
            }
            TokenKind::IntLiteral
            | TokenKind::FloatLiteral
            | TokenKind::StringLiteral
            | TokenKind::CharLiteral
            | TokenKind::True
            | TokenKind::False => {
                let tok = self.advance();
                Ok(Pattern::Literal { value: tok.text.clone() })
            }
            TokenKind::LParen => {
                self.advance();
                let mut elements = Vec::new();
                while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                    elements.push(self.parse_pattern()?);
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(Pattern::Tuple { elements })
            }
            _ => {
                let tok = self.advance();
                Ok(Pattern::Ident { name: tok.text.clone() })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;

    fn parse_source(source: &str) -> Module {
        let tokens = lexer::lex(source);
        parse(&tokens).unwrap()
    }

    #[test]
    fn test_simple_function() {
        let module = parse_source("+f add(a: i32, b: i32) -> i32 { a + b }");
        assert_eq!(module.items.len(), 1);
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_struct() {
        let module = parse_source("+S Point { x: f64, y: f64, }");
        assert_eq!(module.items.len(), 1);
        if let ItemKind::Struct(ref s) = module.items[0].kind {
            assert_eq!(s.name, "Point");
            assert_eq!(s.fields.len(), 2);
        } else {
            panic!("expected struct");
        }
    }

    #[test]
    fn test_enum() {
        let module = parse_source("E Color { Red, Green, Blue, }");
        assert_eq!(module.items.len(), 1);
        if let ItemKind::Enum(ref e) = module.items[0].kind {
            assert_eq!(e.name, "Color");
            assert_eq!(e.variants.len(), 3);
        } else {
            panic!("expected enum");
        }
    }

    #[test]
    fn test_generic_function() {
        let module = parse_source("f identity[T](x: T) -> T { x }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.generics.len(), 1);
            assert_eq!(f.generics[0].name, "T");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_option_result_types() {
        let module = parse_source("f foo(x: ?i32) -> R[i32, Error] { x }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert!(matches!(f.params[0].ty, Type::Option { .. }));
            assert!(matches!(f.return_type, Some(Type::Result { .. })));
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_use_decl() {
        let module = parse_source("u std.io.Read;");
        assert_eq!(module.items.len(), 1);
        if let ItemKind::Use(ref u) = module.items[0].kind {
            assert_eq!(u.path, vec!["std", "io", "Read"]);
        } else {
            panic!("expected use");
        }
    }

    #[test]
    fn test_effect_def() {
        let module = parse_source("effect io { f read(fd: i32) -> i32; }");
        if let ItemKind::Effect(ref e) = module.items[0].kind {
            assert_eq!(e.name, "io");
            assert_eq!(e.operations.len(), 1);
        } else {
            panic!("expected effect");
        }
    }
}
