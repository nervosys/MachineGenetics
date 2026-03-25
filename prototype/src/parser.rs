/// MechGen LL(1) Parser — recursive descent, zero backtracking.
///
/// Parses the MechGen canonical syntax into an AST.
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
        // Single-char keywords and AI keywords can appear as identifiers in
        // contexts like generic params, field names, capabilities lists, etc.
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
            | TokenKind::KwUse
            | TokenKind::KwNet
            | TokenKind::KwLayer
            | TokenKind::KwTensor
            | TokenKind::KwParam
            | TokenKind::KwTrain
            | TokenKind::KwGrad
            | TokenKind::KwForward
            | TokenKind::KwKb
            | TokenKind::KwFact
            | TokenKind::KwRule
            | TokenKind::KwQuery
            | TokenKind::KwEvolve
            | TokenKind::KwGenome
            | TokenKind::KwMutate
            | TokenKind::KwFitness
            | TokenKind::KwSelect
            | TokenKind::KwCrossover
            | TokenKind::KwPopulation
            | TokenKind::KwGenerations
            | TokenKind::KwRl
            | TokenKind::KwPolicy
            | TokenKind::KwReward => Ok(self.advance().text.clone()),
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
        let mut contracts = Vec::new();
        loop {
            match self.peek() {
                TokenKind::At => attributes.push(self.parse_attribute()?),
                TokenKind::KwReq | TokenKind::KwEns | TokenKind::KwInv => {
                    contracts.push(self.parse_contract_clause()?);
                }
                _ => break,
            }
        }

        let (visibility, kind) = match self.peek() {
            TokenKind::Plus => {
                self.advance();
                (Visibility::Public, self.parse_item_kind(contracts)?)
            }
            _ => (Visibility::Private, self.parse_item_kind(contracts)?),
        };

        Ok(Item { visibility, attributes, kind })
    }

    fn parse_item_kind(&mut self, contracts: Vec<ContractClause>) -> Result<ItemKind, ParseError> {
        match self.peek() {
            TokenKind::KwF => {
                self.parse_function_def(false, false, contracts).map(ItemKind::Function)
            }
            TokenKind::KwAf => {
                self.parse_function_def(true, false, contracts).map(ItemKind::Function)
            }
            TokenKind::KwUf => {
                self.parse_function_def(false, true, contracts).map(ItemKind::Function)
            }
            TokenKind::KwS => self.parse_struct_def(contracts).map(ItemKind::Struct),
            TokenKind::KwE => self.parse_enum_def().map(ItemKind::Enum),
            TokenKind::KwT => self.parse_trait_def().map(ItemKind::Trait),
            TokenKind::KwI => self.parse_impl_block().map(ItemKind::Impl),
            TokenKind::KwMod => self.parse_module_def().map(ItemKind::Module),
            TokenKind::KwUse => self.parse_use_decl().map(ItemKind::Use),
            TokenKind::KwType | TokenKind::KwY => self.parse_type_alias().map(ItemKind::TypeAlias),
            TokenKind::KwC => self.parse_const_def().map(ItemKind::Const),
            TokenKind::KwZ => self.parse_static_def().map(ItemKind::Static),
            TokenKind::KwEffect => self.parse_effect_def().map(ItemKind::Effect),
            TokenKind::KwSpec => self.parse_spec_def().map(ItemKind::Spec),
            TokenKind::KwAgent => self.parse_agent_def().map(ItemKind::Agent),
            TokenKind::KwSwarm => self.parse_swarm_def().map(ItemKind::Swarm),
            TokenKind::KwNet => self.parse_net_def().map(ItemKind::Net),
            TokenKind::KwKb => self.parse_kb_def().map(ItemKind::Kb),
            TokenKind::KwEvolve => self.parse_evolve_def().map(ItemKind::Evolve),
            TokenKind::KwTrain => self.parse_train_def().map(ItemKind::Train),
            _ => Err(self.error(&format!("expected item, found {:?}", self.peek()))),
        }
    }

    // ── Contracts ───────────────────────────────────────────

    fn parse_contract_clause(&mut self) -> Result<ContractClause, ParseError> {
        let kind = match self.peek() {
            TokenKind::KwReq => {
                self.advance();
                ContractClauseKind::Requires
            }
            TokenKind::KwEns => {
                self.advance();
                ContractClauseKind::Ensures
            }
            TokenKind::KwInv => {
                self.advance();
                ContractClauseKind::Invariant
            }
            _ => return Err(self.error("expected @req, @ens, or @inv")),
        };

        self.expect(TokenKind::LParen)?;

        // Collect condition tokens until we hit a comma-separated message or close paren.
        // Format: @req(condition) or @req(condition, "message")
        let mut condition_parts = Vec::new();
        let mut message: Option<String> = None;
        let mut depth: usize = 0;

        while self.peek() != TokenKind::RParen || depth > 0 {
            if self.peek() == TokenKind::Eof {
                return Err(self.error("unterminated contract clause"));
            }
            // Track nested parens inside the condition
            if self.peek() == TokenKind::LParen {
                depth += 1;
            }
            if self.peek() == TokenKind::RParen && depth > 0 {
                depth -= 1;
            }
            // A comma at depth 0 separates condition from message
            if self.peek() == TokenKind::Comma && depth == 0 {
                self.advance(); // consume comma
                // Next token should be a string literal (the message)
                if self.peek() == TokenKind::StringLiteral {
                    let tok = self.advance();
                    let text = tok.text.clone();
                    // Strip surrounding quotes
                    message = Some(text.trim_matches('"').to_string());
                }
                break;
            }
            let tok = self.advance();
            condition_parts.push(tok.text.clone());
        }

        self.expect(TokenKind::RParen)?;

        let condition = condition_parts.join(" ");
        Ok(ContractClause { kind, condition, message })
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

    fn parse_function_def(
        &mut self,
        is_async: bool,
        is_unsafe: bool,
        contracts: Vec<ContractClause>,
    ) -> Result<FunctionDef, ParseError> {
        // Consume the function keyword (f, af, or uf)
        self.advance();
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

        let where_clause = if self.peek() == TokenKind::TildeArrow {
            self.parse_where_clause()?
        } else {
            Vec::new()
        };

        let body = self.parse_block()?;

        Ok(FunctionDef {
            name,
            is_async,
            is_unsafe,
            generics,
            params,
            return_type,
            where_clause,
            effects: Vec::new(),
            contracts,
            body,
        })
    }

    // ── Struct ──────────────────────────────────────────────

    fn parse_struct_def(
        &mut self,
        contracts: Vec<ContractClause>,
    ) -> Result<StructDef, ParseError> {
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

        Ok(StructDef { name, generics, contracts, fields })
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
        // Accept both KwType ("type") and KwY ("Y")
        self.advance();
        let name = self.expect_ident()?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::Assign)?;
        let ty = self.parse_type()?;

        // Optional refinement predicate: ~> condition ;
        let refinement = if self.peek() == TokenKind::TildeArrow {
            self.advance();
            let mut parts = Vec::new();
            while self.peek() != TokenKind::Semi && self.peek() != TokenKind::Eof {
                parts.push(self.advance().text.clone());
            }
            Some(parts.join(" "))
        } else {
            None
        };

        self.expect(TokenKind::Semi)?;

        Ok(TypeAlias { name, generics, ty, refinement })
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

    // ── Static ──────────────────────────────────────────────

    fn parse_static_def(&mut self) -> Result<StaticDef, ParseError> {
        self.expect(TokenKind::KwZ)?;
        let mutable = if self.peek() == TokenKind::KwM {
            self.advance();
            true
        } else {
            false
        };
        let name = self.expect_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Assign)?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semi)?;

        Ok(StaticDef { name, mutable, ty, value })
    }

    // ── Where Clause ────────────────────────────────────────

    fn parse_where_clause(&mut self) -> Result<Vec<WherePredicate>, ParseError> {
        self.expect(TokenKind::TildeArrow)?;
        let mut predicates = Vec::new();
        loop {
            let type_param = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let mut bounds = Vec::new();
            loop {
                bounds.push(self.expect_ident()?);
                if self.peek() == TokenKind::Plus {
                    self.advance();
                } else {
                    break;
                }
            }
            predicates.push(WherePredicate { type_param, bounds });
            if self.peek() == TokenKind::Comma {
                self.advance();
            } else {
                break;
            }
        }
        Ok(predicates)
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

        // Optional parameters: spec name[T](param: Type, ...) -> ReturnType
        let params = if self.peek() == TokenKind::LParen {
            self.expect(TokenKind::LParen)?;
            let p = self.parse_param_list()?;
            self.expect(TokenKind::RParen)?;
            p
        } else {
            Vec::new()
        };

        let return_type = if self.peek() == TokenKind::Arrow {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::KwReq => {
                    self.advance();
                    let text = self.collect_paren_text()?;
                    items.push(SpecItem::Require(text));
                }
                TokenKind::KwEns => {
                    self.advance();
                    let text = self.collect_paren_text()?;
                    items.push(SpecItem::Ensure(text));
                }
                TokenKind::KwInv => {
                    self.advance();
                    let text = self.collect_paren_text()?;
                    items.push(SpecItem::Invariant(text));
                }
                TokenKind::KwFx => {
                    self.advance();
                    self.expect(TokenKind::LParen)?;
                    let mut effects = Vec::new();
                    while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                        let tok = self.advance();
                        if tok.kind != TokenKind::Comma {
                            effects.push(tok.text.clone());
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    items.push(SpecItem::Effect(effects));
                }
                TokenKind::KwPerf => {
                    self.advance();
                    self.expect(TokenKind::LParen)?;
                    // Collect metric (tokens up to first comma)
                    let mut metric_parts = Vec::new();
                    while self.peek() != TokenKind::Comma
                        && self.peek() != TokenKind::RParen
                        && self.peek() != TokenKind::Eof
                    {
                        metric_parts.push(self.advance().text.clone());
                    }
                    let metric = metric_parts.join(" ");
                    // Collect bound (tokens after comma, handling nested parens)
                    let mut bound = String::new();
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                        let mut parts = Vec::new();
                        let mut depth: usize = 0;
                        while (self.peek() != TokenKind::RParen || depth > 0)
                            && self.peek() != TokenKind::Eof
                        {
                            if self.peek() == TokenKind::LParen {
                                depth += 1;
                            }
                            if self.peek() == TokenKind::RParen && depth > 0 {
                                depth -= 1;
                            }
                            parts.push(self.advance().text.clone());
                        }
                        bound = parts.join("");
                    }
                    self.expect(TokenKind::RParen)?;
                    items.push(SpecItem::Performance(metric, bound));
                }
                TokenKind::Semi => {
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        Ok(SpecDef { name, generics, params, return_type, items })
    }

    /// Helper: consume `(...)` and return all tokens as a single string.
    fn collect_paren_text(&mut self) -> Result<String, ParseError> {
        self.expect(TokenKind::LParen)?;
        let mut parts = Vec::new();
        let mut depth: usize = 0;
        while self.peek() != TokenKind::RParen || depth > 0 {
            if self.peek() == TokenKind::Eof {
                return Err(self.error("unterminated parenthesized expression"));
            }
            if self.peek() == TokenKind::LParen {
                depth += 1;
            }
            if self.peek() == TokenKind::RParen && depth > 0 {
                depth -= 1;
            }
            let tok = self.advance();
            parts.push(tok.text.clone());
        }
        self.expect(TokenKind::RParen)?;
        Ok(parts.join(" "))
    }

    // ── Agent Definitions ───────────────────────────────────

    /// Parse: `agent Name { capabilities: [cap1, cap2] requires_approval: [op1, op2] }`
    fn parse_agent_def(&mut self) -> Result<AgentDef, ParseError> {
        self.expect(TokenKind::KwAgent)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut capabilities = Vec::new();
        let mut requires_approval = Vec::new();

        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::Ident => {
                    let label = self.advance();
                    let label_text = label.text.clone();
                    match label_text.as_str() {
                        "capabilities" => {
                            self.expect(TokenKind::Colon)?;
                            capabilities = self.parse_bracket_string_list()?;
                        }
                        "requires_approval" => {
                            self.expect(TokenKind::Colon)?;
                            requires_approval = self.parse_bracket_string_list()?;
                        }
                        other => {
                            return Err(self.error(&format!("unknown agent field `{}`", other)));
                        }
                    }
                }
                TokenKind::Comma | TokenKind::Semi => {
                    self.advance();
                }
                _ => {
                    return Err(self
                        .error(&format!("expected agent field or `}}`, found {:?}", self.peek())));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(AgentDef { name, capabilities, requires_approval })
    }

    // ── Swarm ───────────────────────────────────────────────

    /// Parse: `swarm Name { agent: Type; size: N; topology: topo; consensus: strat; dispatch { ... } aggregate { ... } on_failure { ... } }`
    fn parse_swarm_def(&mut self) -> Result<SwarmDef, ParseError> {
        self.expect(TokenKind::KwSwarm)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut agent_type = String::new();
        let mut size = None;
        let mut topology = None;
        let mut consensus = None;
        let mut on_dispatch = None;
        let mut on_aggregate = None;
        let mut on_failure = None;

        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                // `agent` is a keyword, so handle it explicitly
                TokenKind::KwAgent => {
                    self.advance();
                    self.expect(TokenKind::Colon)?;
                    agent_type = self.expect_ident()?;
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                    }
                }
                TokenKind::Ident => {
                    let label = self.advance();
                    let label_text = label.text.clone();
                    match label_text.as_str() {
                        "size" => {
                            self.expect(TokenKind::Colon)?;
                            size = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "topology" | "topo" => {
                            self.expect(TokenKind::Colon)?;
                            topology = Some(self.expect_ident()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "consensus" | "cons" => {
                            self.expect(TokenKind::Colon)?;
                            consensus = Some(self.expect_ident()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "dispatch" => {
                            on_dispatch = Some(self.parse_block()?);
                        }
                        "aggregate" => {
                            on_aggregate = Some(self.parse_block()?);
                        }
                        "on_failure" => {
                            on_failure = Some(self.parse_block()?);
                        }
                        other => {
                            return Err(self.error(&format!("unknown swarm field `{}`", other)));
                        }
                    }
                }
                TokenKind::Comma | TokenKind::Semi => {
                    self.advance();
                }
                _ => {
                    return Err(self
                        .error(&format!("expected swarm field or `}}`, found {:?}", self.peek())));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(SwarmDef {
            name,
            agent_type,
            size,
            topology,
            consensus,
            on_dispatch,
            on_aggregate,
            on_failure,
        })
    }

    // ── Net ─────────────────────────────────────────────────

    /// Parse `net Name[T] { layer ...; forward { ... } }`
    fn parse_net_def(&mut self) -> Result<NetDef, ParseError> {
        self.expect(TokenKind::KwNet)?;
        let name = self.expect_ident()?;

        let generics = if self.peek() == TokenKind::LBrack {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;
        let mut layers = Vec::new();
        let mut forward = None;

        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::KwLayer => {
                    self.advance();
                    let lname = self.expect_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let layer_type = self.parse_type()?;
                    let mut args = Vec::new();
                    if self.peek() == TokenKind::LParen {
                        self.advance();
                        while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                            args.push(self.parse_expr()?);
                            if self.peek() == TokenKind::Comma {
                                self.advance();
                            }
                        }
                        self.expect(TokenKind::RParen)?;
                    }
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                    }
                    layers.push(LayerDef { name: lname, layer_type, args });
                }
                TokenKind::KwForward => {
                    self.advance();
                    forward = Some(self.parse_block()?);
                }
                TokenKind::Semi | TokenKind::Comma => {
                    self.advance();
                }
                _ => {
                    return Err(self.error(&format!(
                        "expected `layer`, `forward`, or `}}` in net, found {:?}",
                        self.peek()
                    )));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        let forward = forward.unwrap_or(Block { stmts: Vec::new(), tail_expr: None });
        Ok(NetDef { name, generics, layers, forward })
    }

    // ── Knowledge Base ──────────────────────────────────────

    /// Parse `kb Name { fact ...; rule ...; }`
    fn parse_kb_def(&mut self) -> Result<KbDef, ParseError> {
        self.expect(TokenKind::KwKb)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut facts = Vec::new();
        let mut rules = Vec::new();

        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::KwFact => {
                    self.advance();
                    let fname = self.expect_ident()?;
                    self.expect(TokenKind::LParen)?;
                    let mut args = Vec::new();
                    while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                        args.push(self.parse_expr()?);
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                    }
                    facts.push(FactDef { name: fname, args });
                }
                TokenKind::KwRule => {
                    self.advance();
                    let rname = self.expect_ident()?;
                    self.expect(TokenKind::LParen)?;
                    let params = self.parse_param_list()?;
                    self.expect(TokenKind::RParen)?;

                    // Optional conditions: `where expr, expr, ...`
                    let mut conditions = Vec::new();
                    if self.peek() == TokenKind::Ident && self.peek_text() == "where" {
                        self.advance();
                        conditions.push(self.parse_expr()?);
                        while self.peek() == TokenKind::Comma {
                            self.advance();
                            conditions.push(self.parse_expr()?);
                        }
                    }

                    let body = self.parse_block()?;
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                    }
                    rules.push(RuleDef { name: rname, params, conditions, body });
                }
                TokenKind::Semi | TokenKind::Comma => {
                    self.advance();
                }
                _ => {
                    return Err(self.error(&format!(
                        "expected `fact`, `rule`, or `}}` in kb, found {:?}",
                        self.peek()
                    )));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        Ok(KbDef { name, facts, rules })
    }

    // ── Evolve ──────────────────────────────────────────────

    /// Parse `evolve Name { genome Type; fitness { ... }; ... }`
    fn parse_evolve_def(&mut self) -> Result<EvolveDef, ParseError> {
        self.expect(TokenKind::KwEvolve)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut genome_type = Type::Inferred;
        let mut population_size = None;
        let mut generations = None;
        let mut fitness = None;
        let mut mutate_fn = None;
        let mut crossover_fn = None;
        let mut select_fn = None;

        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::KwGenome => {
                    self.advance();
                    self.expect(TokenKind::Colon)?;
                    genome_type = self.parse_type()?;
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                    }
                }
                TokenKind::KwPopulation => {
                    self.advance();
                    self.expect(TokenKind::Colon)?;
                    population_size = Some(self.parse_expr()?);
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                    }
                }
                TokenKind::KwGenerations => {
                    self.advance();
                    self.expect(TokenKind::Colon)?;
                    generations = Some(self.parse_expr()?);
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                    }
                }
                TokenKind::KwFitness => {
                    self.advance();
                    fitness = Some(self.parse_block()?);
                }
                TokenKind::KwMutate => {
                    self.advance();
                    mutate_fn = Some(self.parse_block()?);
                }
                TokenKind::KwCrossover => {
                    self.advance();
                    crossover_fn = Some(self.parse_block()?);
                }
                TokenKind::KwSelect => {
                    self.advance();
                    select_fn = Some(self.parse_block()?);
                }
                TokenKind::Semi | TokenKind::Comma => {
                    self.advance();
                }
                _ => {
                    return Err(self.error(&format!(
                        "expected evolve field or `}}`, found {:?}",
                        self.peek()
                    )));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        let fitness = fitness.unwrap_or(Block { stmts: Vec::new(), tail_expr: None });
        Ok(EvolveDef {
            name,
            genome_type,
            population_size,
            generations,
            fitness,
            mutate_fn,
            crossover_fn,
            select_fn,
        })
    }

    // ── Train ───────────────────────────────────────────────

    /// Parse `train Name { net: ident; optimizer: expr; loss: expr; epochs: expr; body { ... } }`
    fn parse_train_def(&mut self) -> Result<TrainDef, ParseError> {
        self.expect(TokenKind::KwTrain)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LBrace)?;

        let mut net = String::new();
        let mut optimizer = None;
        let mut loss = None;
        let mut epochs = None;
        let mut body = None;

        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                // Accept Ident or KwNet (since `net` is now a keyword)
                TokenKind::Ident | TokenKind::KwNet => {
                    let label = self.advance();
                    let label_text = label.text.clone();
                    match label_text.as_str() {
                        "net" => {
                            self.expect(TokenKind::Colon)?;
                            net = self.expect_ident()?;
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "optimizer" => {
                            self.expect(TokenKind::Colon)?;
                            optimizer = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "loss" => {
                            self.expect(TokenKind::Colon)?;
                            loss = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "epochs" => {
                            self.expect(TokenKind::Colon)?;
                            epochs = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "body" => {
                            body = Some(self.parse_block()?);
                        }
                        other => {
                            return Err(self.error(&format!("unknown train field `{}`", other)));
                        }
                    }
                }
                TokenKind::Semi | TokenKind::Comma => {
                    self.advance();
                }
                _ => {
                    return Err(self
                        .error(&format!("expected train field or `}}`, found {:?}", self.peek())));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        let body = body.unwrap_or(Block { stmts: Vec::new(), tail_expr: None });
        Ok(TrainDef { name, net, optimizer, loss, epochs, body })
    }

    /// Parse `[ident1, ident2, ...]` → Vec<String>
    fn parse_bracket_string_list(&mut self) -> Result<Vec<String>, ParseError> {
        self.expect(TokenKind::LBrack)?;
        let mut items = Vec::new();
        while self.peek() != TokenKind::RBrack && self.peek() != TokenKind::Eof {
            let tok = self.expect_ident()?;
            items.push(tok);
            if self.peek() == TokenKind::Comma {
                self.advance();
            }
        }
        self.expect(TokenKind::RBrack)?;
        Ok(items)
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

            // &~T (Cow)
            TokenKind::AndTilde => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Cow { inner: Box::new(inner) })
            }

            // %T (Cell) / %!T (RefCell)
            TokenKind::Percent => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Cell { inner: Box::new(inner) })
            }
            TokenKind::PercentNot => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::RefCell { inner: Box::new(inner) })
            }

            // #T (Mutex) / #~T (RwLock)
            TokenKind::Hash => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Mutex { inner: Box::new(inner) })
            }
            TokenKind::HashTilde => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::RwLock { inner: Box::new(inner) })
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

            // {K: V} (map) or {K} (set)
            TokenKind::LBrace => {
                self.advance();
                let key = self.parse_type()?;
                if self.peek() == TokenKind::Colon {
                    self.advance();
                    let value = self.parse_type()?;
                    self.expect(TokenKind::RBrace)?;
                    Ok(Type::Map { key: Box::new(key), value: Box::new(value) })
                } else {
                    self.expect(TokenKind::RBrace)?;
                    Ok(Type::Set { inner: Box::new(key) })
                }
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

            // AI-native types
            // Tensor[T, N, M, ...]
            TokenKind::KwTensor => {
                self.advance();
                self.expect(TokenKind::LBrack)?;
                let inner = self.parse_type()?;
                let mut shape = Vec::new();
                while self.peek() == TokenKind::Comma {
                    self.advance();
                    match self.peek() {
                        TokenKind::IntLiteral => {
                            let tok = self.advance();
                            shape.push(TensorDim::Lit(tok.text.parse().unwrap_or(0)));
                        }
                        _ => {
                            let name = self.expect_ident()?;
                            shape.push(TensorDim::Var(name));
                        }
                    }
                }
                self.expect(TokenKind::RBrack)?;
                Ok(Type::Tensor { inner: Box::new(inner), shape })
            }

            // Param[T, N, M, ...]
            TokenKind::KwParam => {
                self.advance();
                self.expect(TokenKind::LBrack)?;
                let inner = self.parse_type()?;
                let mut shape = Vec::new();
                while self.peek() == TokenKind::Comma {
                    self.advance();
                    match self.peek() {
                        TokenKind::IntLiteral => {
                            let tok = self.advance();
                            shape.push(TensorDim::Lit(tok.text.parse().unwrap_or(0)));
                        }
                        _ => {
                            let name = self.expect_ident()?;
                            shape.push(TensorDim::Var(name));
                        }
                    }
                }
                self.expect(TokenKind::RBrack)?;
                Ok(Type::ParamTy { inner: Box::new(inner), shape })
            }

            // Genome[T]
            TokenKind::KwGenome => {
                self.advance();
                self.expect(TokenKind::LBrack)?;
                let inner = self.parse_type()?;
                self.expect(TokenKind::RBrack)?;
                Ok(Type::Genome { inner: Box::new(inner) })
            }

            // Policy[State, Action]
            TokenKind::KwPolicy => {
                self.advance();
                self.expect(TokenKind::LBrack)?;
                let state = self.parse_type()?;
                self.expect(TokenKind::Comma)?;
                let action = self.parse_type()?;
                self.expect(TokenKind::RBrack)?;
                Ok(Type::Policy { state: Box::new(state), action: Box::new(action) })
            }

            // KnowledgeBase (no params)
            TokenKind::KwKb => {
                self.advance();
                Ok(Type::KnowledgeBase)
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
                // Postfix tensor ops: ⊤ (transpose), ⊥ (flatten)
                TokenKind::TensorTranspose => {
                    self.advance();
                    lhs = Expr::Unary { op: "⊤".to_string(), operand: Box::new(lhs) };
                    continue;
                }
                TokenKind::TensorFlatten => {
                    self.advance();
                    lhs = Expr::Unary { op: "⊥".to_string(), operand: Box::new(lhs) };
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
                // Tensor operators (higher precedence than arithmetic)
                TokenKind::TensorMatmul => ("⊗", 23, 24),
                TokenKind::TensorHadamard => ("⊙", 23, 24),
                TokenKind::TensorPipeline => ("▸", 3, 4),
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

            // Break (KwBreak keyword — note: `!` as break is handled via
            // context in statement parsing; in expr prefix position `!` is unary-not)
            TokenKind::KwBreak => {
                self.advance();
                if self.peek() == TokenKind::Semi || self.peek() == TokenKind::RBrace {
                    Ok(Expr::Break { value: None })
                } else {
                    let val = self.parse_expr()?;
                    Ok(Expr::Break { value: Some(Box::new(val)) })
                }
            }

            // Continue (also via >> token which lexes as Shr)
            TokenKind::KwContinue | TokenKind::Shr => {
                self.advance();
                Ok(Expr::Continue)
            }

            // Todo (??) and Unimplemented (???)
            TokenKind::Todo => {
                self.advance();
                Ok(Expr::Todo)
            }
            TokenKind::Unimplemented => {
                self.advance();
                Ok(Expr::Unimplemented)
            }

            // Match with scrutinee: ?= expr { pat => expr, ... }
            TokenKind::QuestionEq => {
                self.advance();
                let scrutinee = self.parse_expr()?;
                self.expect(TokenKind::LBrace)?;
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
                Ok(Expr::Match { scrutinee: Some(Box::new(scrutinee)), arms })
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
                    Ok(Expr::Match { scrutinee: None, arms })
                } else {
                    // If expression: ? cond { ... } : { ... }
                    let cond = self.parse_expr()?;
                    let then_block = self.parse_block()?;
                    let else_block = if self.peek() == TokenKind::Colon
                        || self.peek() == TokenKind::KwOr
                    {
                        self.advance();
                        Some(self.parse_block()?)
                    } else {
                        None
                    };
                    Ok(Expr::If { cond: Box::new(cond), then_block, else_block })
                }
            }

            // For loop: @ pattern : iter { ... } or each pattern of iter { ... }
            TokenKind::At => {
                self.advance();
                let pattern = self.parse_pattern()?;
                // Accept both `:` (agent mode) and `of` (human mode)
                if self.peek() == TokenKind::KwOf {
                    self.advance();
                } else {
                    self.expect(TokenKind::Colon)?;
                }
                let iter = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Expr::For { pattern, iter: Box::new(iter), body })
            }

            // While loop: @w cond { ... }
            TokenKind::AtW => {
                self.advance();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Expr::While { cond: Box::new(cond), body })
            }

            // Infinite loop: @@ { ... } (also legacy "loop" keyword)
            TokenKind::AtAt | TokenKind::KwLoop => {
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
            TokenKind::StringLiteral
            | TokenKind::FormatString
            | TokenKind::PrintString
            | TokenKind::EprintString => {
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

    #[test]
    fn test_async_function() {
        let module = parse_source("af fetch(url: s) -> s { url }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.name, "fetch");
            assert!(f.is_async);
            assert!(!f.is_unsafe);
        } else {
            panic!("expected async function");
        }
    }

    #[test]
    fn test_unsafe_function() {
        let module = parse_source("uf deref(ptr: ^i32) -> i32 { 0 }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.name, "deref");
            assert!(!f.is_async);
            assert!(f.is_unsafe);
        } else {
            panic!("expected unsafe function");
        }
    }

    #[test]
    fn test_static_def() {
        let module = parse_source("Z COUNT: i32 = 0;");
        if let ItemKind::Static(ref s) = module.items[0].kind {
            assert_eq!(s.name, "COUNT");
            assert!(!s.mutable);
        } else {
            panic!("expected static");
        }
    }

    #[test]
    fn test_static_mutable() {
        let module = parse_source("Z m COUNTER: i32 = 0;");
        if let ItemKind::Static(ref s) = module.items[0].kind {
            assert_eq!(s.name, "COUNTER");
            assert!(s.mutable);
        } else {
            panic!("expected mutable static");
        }
    }

    #[test]
    fn test_type_alias_y() {
        let module = parse_source("Y Num = i32;");
        if let ItemKind::TypeAlias(ref ta) = module.items[0].kind {
            assert_eq!(ta.name, "Num");
        } else {
            panic!("expected type alias");
        }
    }

    #[test]
    fn test_while_loop() {
        let module = parse_source("f run() { @w true { 0 } }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            // while loop is the tail expression of the function body
            assert!(f.body.tail_expr.is_some() || !f.body.stmts.is_empty());
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_infinite_loop() {
        let module = parse_source("f run() { @@ { 0 } }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert!(f.body.tail_expr.is_some() || !f.body.stmts.is_empty());
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_todo_expr() {
        let module = parse_source("f placeholder() -> i32 { ?? }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert!(f.body.tail_expr.is_some());
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_unimplemented_expr() {
        let module = parse_source("f stub() -> i32 { ??? }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert!(f.body.tail_expr.is_some());
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_continue_expr() {
        let module = parse_source("f run() { @@ { >> } }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert!(f.body.tail_expr.is_some() || !f.body.stmts.is_empty());
        } else {
            panic!("expected function with continue");
        }
    }

    // ── Contract parsing tests ──────────────────────────────

    #[test]
    fn test_function_with_requires() {
        let module = parse_source("@req(n > 0) f factorial(n: u64) -> u64 { n }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.name, "factorial");
            assert_eq!(f.contracts.len(), 1);
            assert_eq!(f.contracts[0].kind, ContractClauseKind::Requires);
            assert_eq!(f.contracts[0].condition, "n > 0");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_function_with_ensures() {
        let module = parse_source("@ens(result > 0) f positive() -> i32 { 1 }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.contracts.len(), 1);
            assert_eq!(f.contracts[0].kind, ContractClauseKind::Ensures);
            assert_eq!(f.contracts[0].condition, "result > 0");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_function_with_multiple_contracts() {
        let src = "@req(divisor != 0) @ens(result * divisor == dividend) f safe_div(dividend: i64, divisor: i64) -> i64 { dividend / divisor }";
        let module = parse_source(src);
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.name, "safe_div");
            assert_eq!(f.contracts.len(), 2);
            assert_eq!(f.contracts[0].kind, ContractClauseKind::Requires);
            assert_eq!(f.contracts[1].kind, ContractClauseKind::Ensures);
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_function_with_contract_message() {
        let src = r#"@req(n > 0, "n must be positive") f factorial(n: u64) -> u64 { n }"#;
        let module = parse_source(src);
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.contracts[0].message.as_deref(), Some("n must be positive"));
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_pub_function_with_contracts() {
        let src = "@req(x >= 0) +f sqrt(x: f64) -> f64 { x }";
        let module = parse_source(src);
        assert_eq!(module.items[0].visibility, Visibility::Public);
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert_eq!(f.contracts.len(), 1);
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_struct_with_invariant() {
        let src = "@inv(_.len <= _.cap) S Buffer { len: usize, cap: usize }";
        let module = parse_source(src);
        if let ItemKind::Struct(ref s) = module.items[0].kind {
            assert_eq!(s.name, "Buffer");
            assert_eq!(s.contracts.len(), 1);
            assert_eq!(s.contracts[0].kind, ContractClauseKind::Invariant);
            assert!(s.contracts[0].condition.contains("len"));
        } else {
            panic!("expected struct");
        }
    }

    #[test]
    fn test_function_no_contracts() {
        let module = parse_source("f noop() { }");
        if let ItemKind::Function(ref f) = module.items[0].kind {
            assert!(f.contracts.is_empty());
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_spec_block_with_items() {
        let src = "spec Sortable { @req(_.len > 0) @ens(is_sorted(result)) }";
        let module = parse_source(src);
        if let ItemKind::Spec(ref s) = module.items[0].kind {
            assert_eq!(s.name, "Sortable");
            assert_eq!(s.items.len(), 2);
        } else {
            panic!("expected spec");
        }
    }

    // ── Spec definition (Step 31) tests ─────────────────────

    #[test]
    fn test_spec_with_params_and_return_type() {
        let src = "spec sort_unstable[T](slice: [T]) -> [T] { @req(slice.len > 0) @ens(result.is_sorted) }";
        let module = parse_source(src);
        if let ItemKind::Spec(ref s) = module.items[0].kind {
            assert_eq!(s.name, "sort_unstable");
            assert_eq!(s.generics.len(), 1);
            assert_eq!(s.params.len(), 1);
            assert!(s.return_type.is_some());
            assert_eq!(s.items.len(), 2);
        } else {
            panic!("expected spec");
        }
    }

    #[test]
    fn test_spec_no_params() {
        let src = "spec Invariants { @inv(_.len <= _.cap) }";
        let module = parse_source(src);
        if let ItemKind::Spec(ref s) = module.items[0].kind {
            assert_eq!(s.name, "Invariants");
            assert!(s.params.is_empty());
            assert!(s.return_type.is_none());
            assert_eq!(s.items.len(), 1);
        } else {
            panic!("expected spec");
        }
    }

    #[test]
    fn test_spec_with_fx_and_perf() {
        let src = "spec pure_sort[T](s: [T]) -> [T] { @req(s.len > 0) @fx(none) @perf(time, O(n*log(n))) }";
        let module = parse_source(src);
        if let ItemKind::Spec(ref s) = module.items[0].kind {
            assert_eq!(s.name, "pure_sort");
            assert_eq!(s.items.len(), 3);
            match &s.items[1] {
                SpecItem::Effect(effs) => assert!(effs.contains(&"none".to_string())),
                other => panic!("expected Effect, got {:?}", other),
            }
            match &s.items[2] {
                SpecItem::Performance(metric, bound) => {
                    assert_eq!(metric, "time");
                    assert!(!bound.is_empty());
                }
                other => panic!("expected Performance, got {:?}", other),
            }
        } else {
            panic!("expected spec");
        }
    }

    #[test]
    fn test_spec_multiple_requires() {
        let src = "spec bounded(x: i32) { @req(x >= 0) @req(x < 100) @ens(result > 0) }";
        let module = parse_source(src);
        if let ItemKind::Spec(ref s) = module.items[0].kind {
            assert_eq!(s.items.len(), 3);
            let req_count = s.items.iter().filter(|i| matches!(i, SpecItem::Require(_))).count();
            assert_eq!(req_count, 2);
        } else {
            panic!("expected spec");
        }
    }

    // ── Refinement types (Step 32) tests ────────────────────

    #[test]
    fn test_type_alias_with_refinement() {
        let src = "Y NonZeroPort = u16 ~> _.value > 0 && _.value <= 65535;";
        let module = parse_source(src);
        if let ItemKind::TypeAlias(ref ta) = module.items[0].kind {
            assert_eq!(ta.name, "NonZeroPort");
            assert!(ta.refinement.is_some());
            assert!(ta.refinement.as_deref().unwrap().contains("> 0"));
        } else {
            panic!("expected type alias");
        }
    }

    #[test]
    fn test_type_alias_no_refinement() {
        let src = "Y Meters = f64;";
        let module = parse_source(src);
        if let ItemKind::TypeAlias(ref ta) = module.items[0].kind {
            assert_eq!(ta.name, "Meters");
            assert!(ta.refinement.is_none());
        } else {
            panic!("expected type alias");
        }
    }

    #[test]
    fn test_type_alias_generic_with_refinement() {
        let src = "Y ValidIndex[N] = usize ~> _.value < N;";
        let module = parse_source(src);
        if let ItemKind::TypeAlias(ref ta) = module.items[0].kind {
            assert_eq!(ta.name, "ValidIndex");
            assert_eq!(ta.generics.len(), 1);
            assert!(ta.refinement.is_some());
        } else {
            panic!("expected type alias");
        }
    }

    // ── Agent definition tests ──────────────────────────────

    #[test]
    fn test_agent_def_basic() {
        let src = "agent Reviewer { capabilities: [read_source, query_types] }";
        let module = parse_source(src);
        if let ItemKind::Agent(ref ad) = module.items[0].kind {
            assert_eq!(ad.name, "Reviewer");
            assert_eq!(ad.capabilities, vec!["read_source", "query_types"]);
            assert!(ad.requires_approval.is_empty());
        } else {
            panic!("expected agent def");
        }
    }

    #[test]
    fn test_agent_def_with_approval() {
        let src = "agent Deployer { capabilities: [io_write, net] requires_approval: [exec, ffi] }";
        let module = parse_source(src);
        if let ItemKind::Agent(ref ad) = module.items[0].kind {
            assert_eq!(ad.name, "Deployer");
            assert_eq!(ad.capabilities, vec!["io_write", "net"]);
            assert_eq!(ad.requires_approval, vec!["exec", "ffi"]);
        } else {
            panic!("expected agent def");
        }
    }

    #[test]
    fn test_agent_def_empty_capabilities() {
        let src = "agent Minimal { capabilities: [] }";
        let module = parse_source(src);
        if let ItemKind::Agent(ref ad) = module.items[0].kind {
            assert_eq!(ad.name, "Minimal");
            assert!(ad.capabilities.is_empty());
        } else {
            panic!("expected agent def");
        }
    }

    // ── AI construct parsing tests ──────────────────────────

    #[test]
    fn test_parse_net_def() {
        let src = r#"net MyNet {
            layer fc1: Linear(784, 128);
            layer fc2: Linear(128, 10);
            forward { fc1 }
        }"#;
        let module = parse_source(src);
        if let ItemKind::Net(ref nd) = module.items[0].kind {
            assert_eq!(nd.name, "MyNet");
            assert_eq!(nd.layers.len(), 2);
            assert_eq!(nd.layers[0].name, "fc1");
            assert_eq!(nd.layers[1].name, "fc2");
        } else {
            panic!("expected net def");
        }
    }

    #[test]
    fn test_parse_kb_def() {
        let src = r#"kb Animals {
            fact dog(1);
            fact cat(2);
        }"#;
        let module = parse_source(src);
        if let ItemKind::Kb(ref kd) = module.items[0].kind {
            assert_eq!(kd.name, "Animals");
            assert_eq!(kd.facts.len(), 2);
            assert_eq!(kd.facts[0].name, "dog");
            assert_eq!(kd.facts[1].name, "cat");
        } else {
            panic!("expected kb def");
        }
    }

    #[test]
    fn test_parse_evolve_def() {
        let src = r#"evolve Optimizer {
            genome: Weights;
            population: 100;
            generations: 50;
            fitness { 0 }
        }"#;
        let module = parse_source(src);
        if let ItemKind::Evolve(ref ed) = module.items[0].kind {
            assert_eq!(ed.name, "Optimizer");
            assert!(ed.population_size.is_some());
            assert!(ed.generations.is_some());
        } else {
            panic!("expected evolve def");
        }
    }

    #[test]
    fn test_parse_train_def() {
        let src = r#"train MyTraining {
            net: MyNet;
            epochs: 10;
            body { 0 }
        }"#;
        let module = parse_source(src);
        if let ItemKind::Train(ref td) = module.items[0].kind {
            assert_eq!(td.name, "MyTraining");
            assert_eq!(td.net, "MyNet");
            assert!(td.epochs.is_some());
        } else {
            panic!("expected train def");
        }
    }

    #[test]
    fn test_parse_tensor_type() {
        let src = "f identity(x: tensor[Float, 3, 3]) -> tensor[Float, 3, 3] { x }";
        let module = parse_source(src);
        if let ItemKind::Function(ref fd) = module.items[0].kind {
            assert_eq!(fd.name, "identity");
            match &fd.params[0].ty {
                Type::Tensor { shape, .. } => assert_eq!(shape.len(), 2),
                other => panic!("expected Tensor type, got {:?}", other),
            }
        } else {
            panic!("expected function def");
        }
    }

    #[test]
    fn test_parse_policy_type() {
        let src = "f act(p: policy[State, Action]) -> Action { p }";
        let tokens = lexer::lex(src);
        let module = parse(&tokens).expect("parse failed");
        if let ItemKind::Function(ref fd) = module.items[0].kind {
            match &fd.params[0].ty {
                Type::Policy { .. } => {}
                other => panic!("expected Policy type, got {:?}", other),
            }
        } else {
            panic!("expected function def, got {:?}", module.items[0].kind);
        }
    }
}
