/// MAGE LL(1) Parser — recursive descent, zero backtracking.
///
/// Parses the MAGE canonical syntax into an AST.
/// Every decision point uses a single token of lookahead.
use crate::ast::*;
use crate::lexer::{Token, TokenKind};
use std::collections::HashMap;

/// A parse-time **block macro**: `block Name(p1, p2) { layer …; }`. Stored on the
/// parser and expanded where referenced inside a `net`/`stack` body, with the
/// params substituted into the layer args. Blocks are macros — they lower away,
/// so nothing downstream of the parser needs to know about them.
#[derive(Debug, Clone)]
struct BlockDef {
    params: Vec<String>,
    /// Every layer declared in the block body (the block's local symbol table).
    layers: Vec<LayerDef>,
    /// The block body's dataflow structure over those layers — so a block can
    /// itself be a `residual`/`wrap`/`branch` composition, not just a flat layer
    /// stack. `Compose::Layer` leaves reference `layers` by name.
    body: Vec<Compose>,
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

/// Fold a well-known std wrapper path (`Option<T>`, `Box<T>`, `Result<T, E>`,
/// …) into its canonical sigil [`Type`]. Returns `None` for anything that
/// isn't a recognized wrapper at the exact arity — so user types and
/// wrong-arity paths fall through to `Type::Path`.
///
/// This is what makes the two surface forms (`Option<T>` and `?T`) denote the
/// *same* AST node: the verbose Rust-style spelling is accepted, but the
/// formatter re-emits the terse sigil, so canonical/idiomatic code pays the
/// sigil's lower token cost.
fn canonical_wrapper(name: &str, args: &[Type]) -> Option<Type> {
    let one = |args: &[Type]| args.first().cloned().map(Box::new);
    match (name, args.len()) {
        ("Option", 1) => Some(Type::Option { inner: one(args)? }),
        ("Box", 1) => Some(Type::OwnedPtr { inner: one(args)? }),
        ("Rc", 1) => Some(Type::Rc { inner: one(args)? }),
        ("Arc", 1) => Some(Type::Arc { inner: one(args)? }),
        ("Vec", 1) => Some(Type::Vec { inner: one(args)? }),
        ("Cell", 1) => Some(Type::Cell { inner: one(args)? }),
        ("RefCell", 1) => Some(Type::RefCell { inner: one(args)? }),
        ("Mutex", 1) => Some(Type::Mutex { inner: one(args)? }),
        ("RwLock", 1) => Some(Type::RwLock { inner: one(args)? }),
        ("Result", 2) => Some(Type::Result {
            ok: Box::new(args[0].clone()),
            err: Box::new(args[1].clone()),
        }),
        _ => None,
    }
}

pub fn parse(tokens: &[Token]) -> Result<Module, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_module()
}

/// Substitute block-macro parameters (`Ident`s) with their call-site argument
/// expressions, recursively through the common layer-arg shapes.
fn subst_expr(e: &Expr, map: &HashMap<String, Expr>) -> Expr {
    match e {
        Expr::Ident { name } => map.get(name).cloned().unwrap_or_else(|| e.clone()),
        Expr::Binary { op, left, right } => Expr::Binary {
            op: op.clone(),
            left: Box::new(subst_expr(left, map)),
            right: Box::new(subst_expr(right, map)),
        },
        Expr::Unary { op, operand } => Expr::Unary {
            op: op.clone(),
            operand: Box::new(subst_expr(operand, map)),
        },
        Expr::Call { func, args } => Expr::Call {
            func: Box::new(subst_expr(func, map)),
            args: args.iter().map(|a| subst_expr(a, map)).collect(),
        },
        other => other.clone(),
    }
}

/// Suffix every layer name (used to make each `stack`/block instance unique).
fn rename_layers(layers: &[LayerDef], suffix: &str) -> Vec<LayerDef> {
    layers
        .iter()
        .map(|l| LayerDef {
            name: format!("{}{}", l.name, suffix),
            layer_type: l.layer_type.clone(),
            args: l.args.clone(),
        })
        .collect()
}

/// Suffix every `Compose::Layer` reference (kept in lock-step with
/// [`rename_layers`] so an instance's dataflow still points at its own layers).
/// `Wrap`'s op is a layer *type*, not a declared layer — left untouched.
fn rename_compose(items: &[Compose], suffix: &str) -> Vec<Compose> {
    items.iter().map(|c| rename_compose_one(c, suffix)).collect()
}

fn rename_compose_one(c: &Compose, suffix: &str) -> Compose {
    match c {
        Compose::Layer(n) => Compose::Layer(format!("{n}{suffix}")),
        Compose::Residual(b) => Compose::Residual(rename_compose(b, suffix)),
        Compose::Wrap(op, b) => Compose::Wrap(op.clone(), rename_compose(b, suffix)),
        Compose::Branch(paths) => {
            Compose::Branch(paths.iter().map(|p| rename_compose(p, suffix)).collect())
        }
    }
}

/// Whether a compose body carries any real dataflow (a `residual`/`branch`/`wrap`
/// node) — i.e. it must drive lowering instead of plain declaration order.
fn compose_has_dataflow(items: &[Compose]) -> bool {
    items.iter().any(|c| !matches!(c, Compose::Layer(_)))
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    /// `block` macros seen so far, by name (resolved at their use site).
    blocks: HashMap<String, BlockDef>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0, blocks: HashMap::new() }
    }

    fn peek(&self) -> TokenKind {
        self.tokens.get(self.pos).map_or(TokenKind::Eof, |t| t.kind)
    }

    /// Look n tokens ahead (0-based: peek_n(0) == peek()).
    fn peek_n(&self, n: usize) -> TokenKind {
        self.tokens.get(self.pos + n).map_or(TokenKind::Eof, |t| t.kind)
    }

    /// Decide whether the next `{ … }` after `? expr` is a match-arm body
    /// (contains `pattern => expr`) or a then-block (statements). Scans
    /// forward without consuming until a `=>`, `;`, or unbalanced `}` is
    /// found. Cheap because match-arm bodies are usually shallow.
    fn is_match_arm_body(&self) -> bool {
        // Must be sitting on `{`.
        if self.peek() != TokenKind::LBrace {
            return false;
        }
        let mut depth = 1usize;
        let mut i = self.pos + 1;
        while i < self.tokens.len() && depth > 0 {
            match self.tokens[i].kind {
                TokenKind::LBrace | TokenKind::LParen | TokenKind::LBrack => depth += 1,
                TokenKind::RBrace | TokenKind::RParen | TokenKind::RBrack => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::FatArrow if depth == 1 => return true,
                TokenKind::Semi if depth == 1 => return false,
                _ => {}
            }
            i += 1;
        }
        false
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
            | TokenKind::KwReward
            | TokenKind::KwData
            | TokenKind::KwVal
            | TokenKind::KwVar
            | TokenKind::KwGuard
            | TokenKind::KwDefer
            | TokenKind::KwExtend
            | TokenKind::KwIs
            | TokenKind::KwHandle
            // Sum-type constructors are keywords in expression position but
            // need to act as identifiers in path / pattern contexts (e.g.
            // `R.Ok(x)`, `match { R.Ok(v) => ... }`).
            | TokenKind::KwOk
            | TokenKind::KwErr
            | TokenKind::KwSome
            | TokenKind::KwNone
            // `async` is reserved as KwAf (agent-mode `af` = async fn) but
            // the corpus uses `async` as a regular identifier (effect name,
            // module path, etc.). Allow it.
            | TokenKind::KwAf
            | TokenKind::KwUf
            // Additional keywords the corpus uses as plain identifiers
            // (effect-handler methods, struct field names, etc.). KwNet / KwVal /
            // KwVar / KwRule / KwQuery / KwSelect are already covered above.
            | TokenKind::KwYield => Ok(self.advance().text.clone()),
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
        ParseError {
            line: tok.span.line,
            col: tok.span.col,
            message: message.to_string(),
        }
    }

    // ── Module ──────────────────────────────────────────────

    fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut items = Vec::new();
        while self.peek() != TokenKind::Eof {
            // `block Name(params) { layer …; }` — a macro for the architecture
            // DSL. Recorded for expansion at use sites; emits no module item.
            // Contextual: `block` stays a plain identifier everywhere else.
            if self.peek() == TokenKind::Ident
                && self.current().text == "block"
                && self.peek_n(1) == TokenKind::Ident
            {
                self.parse_block_def()?;
                continue;
            }
            items.push(self.parse_item()?);
        }
        Ok(Module { items })
    }

    /// Parse and record a `block Name(p1, p2) { layer …; }` macro.
    fn parse_block_def(&mut self) -> Result<(), ParseError> {
        self.advance(); // `block`
        let name = self.expect_ident()?;
        let mut params = Vec::new();
        if self.peek() == TokenKind::LParen {
            self.advance();
            while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                params.push(self.expect_ident()?);
                if self.peek() == TokenKind::Comma {
                    self.advance();
                }
            }
            self.expect(TokenKind::RParen)?;
        }
        // The body may itself compose with `residual`/`wrap`/`branch`, so it is
        // parsed exactly like a net's dataflow body (which also collects every
        // declared layer). A block referencing another already-defined block is
        // fine; it cannot reference itself (not yet inserted).
        let mut layers = Vec::new();
        let body = self.parse_compose_body(&mut layers)?;
        self.blocks.insert(name, BlockDef { params, layers, body });
        Ok(())
    }

    /// If the cursor is on a known block reference `Name(args)`, consume it and
    /// return the block's (layers, body) with params substituted into the layer
    /// args. Names are returned unsuffixed; callers (e.g. `stack`) rename them
    /// for each instance. Returns `None` if the cursor is not a block reference.
    fn expand_block_ref(
        &mut self,
    ) -> Result<Option<(Vec<LayerDef>, Vec<Compose>)>, ParseError> {
        if self.peek() != TokenKind::Ident {
            return Ok(None);
        }
        let name = self.current().text.clone();
        if !self.blocks.contains_key(&name) {
            return Ok(None);
        }
        self.advance(); // block name
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
        let block = self.blocks[&name].clone();
        let subst: HashMap<String, Expr> = block.params.iter().cloned().zip(args).collect();
        let layers = block
            .layers
            .iter()
            .map(|l| LayerDef {
                name: l.name.clone(),
                layer_type: l.layer_type.clone(),
                args: l.args.iter().map(|a| subst_expr(a, &subst)).collect(),
            })
            .collect();
        Ok(Some((layers, block.body.clone())))
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

        Ok(Item {
            visibility,
            attributes,
            kind,
        })
    }

    fn parse_item_kind(&mut self, contracts: Vec<ContractClause>) -> Result<ItemKind, ParseError> {
        match self.peek() {
            TokenKind::KwF => self
                .parse_function_def(false, false, contracts)
                .map(ItemKind::Function),
            TokenKind::KwAf => self
                .parse_function_def(true, false, contracts)
                .map(ItemKind::Function),
            TokenKind::KwUf => self
                .parse_function_def(false, true, contracts)
                .map(ItemKind::Function),
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
            TokenKind::KwData => self.parse_data_def().map(ItemKind::Data),
            TokenKind::KwExtend => self.parse_extend_block().map(ItemKind::Extend),
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
        Ok(ContractClause {
            kind,
            condition,
            message,
        })
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

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen)?;

        let return_type = if self.peek() == TokenKind::Arrow {
            self.advance();
            let ty = self.parse_type()?;
            // T or E → Result { ok: T, err: E }
            if self.peek() == TokenKind::KwOr {
                self.advance();
                let err_ty = self.parse_type()?;
                Some(Type::Result {
                    ok: Box::new(ty),
                    err: Box::new(err_ty),
                })
            } else {
                Some(ty)
            }
        } else {
            None
        };

        let where_clause = if self.peek() == TokenKind::TildeArrow {
            self.parse_where_clause()?
        } else {
            Vec::new()
        };

        // Effect annotations:  `/ io`, `/ io + net + ...`, or
        // `/ llm, tools, io` (corpus uses both `+` and `,` as separator).
        let effects = if self.peek() == TokenKind::Slash {
            self.advance();
            let mut effs = vec![self.expect_ident()?];
            // Effects may carry generic-type arguments: `channel[i32]`.
            // We don't currently model them in the AST - parse-and-drop
            // so the rest of the signature stays well-formed.
            if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
                let close = if self.peek() == TokenKind::LBrack {
                    TokenKind::RBrack
                } else {
                    TokenKind::Gt
                };
                self.advance();
                while self.peek() != close && self.peek() != TokenKind::Eof {
                    let _ = self.parse_type()?;
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(close)?;
            }
            while self.peek() == TokenKind::Plus || self.peek() == TokenKind::Comma {
                self.advance();
                effs.push(self.expect_ident()?);
                if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
                    let close = if self.peek() == TokenKind::LBrack {
                        TokenKind::RBrack
                    } else {
                        TokenKind::Gt
                    };
                    self.advance();
                    while self.peek() != close && self.peek() != TokenKind::Eof {
                        let _ = self.parse_type()?;
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect(close)?;
                }
            }
            effs
        } else {
            Vec::new()
        };

        // Expression-body: fn name(...) -> Type = expr
        if self.peek() == TokenKind::Assign {
            self.advance();
            let expr = self.parse_expr()?;
            if self.peek() == TokenKind::Semi {
                self.advance();
            }
            return Ok(FunctionDef {
                name,
                is_async,
                is_unsafe,
                generics,
                params,
                return_type,
                where_clause,
                effects: effects.clone(),
                contracts,
                body: Block {
                    stmts: Vec::new(),
                    tail_expr: None,
                },
                body_expr: Some(Box::new(expr)),
            });
        }

        // Function body is OPTIONAL: a `fn` declaration ending in `;`
        // instead of `{ … }` is a signature-only declaration (trait
        // method signature). When the body is absent we synthesize an
        // empty block — downstream verifiers can treat it as "to be
        // implemented" rather than failing the parse.
        let body = if self.peek() == TokenKind::Semi {
            self.advance();
            crate::ast::Block {
                stmts: Vec::new(),
                tail_expr: None,
            }
        } else {
            self.parse_block()?
        };

        Ok(FunctionDef {
            name,
            is_async,
            is_unsafe,
            generics,
            params,
            return_type,
            where_clause,
            // Keep the parsed `/ effect` annotations: the block-body path
            // previously dropped them (`Vec::new()`), silently disabling effect
            // enforcement for every `f name() / io { ... }`. Now declared
            // effects flow to the checker so undeclared effects are caught.
            effects,
            contracts,
            body,
            body_expr: None,
        })
    }

    // ── Struct ──────────────────────────────────────────────

    fn parse_struct_def(
        &mut self,
        contracts: Vec<ContractClause>,
    ) -> Result<StructDef, ParseError> {
        self.expect(TokenKind::KwS)?;
        let name = self.expect_ident()?;

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
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
            fields.push(StructField {
                visibility: vis,
                name: field_name,
                ty,
            });
        }
        self.expect(TokenKind::RBrace)?;

        Ok(StructDef {
            name,
            generics,
            contracts,
            fields,
        })
    }

    // ── Enum ────────────────────────────────────────────────

    fn parse_enum_def(&mut self) -> Result<EnumDef, ParseError> {
        self.expect(TokenKind::KwE)?;
        let name = self.expect_ident()?;

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
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
            variants.push(EnumVariant {
                name: variant_name,
                kind,
            });
        }
        self.expect(TokenKind::RBrace)?;

        Ok(EnumDef {
            name,
            generics,
            variants,
        })
    }

    // ── Trait ────────────────────────────────────────────────

    fn parse_trait_def(&mut self) -> Result<TraitDef, ParseError> {
        self.expect(TokenKind::KwT)?;
        let name = self.expect_ident()?;

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
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

        Ok(TraitDef {
            name,
            generics,
            super_traits,
            items,
        })
    }

    // ── Impl ────────────────────────────────────────────────

    fn parse_impl_block(&mut self) -> Result<ImplBlock, ParseError> {
        self.expect(TokenKind::KwI)?;

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        let self_type = self.parse_type()?;

        // The lexer aliases `for` → TokenKind::At (used for the @-for
        // loop sigil too). In impl-block position, At unambiguously
        // means the `impl Trait for Type` keyword. Accept both.
        let trait_path = if matches!(self.peek(), TokenKind::KwFor | TokenKind::At) {
            self.advance();
            let _actual_type = self.parse_type()?;
            // The "self_type" was actually the trait path
            if let Type::Path { segments, .. } = &self_type {
                Some(segments.clone())
            } else {
                None
            }
        } else {
            None
        };

        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            items.push(self.parse_item()?);
        }
        self.expect(TokenKind::RBrace)?;

        Ok(ImplBlock {
            generics,
            self_type,
            trait_path,
            items,
        })
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
            Ok(ModuleDef {
                name,
                items: Some(items),
            })
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
                if self.peek() == TokenKind::Semi {
                    self.advance();
                }
                return Ok(UseDef {
                    path,
                    alias: None,
                    glob: true,
                    group: Vec::new(),
                });
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
                    group.push(UseDef {
                        path: vec![name],
                        alias,
                        glob: false,
                        group: Vec::new(),
                    });
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBrace)?;
                // Optional trailing `;` - corpus sometimes omits it.
                if self.peek() == TokenKind::Semi {
                    self.advance();
                }
                return Ok(UseDef {
                    path,
                    alias: None,
                    glob: false,
                    group,
                });
            }
            path.push(self.expect_ident()?);
        }

        if self.peek() == TokenKind::Semi {
            self.advance();
        }
        Ok(UseDef {
            path,
            alias: None,
            glob: false,
            group: Vec::new(),
        })
    }

    // ── Type Alias ──────────────────────────────────────────

    fn parse_type_alias(&mut self) -> Result<TypeAlias, ParseError> {
        // Accept both KwType ("type") and KwY ("Y")
        self.advance();
        let name = self.expect_ident()?;

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        // Body is optional: `type Output;` (associated-type declaration
        // inside a trait) is shorthand for "declared, no body". The body
        // form `type Foo = Bar;` is the standard alias.
        let ty = if self.peek() == TokenKind::Assign {
            self.advance();
            self.parse_type()?
        } else {
            // Sentinel `_` Type marks an undefined associated type.
            crate::ast::Type::Path {
                segments: vec!["_".to_string()],
                type_args: vec![],
            }
        };

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

        Ok(TypeAlias {
            name,
            generics,
            ty,
            refinement,
        })
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

        Ok(StaticDef {
            name,
            mutable,
            ty,
            value,
        })
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
        // Optional generic params on the effect itself: `effect channel[T] { ... }`.
        if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
            let _ = self.parse_generic_params()?;
        }
        self.expect(TokenKind::LBrace)?;

        let mut operations = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            self.expect(TokenKind::KwF)?;
            let op_name = self.expect_ident()?;
            // Optional generic params on the operation: `f spawn[T](...)`.
            if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
                let _ = self.parse_generic_params()?;
            }
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
            operations.push(EffectOp {
                name: op_name,
                params,
                return_type,
            });
        }

        self.expect(TokenKind::RBrace)?;
        Ok(EffectDef { name, operations })
    }

    // ── Spec ────────────────────────────────────────────────

    fn parse_spec_def(&mut self) -> Result<SpecDef, ParseError> {
        self.expect(TokenKind::KwSpec)?;
        let name = self.expect_ident()?;

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
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

        Ok(SpecDef {
            name,
            generics,
            params,
            return_type,
            items,
        })
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
                    return Err(self.error(&format!(
                        "expected agent field or `}}`, found {:?}",
                        self.peek()
                    )));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(AgentDef {
            name,
            capabilities,
            requires_approval,
        })
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
        let mut transport = None;

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
                        "transport" => {
                            self.expect(TokenKind::Colon)?;
                            transport = Some(self.expect_ident()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
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
                    return Err(self.error(&format!(
                        "expected swarm field or `}}`, found {:?}",
                        self.peek()
                    )));
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
            transport,
        })
    }

    // ── Net ─────────────────────────────────────────────────

    /// Parse `net Name[T] { layer ...; forward { ... } }`
    fn parse_net_def(&mut self) -> Result<NetDef, ParseError> {
        self.expect(TokenKind::KwNet)?;
        let name = self.expect_ident()?;

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LBrace)?;
        let mut layers = Vec::new();
        // Compose body, tracked in parallel with `layers`: plain layers and
        // `stack` expansions become `Compose::Layer` leaves; the dataflow
        // combinators wrap sub-bodies. Only emitted (as `composition: Some`)
        // when a dataflow operator actually appears — otherwise the net lowers
        // exactly as before via declaration order.
        let mut body: Vec<Compose> = Vec::new();
        let mut has_dataflow = false;
        let mut forward = None;

        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::KwLayer => {
                    self.advance();
                    let l = self.parse_layer_body()?;
                    body.push(Compose::Layer(l.name.clone()));
                    layers.push(l);
                }
                // `stack N { layer …; layer …; }` — the repeat combinator. Expands
                // to N copies of the body with names suffixed `_<i>`, so a deep
                // net costs ~one block + a count at the surface instead of N× the
                // block. (Contextual keyword: `stack` stays a plain identifier
                // elsewhere.)
                TokenKind::Ident if self.current().text == "stack" => {
                    self.advance(); // stack
                    let count: usize = if self.peek() == TokenKind::IntLiteral {
                        self.advance().text.parse().unwrap_or(0)
                    } else {
                        return Err(self.error("expected a repeat count after `stack`"));
                    };
                    // Parse the stack body ONCE as a (layers, dataflow) template,
                    // then instantiate it `count` times with names suffixed `_<i>`
                    // (both the layers and their `Compose` references), so a stack
                    // of `residual`/`wrap`/`branch` blocks keeps its structure.
                    let mut tmpl_layers = Vec::new();
                    let tmpl_body = self.parse_compose_body(&mut tmpl_layers)?;
                    if compose_has_dataflow(&tmpl_body) {
                        has_dataflow = true;
                    }
                    for i in 0..count {
                        let sfx = format!("_{i}");
                        layers.extend(rename_layers(&tmpl_layers, &sfx));
                        body.extend(rename_compose(&tmpl_body, &sfx));
                    }
                }
                // `residual { … }` — wrap the body in `x + f(x)` (RMIL RES_ADD).
                TokenKind::Ident if self.current().text == "residual" => {
                    self.advance();
                    has_dataflow = true;
                    let inner = self.parse_compose_body(&mut layers)?;
                    body.push(Compose::Residual(inner));
                }
                // `wrap Op { … }` — sandwich the body: `Op >> body >> Op` (e.g. norm).
                TokenKind::Ident if self.current().text == "wrap" => {
                    self.advance();
                    has_dataflow = true;
                    let op = self.expect_ident()?;
                    let inner = self.parse_compose_body(&mut layers)?;
                    body.push(Compose::Wrap(op, inner));
                }
                // `branch { … } { … }` — parallel paths (RMIL PAR), one `{}` each.
                TokenKind::Ident if self.current().text == "branch" => {
                    self.advance();
                    has_dataflow = true;
                    let mut paths = Vec::new();
                    while self.peek() == TokenKind::LBrace {
                        paths.push(self.parse_compose_body(&mut layers)?);
                    }
                    if paths.is_empty() {
                        return Err(self.error("`branch` needs at least one `{ … }` path"));
                    }
                    body.push(Compose::Branch(paths));
                }
                TokenKind::KwForward => {
                    self.advance();
                    forward = Some(self.parse_block()?);
                }
                TokenKind::Semi | TokenKind::Comma => {
                    self.advance();
                }
                // A `block` reference used directly in the net body.
                TokenKind::Ident if self.blocks.contains_key(&self.current().text) => {
                    if let Some((blk_layers, blk_body)) = self.expand_block_ref()? {
                        if compose_has_dataflow(&blk_body) {
                            has_dataflow = true;
                        }
                        layers.extend(blk_layers);
                        body.extend(blk_body);
                    }
                }
                _ => {
                    return Err(self.error(&format!(
                        "expected `layer`, `forward`, a block reference, or `}}` in net, found {:?}",
                        self.peek()
                    )));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        let forward = forward.unwrap_or(Block {
            stmts: Vec::new(),
            tail_expr: None,
        });
        Ok(NetDef {
            name,
            generics,
            layers,
            forward,
            composition: if has_dataflow { Some(body) } else { None },
        })
    }

    /// Parse a `{ … }` body for a dataflow combinator (`residual`/`branch`/`wrap`),
    /// returning its [`Compose`] items and appending every layer it declares to
    /// `layers_out`. Handles `layer`, block references, and nested combinators —
    /// so combinators compose arbitrarily (e.g. `residual { wrap Norm { … } }`).
    fn parse_compose_body(
        &mut self,
        layers_out: &mut Vec<LayerDef>,
    ) -> Result<Vec<Compose>, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::KwLayer => {
                    self.advance();
                    let l = self.parse_layer_body()?;
                    items.push(Compose::Layer(l.name.clone()));
                    layers_out.push(l);
                }
                TokenKind::Semi | TokenKind::Comma => {
                    self.advance();
                }
                TokenKind::Ident if self.current().text == "residual" => {
                    self.advance();
                    items.push(Compose::Residual(self.parse_compose_body(layers_out)?));
                }
                TokenKind::Ident if self.current().text == "wrap" => {
                    self.advance();
                    let op = self.expect_ident()?;
                    items.push(Compose::Wrap(op, self.parse_compose_body(layers_out)?));
                }
                TokenKind::Ident if self.current().text == "branch" => {
                    self.advance();
                    let mut paths = Vec::new();
                    while self.peek() == TokenKind::LBrace {
                        paths.push(self.parse_compose_body(layers_out)?);
                    }
                    if paths.is_empty() {
                        return Err(self.error("`branch` needs at least one `{ … }` path"));
                    }
                    items.push(Compose::Branch(paths));
                }
                TokenKind::Ident if self.blocks.contains_key(&self.current().text) => {
                    if let Some((blk_layers, blk_body)) = self.expand_block_ref()? {
                        layers_out.extend(blk_layers);
                        items.extend(blk_body);
                    }
                }
                _ => {
                    return Err(self.error(
                        "expected `layer`, a block reference, or a nested combinator inside `{ … }`",
                    ))
                }
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(items)
    }

    /// Parse the body of a layer declaration — `name: Type(args)` plus an
    /// optional trailing `;` — after the `layer` keyword has been consumed.
    /// Shared by plain `layer …` and the `stack N { … }` combinator.
    fn parse_layer_body(&mut self) -> Result<LayerDef, ParseError> {
        let name = self.expect_ident()?;
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
        Ok(LayerDef { name, layer_type, args })
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

                    // Optional conditions: `where expr, expr, ...` (the lexer
                    // maps the `where` keyword to TildeArrow, same as `~>`).
                    let mut conditions = Vec::new();
                    if self.peek() == TokenKind::TildeArrow {
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
                    rules.push(RuleDef {
                        name: rname,
                        params,
                        conditions,
                        body,
                    });
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

        let fitness = fitness.unwrap_or(Block {
            stmts: Vec::new(),
            tail_expr: None,
        });
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
        let mut inputs = None;
        let mut targets = None;
        let mut dataset = None;
        let mut val_split = None;
        let mut checkpoint = None;
        let mut batch_size = None;
        let mut patience = None;
        let mut prompt = None;
        let mut max_tokens = None;
        let mut temperature = None;
        let mut top_k = None;
        let mut top_p = None;
        let mut seed = None;
        let mut clip_grad = None;
        let mut warmup_steps = None;
        let mut lr_schedule = None;
        let mut weight_decay = None;
        let mut tied_embeddings = None;
        let mut plateau_patience = None;
        let mut lr_factor = None;

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
                        "inputs" => {
                            self.expect(TokenKind::Colon)?;
                            inputs = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "targets" => {
                            self.expect(TokenKind::Colon)?;
                            targets = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "dataset" => {
                            self.expect(TokenKind::Colon)?;
                            dataset = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "val_split" => {
                            self.expect(TokenKind::Colon)?;
                            val_split = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "checkpoint" => {
                            self.expect(TokenKind::Colon)?;
                            checkpoint = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "batch_size" => {
                            self.expect(TokenKind::Colon)?;
                            batch_size = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "patience" => {
                            self.expect(TokenKind::Colon)?;
                            patience = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "prompt" => {
                            self.expect(TokenKind::Colon)?;
                            prompt = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "max_tokens" => {
                            self.expect(TokenKind::Colon)?;
                            max_tokens = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "temperature" => {
                            self.expect(TokenKind::Colon)?;
                            temperature = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "top_k" => {
                            self.expect(TokenKind::Colon)?;
                            top_k = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "top_p" => {
                            self.expect(TokenKind::Colon)?;
                            top_p = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "seed" => {
                            self.expect(TokenKind::Colon)?;
                            seed = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "clip_grad" => {
                            self.expect(TokenKind::Colon)?;
                            clip_grad = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "warmup_steps" => {
                            self.expect(TokenKind::Colon)?;
                            warmup_steps = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "lr_schedule" => {
                            self.expect(TokenKind::Colon)?;
                            lr_schedule = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "weight_decay" => {
                            self.expect(TokenKind::Colon)?;
                            weight_decay = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "tied_embeddings" => {
                            self.expect(TokenKind::Colon)?;
                            tied_embeddings = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "plateau_patience" => {
                            self.expect(TokenKind::Colon)?;
                            plateau_patience = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
                        }
                        "lr_factor" => {
                            self.expect(TokenKind::Colon)?;
                            lr_factor = Some(self.parse_expr()?);
                            if self.peek() == TokenKind::Semi {
                                self.advance();
                            }
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
                    return Err(self.error(&format!(
                        "expected train field or `}}`, found {:?}",
                        self.peek()
                    )));
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        let body = body.unwrap_or(Block {
            stmts: Vec::new(),
            tail_expr: None,
        });
        Ok(TrainDef {
            name,
            net,
            optimizer,
            loss,
            epochs,
            body,
            inputs,
            targets,
            dataset,
            val_split,
            checkpoint,
            batch_size,
            patience,
            prompt,
            max_tokens,
            temperature,
            top_k,
            top_p,
            seed,
            clip_grad,
            warmup_steps,
            lr_schedule,
            weight_decay,
            tied_embeddings,
            plateau_patience,
            lr_factor,
        })
    }

    // ── Data Definition ─────────────────────────────────────

    fn parse_data_def(&mut self) -> Result<DataDef, ParseError> {
        self.advance(); // consume 'data' / 'D'
        let name = self.expect_ident()?;

        let generics = if matches!(self.peek(), TokenKind::LBrack | TokenKind::Lt) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        if self.peek() == TokenKind::LParen {
            // Record form: data Name(field: Type, ...)
            self.advance();
            let mut fields = Vec::new();
            while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                let fname = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                let default = if self.peek() == TokenKind::Assign {
                    self.advance();
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                fields.push(DataField {
                    name: fname,
                    ty,
                    default,
                });
                if self.peek() == TokenKind::Comma {
                    self.advance();
                }
            }
            self.expect(TokenKind::RParen)?;
            Ok(DataDef {
                name,
                generics,
                kind: DataKind::Record(fields),
            })
        } else if self.peek() == TokenKind::Assign {
            // Sum type: data Name = Variant1 | Variant2(Type)
            self.advance();
            let mut variants = Vec::new();
            loop {
                let vname = self.expect_ident()?;
                let fields = if self.peek() == TokenKind::LParen {
                    self.advance();
                    let mut types = Vec::new();
                    while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                        types.push(self.parse_type()?);
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    types
                } else {
                    Vec::new()
                };
                variants.push(DataVariant {
                    name: vname,
                    fields,
                });
                if self.peek() == TokenKind::BitOr {
                    self.advance();
                } else {
                    break;
                }
            }
            Ok(DataDef {
                name,
                generics,
                kind: DataKind::Sum(variants),
            })
        } else {
            // Empty record: data Unit
            Ok(DataDef {
                name,
                generics,
                kind: DataKind::Record(Vec::new()),
            })
        }
    }

    // ── Extend Block ────────────────────────────────────────

    fn parse_extend_block(&mut self) -> Result<ExtendBlock, ParseError> {
        self.advance(); // consume 'extend' / 'xd'
        let target_type = self.parse_type()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
            items.push(self.parse_item()?);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(ExtendBlock { target_type, items })
    }

    // ── Guard Statement ─────────────────────────────────────

    fn parse_guard_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume 'guard' / 'gd'
        let cond = self.parse_expr()?;
        if self.peek() != TokenKind::KwElse
            && self.peek() != TokenKind::KwOr
            && self.peek() != TokenKind::Colon
        {
            return Err(self.error("expected 'else' after guard condition"));
        }
        self.advance();
        let else_block = self.parse_block()?;
        Ok(Stmt::Guard { cond, else_block })
    }

    // ── Defer Statement ─────────────────────────────────────

    fn parse_defer_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance(); // consume 'defer' / 'df'
        let expr = self.parse_expr()?;
        if self.peek() == TokenKind::Semi {
            self.advance();
        }
        Ok(Stmt::Defer { expr })
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
        // Accept either `[T, ...]` (MAGE native) or `<T, ...>`
        // (Rust-style). The corpus mixes both styles.
        let (open, close) = match self.peek() {
            TokenKind::LBrack => (TokenKind::LBrack, TokenKind::RBrack),
            TokenKind::Lt => (TokenKind::Lt, TokenKind::Gt),
            _ => return Err(self.error("expected `[` or `<` to open generic params")),
        };
        self.expect(open)?;
        let mut params = Vec::new();

        while self.peek() != close && self.peek() != TokenKind::Eof {
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

            params.push(GenericParam {
                name,
                bounds,
                default,
            });

            if self.peek() == TokenKind::Comma {
                self.advance();
            }
        }

        self.expect(close)?;
        Ok(params)
    }

    // ── Param List ──────────────────────────────────────────

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();

        while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
            // Method-receiver shorthand: `&self`, `&!self`, `self` — no
            // explicit type. Desugar to a `self` param with a synthetic
            // reference type. The downstream type-checker resolves `Self`.
            let (name, ty) = match self.peek() {
                TokenKind::BitAnd if self.peek_n(1) == TokenKind::Ident
                    && self.current_at(self.pos + 1).text == "self" =>
                {
                    self.advance(); // &
                    self.advance(); // self
                    (
                        "self".to_string(),
                        Type::Reference {
                            mutable: false,
                            inner: Box::new(Type::Path {
                                segments: vec!["Self".to_string()],
                                type_args: Vec::new(),
                            }),
                        },
                    )
                }
                TokenKind::AndNot if self.peek_n(1) == TokenKind::Ident
                    && self.current_at(self.pos + 1).text == "self" =>
                {
                    self.advance(); // &!
                    self.advance(); // self
                    (
                        "self".to_string(),
                        Type::Reference {
                            mutable: true,
                            inner: Box::new(Type::Path {
                                segments: vec!["Self".to_string()],
                                type_args: Vec::new(),
                            }),
                        },
                    )
                }
                // Bare `self` (by-value receiver). Desugars to `self: Self`
                // - the type-checker resolves `Self` to the enclosing
                // impl's self_type. Matches Rust's bare-self syntax.
                TokenKind::Ident if self.current().text == "self"
                    && matches!(self.peek_n(1), TokenKind::Comma | TokenKind::RParen) =>
                {
                    self.advance(); // self
                    (
                        "self".to_string(),
                        Type::Path {
                            segments: vec!["Self".to_string()],
                            type_args: Vec::new(),
                        },
                    )
                }
                // Builder-pattern shorthand: `!self` = `&!self`. Used by
                // chaining builders that mutate-in-place and return Self.
                TokenKind::Bang if self.peek_n(1) == TokenKind::Ident
                    && self.current_at(self.pos + 1).text == "self" =>
                {
                    self.advance(); // !
                    self.advance(); // self
                    (
                        "self".to_string(),
                        Type::Reference {
                            mutable: true,
                            inner: Box::new(Type::Path {
                                segments: vec!["Self".to_string()],
                                type_args: Vec::new(),
                            }),
                        },
                    )
                }
                _ => {
                    let name = self.expect_ident()?;
                    // The param type is optional — omit it to infer it from use
                    // (`f add(a, b) { a + b }`). Annotated params are unchanged.
                    let ty = if self.peek() == TokenKind::Colon {
                        self.advance();
                        self.parse_type()?
                    } else {
                        Type::Inferred
                    };
                    (name, ty)
                }
            };
            let default = if self.peek() == TokenKind::Assign {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            params.push(Param { name, ty, default });

            if self.peek() == TokenKind::Comma {
                self.advance();
            }
        }

        Ok(params)
    }

    /// Peek the token at an absolute position. Used for two-token-deep
    /// pattern checks (e.g. `&` followed by `self`).
    fn current_at(&self, pos: usize) -> &Token {
        &self.tokens[pos.min(self.tokens.len() - 1)]
    }

    /// Parse a closure parameter list — like `parse_param_list` but each
    /// param's type annotation is optional (`fn(x) => …`, `fn(x: i32) => …`).
    fn parse_closure_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
            let name = self.expect_ident()?;
            let ty = if self.peek() == TokenKind::Colon {
                self.advance();
                self.parse_type()?
            } else {
                // Untyped — inferred at use site.
                Type::Inferred
            };
            params.push(Param { name, ty, default: None });
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
                Ok(Type::Reference {
                    mutable: false,
                    inner: Box::new(inner),
                })
            }
            TokenKind::AndNot => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Reference {
                    mutable: true,
                    inner: Box::new(inner),
                })
            }

            // ^T (owned ptr)
            TokenKind::BitXor => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::OwnedPtr {
                    inner: Box::new(inner),
                })
            }

            // $T (Rc)
            TokenKind::Dollar => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Rc {
                    inner: Box::new(inner),
                })
            }

            // @T (Arc) — README documents this; was missing from type parser.
            TokenKind::At => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Arc {
                    inner: Box::new(inner),
                })
            }

            // f(T1, T2, ...) -> R — function type. Lexer treats `f` and
            // `af` and `uf` as fn-keyword variants; in type position they
            // all open a Type::Fn.
            TokenKind::KwF | TokenKind::KwAf | TokenKind::KwUf => {
                self.advance();
                self.expect(TokenKind::LParen)?;
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

            // &~T (Cow)
            TokenKind::AndTilde => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Cow {
                    inner: Box::new(inner),
                })
            }

            // %T (Cell) / %!T (RefCell)
            TokenKind::Percent => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Cell {
                    inner: Box::new(inner),
                })
            }
            TokenKind::PercentNot => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::RefCell {
                    inner: Box::new(inner),
                })
            }

            // #T (Mutex) / #~T (RwLock)
            TokenKind::Hash => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Mutex {
                    inner: Box::new(inner),
                })
            }
            TokenKind::HashTilde => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::RwLock {
                    inner: Box::new(inner),
                })
            }

            // ?T (Option)
            TokenKind::Question => {
                self.advance();
                let inner = self.parse_type()?;
                Ok(Type::Option {
                    inner: Box::new(inner),
                })
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
                    Ok(Type::Array {
                        inner: Box::new(inner),
                        size: Box::new(size),
                    })
                } else {
                    self.expect(TokenKind::RBrack)?;
                    if self.peek() == TokenKind::Tilde {
                        self.advance();
                        Ok(Type::Vec {
                            inner: Box::new(inner),
                        })
                    } else {
                        Ok(Type::Slice {
                            inner: Box::new(inner),
                        })
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
                    Ok(Type::Map {
                        key: Box::new(key),
                        value: Box::new(value),
                    })
                } else {
                    self.expect(TokenKind::RBrace)?;
                    Ok(Type::Set {
                        inner: Box::new(key),
                    })
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
                // `dyn Trait` prefix: erase the dyn-ness for parsing
                // purposes and parse the next ident as a normal type
                // path. Matches Rust syntax; type-checker treats it
                // structurally identical to a bare trait-object name.
                if self.peek() == TokenKind::Ident && self.current().text == "dyn" {
                    self.advance();
                    return self.parse_type();
                }
                let name = self.advance().text.clone();

                match name.as_str() {
                    "R" if self.peek() == TokenKind::LBrack => {
                        self.advance();
                        let ok = self.parse_type()?;
                        self.expect(TokenKind::Comma)?;
                        let err = self.parse_type()?;
                        self.expect(TokenKind::RBrack)?;
                        Ok(Type::Result {
                            ok: Box::new(ok),
                            err: Box::new(err),
                        })
                    }
                    "Ptr" if self.peek() == TokenKind::LBrack => {
                        self.advance();
                        let inner = self.parse_type()?;
                        self.expect(TokenKind::RBrack)?;
                        Ok(Type::Ptr {
                            inner: Box::new(inner),
                        })
                    }
                    "Simd" if self.peek() == TokenKind::LBrack => {
                        self.advance();
                        let inner = self.parse_type()?;
                        self.expect(TokenKind::Comma)?;
                        let width_tok = self.expect(TokenKind::IntLiteral)?;
                        let width: u64 = width_tok.text.parse().unwrap_or(0);
                        self.expect(TokenKind::RBrack)?;
                        Ok(Type::Simd {
                            inner: Box::new(inner),
                            width,
                        })
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

                        // Accept either `Path[T, ...]` (MAGE) or
                        // `Path<T, ...>` (Rust-style) type-arg lists.
                        let (open, close) = match self.peek() {
                            TokenKind::LBrack => (Some(TokenKind::LBrack), TokenKind::RBrack),
                            TokenKind::Lt => (Some(TokenKind::Lt), TokenKind::Gt),
                            _ => (None, TokenKind::RBrack),
                        };
                        let type_args = if let Some(open_tok) = open {
                            self.advance();
                            let _ = open_tok;
                            let mut args = Vec::new();
                            while self.peek() != close && self.peek() != TokenKind::Eof {
                                args.push(self.parse_type()?);
                                if self.peek() == TokenKind::Comma {
                                    self.advance();
                                }
                            }
                            self.expect(close)?;
                            args
                        } else {
                            Vec::new()
                        };

                        // Canonicalize well-known std wrapper paths into their
                        // terse sigil AST forms. Either surface compiles, but
                        // the formatter then emits the canonical sigil
                        // (`Option<T>` → `?T`), so idiomatic code carries the
                        // sigil's lower token cost. Folding only fires for a
                        // single bare segment with the exact arity — a
                        // user-defined `mymod.Option<T>` (dotted) or wrong arity
                        // stays a plain path.
                        if segments.len() == 1 {
                            if let Some(folded) = canonical_wrapper(&segments[0], &type_args) {
                                return Ok(folded);
                            }
                        }
                        Ok(Type::Path {
                            segments,
                            type_args,
                        })
                    }
                }
            }

            // (`f(T, T) -> T` fn types are handled by the KwF | KwAf | KwUf arm above.)

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
                Ok(Type::Tensor {
                    inner: Box::new(inner),
                    shape,
                })
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
                Ok(Type::ParamTy {
                    inner: Box::new(inner),
                    shape,
                })
            }

            // Genome[T]
            TokenKind::KwGenome => {
                self.advance();
                self.expect(TokenKind::LBrack)?;
                let inner = self.parse_type()?;
                self.expect(TokenKind::RBrack)?;
                Ok(Type::Genome {
                    inner: Box::new(inner),
                })
            }

            // Policy[State, Action]
            TokenKind::KwPolicy => {
                self.advance();
                self.expect(TokenKind::LBrack)?;
                let state = self.parse_type()?;
                self.expect(TokenKind::Comma)?;
                let action = self.parse_type()?;
                self.expect(TokenKind::RBrack)?;
                Ok(Type::Policy {
                    state: Box::new(state),
                    action: Box::new(action),
                })
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
        if self.peek() == TokenKind::LBrace {
            self.advance();
            let block = self.parse_block_body(None)?;
            self.expect(TokenKind::RBrace)?;
            return Ok(block);
        }
        // Offside-rule layout (step 1b): a body on a new line, no braces. The
        // block's statements share the indentation of its first token; a token
        // that dedents below that column (or EOF / an enclosing `}`) ends it.
        // Braced code never reaches here (the `{` path runs first), so existing
        // programs are unaffected.
        if self.newline_before_current() {
            let base_col = self.current().span.col;
            return self.parse_block_body(Some(base_col));
        }
        // Same line, no brace → preserve the existing "expected `{`" error.
        self.expect(TokenKind::LBrace)?;
        unreachable!("expect(LBrace) on a non-brace token returns Err above")
    }

    /// At the end of a block: `}` (braced) or EOF / enclosing `}` / a dedent
    /// below `col` (layout).
    fn at_block_end(&self, layout_col: Option<usize>) -> bool {
        match layout_col {
            None => self.peek() == TokenKind::RBrace,
            Some(col) => {
                self.peek() == TokenKind::Eof
                    || self.peek() == TokenKind::RBrace
                    || self.current().span.col < col
            }
        }
    }

    /// Parse a block's statements. `None` = braced (bounded by `}`); `Some(col)`
    /// = layout (bounded by a dedent below `col`). The braced path reproduces the
    /// original behaviour exactly.
    fn parse_block_body(&mut self, layout_col: Option<usize>) -> Result<Block, ParseError> {
        let mut stmts = Vec::new();
        let mut tail_expr = None;

        while self.peek() != TokenKind::Eof && !self.at_block_end(layout_col) {
            // Try to parse a statement
            match self.peek() {
                // `let` was removed from MAGE — bindings use `val`
                // (immutable) or `var` (mutable). Reject with a clear,
                // actionable diagnostic instead of a cryptic cascade.
                TokenKind::KwLet => {
                    let tok = self.current();
                    return Err(ParseError {
                        line: tok.span.line,
                        col: tok.span.col,
                        message: "`let` is not a MAGE keyword — use `val` for an immutable \
                                  binding or `var` for a mutable one (e.g. `val x = 1;` / \
                                  `var x = 1;`)"
                            .to_string(),
                    });
                }
                TokenKind::KwV | TokenKind::KwM | TokenKind::KwVal | TokenKind::KwVar
                    if self.is_let_statement() =>
                {
                    stmts.push(self.parse_let_stmt()?);
                }
                TokenKind::KwGuard => {
                    stmts.push(self.parse_guard_stmt()?);
                }
                TokenKind::KwDefer => {
                    stmts.push(self.parse_defer_stmt()?);
                }
                // Nested function declaration: `f name(…){…}`. Disambiguated from
                // a `fn(…) => …` closure (also KwF) by the following token — a
                // name means a declaration, `(` means a closure expression.
                TokenKind::KwF | TokenKind::KwAf | TokenKind::KwUf
                    if matches!(
                        self.tokens.get(self.pos + 1).map(|t| t.kind),
                        Some(TokenKind::Ident)
                    ) =>
                {
                    let item = self.parse_item()?;
                    stmts.push(Stmt::Item { item: Box::new(item) });
                }
                _ => {
                    let expr = self.parse_expr()?;
                    if self.peek() == TokenKind::Semi {
                        self.advance();
                        stmts.push(Stmt::Expr { expr });
                    } else if self.at_block_end(layout_col) {
                        tail_expr = Some(Box::new(expr));
                        break;
                    } else {
                        stmts.push(Stmt::Expr { expr });
                    }
                }
            }
        }

        Ok(Block { stmts, tail_expr })
    }

    /// Peek inside a leading `{ ... }` to decide whether it's a map
    /// literal or a block. Caller must be positioned ON the `{`.
    ///
    /// Rules:
    /// - `{}`              -> map literal (empty map)
    /// - `{ ident: ...`    -> map literal (depth-0 `:` before `;`)
    /// - `{ "str": ...`    -> map literal
    /// - `{ stmt;`         -> block (depth-0 `;` first)
    /// - `{ expr }`        -> block (no `:`, no `;`)
    fn is_map_literal(&self) -> bool {
        if self.peek() != TokenKind::LBrace {
            return false;
        }
        // Look at the very next token; if RBrace, it's an empty map literal.
        if self.peek_n(1) == TokenKind::RBrace {
            return true;
        }
        // If the first non-trivia token is a let-binding keyword or
        // statement-start keyword, it's a block - NOT a map literal.
        // Catches the common ambiguity `{ v x: T = ... }` where the
        // depth-0 `:` belongs to a type annotation, not a map entry.
        if matches!(
            self.peek_n(1),
            TokenKind::KwV
                | TokenKind::KwM
                | TokenKind::KwVal
                | TokenKind::KwVar
                | TokenKind::KwGuard
                | TokenKind::KwDefer
                | TokenKind::KwRet
                | TokenKind::KwYield
        ) {
            return false;
        }
        // Scan depth-0 tokens until matching `}`. First `:` => map, first
        // `;` => block, RBrace without `:` => block.
        let mut depth = 0i32;
        let mut i = 1usize; // start AFTER the opening `{`
        loop {
            let kind = match self.tokens.get(self.pos + i) {
                Some(t) => t.kind,
                None => return false,
            };
            match kind {
                TokenKind::LBrace | TokenKind::LParen | TokenKind::LBrack => depth += 1,
                TokenKind::RBrace if depth == 0 => return false,
                TokenKind::RBrace | TokenKind::RParen | TokenKind::RBrack => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                }
                TokenKind::Colon if depth == 0 => return true,
                TokenKind::Semi if depth == 0 => return false,
                TokenKind::FatArrow if depth == 0 => return false, // looks like match arm
                TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }
    }

    fn is_let_statement(&self) -> bool {
        // v/m/val/var followed by an identifier or a keyword that can
        // legally serve as a binding name (val, var, count, etc).
        // The lexer eagerly tokenises `val` -> KwVal even when it
        // appears as a binding NAME (`m val = 1`), so consecutive
        // let-statements like `v a = 0; v val = 1;` previously parsed
        // the first then failed the second. Accept the most common
        // keyword-but-identifier-like next tokens here.
        let first = self.peek();
        if !matches!(
            first,
            TokenKind::KwV | TokenKind::KwM | TokenKind::KwVal | TokenKind::KwVar
        ) {
            return false;
        }
        let next = match self.tokens.get(self.pos + 1).map(|t| t.kind) {
            Some(k) => k,
            None => return false,
        };
        // Destructuring binders `val (a, b) = …` / `val [h, ..t] = …` are only
        // recognised after the LONG keywords `val`/`var`. After the short forms
        // `v`/`m` a following `(`/`[` is a variable call/index (`m[k] = …`,
        // `v(x)`) — those `v`/`m` are ordinary variable names, not binders.
        if matches!(next, TokenKind::LParen | TokenKind::LBrack) {
            return matches!(first, TokenKind::KwVal | TokenKind::KwVar);
        }
        matches!(
            next,
            TokenKind::Ident
                    | TokenKind::Underscore
                    // Keywords that double as identifiers in
                    // binding-name position. The lexer tokenises
                    // common variable names (`val`, `guard`, `data`,
                    // `query`, etc.) as their keyword variants - any
                    // of these can legally appear as a binding name,
                    // so peek-ahead must treat them as ident-like.
                    | TokenKind::KwVal
                    | TokenKind::KwVar
                    | TokenKind::KwData
                    | TokenKind::KwGuard
                    | TokenKind::KwDefer
                    | TokenKind::KwQuery
                    | TokenKind::KwRule
                    | TokenKind::KwFact
                    | TokenKind::KwSelect
                    | TokenKind::KwYield
                    | TokenKind::KwOk
                    | TokenKind::KwErr
                    | TokenKind::KwSome
                    | TokenKind::KwNone
                    | TokenKind::KwIs
                    | TokenKind::KwLayer
                    | TokenKind::KwTensor
                    | TokenKind::KwParam
                    | TokenKind::KwForward
                    | TokenKind::KwReward
                    | TokenKind::KwPolicy
                    | TokenKind::KwFitness
                    | TokenKind::KwGenome
                    | TokenKind::KwMutate
        )
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        // Binding keyword: `v`/`val` (immutable) or `m`/`var` (mutable).
        // `let` was removed from the language — it's rejected earlier in the
        // statement dispatcher with a "use val/var" hint, so it never reaches
        // here.
        let mutable = matches!(self.peek(), TokenKind::KwM | TokenKind::KwVar);
        self.advance(); // consume the binding keyword

        let pattern = self.parse_pattern()?;

        let ty = if self.peek() == TokenKind::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(TokenKind::Assign)?;
        let value = self.parse_expr()?;
        self.expect_stmt_end()?;

        Ok(Stmt::Let {
            mutable,
            pattern,
            ty,
            value,
        })
    }

    /// True when the current token starts a new source line relative to the
    /// previously consumed token (i.e. a newline separates them).
    fn newline_before_current(&self) -> bool {
        if self.pos == 0 {
            return false;
        }
        match (self.tokens.get(self.pos - 1), self.tokens.get(self.pos)) {
            (Some(prev), Some(cur)) => cur.span.line > prev.span.line,
            _ => false,
        }
    }

    /// Consume a statement terminator (offside-rule layout). A `;` is consumed
    /// when present, but is **optional** when the statement is already ended by a
    /// newline, a closing `}`, or EOF — so `val x = 5` on its own line needs no
    /// `;`. A same-line statement with no separator is still an error.
    fn expect_stmt_end(&mut self) -> Result<(), ParseError> {
        if self.peek() == TokenKind::Semi {
            self.advance();
            Ok(())
        } else if matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof)
            || self.newline_before_current()
        {
            Ok(())
        } else {
            let tok = self.current();
            Err(ParseError {
                line: tok.span.line,
                col: tok.span.col,
                message: "expected `;` or a newline to end the statement".to_string(),
            })
        }
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
                    // Postfix `?` (try operator) does NOT apply to
                    // control-flow expressions whose `}` just closed.
                    // Two consecutive if-stmts like `? a {}\n? b {}`
                    // would otherwise be parsed as `(if a {})?(b ...)`,
                    // shadowing the second if. Same for match / loop /
                    // for / while / plain blocks.
                    let is_control_flow = matches!(
                        lhs,
                        Expr::If { .. }
                            | Expr::Match { .. }
                            | Expr::Loop { .. }
                            | Expr::For { .. }
                            | Expr::While { .. }
                            | Expr::Block { .. }
                    );
                    // A `?` that begins a new line is the next statement's `if`
                    // (the lexer emits `if` as Question too), not a postfix try
                    // on `lhs`. A real try (`first(xs)?`) hugs its operand with
                    // no intervening newline. Without this, `7\n if c {} else {}`
                    // parses as `(7?) ...` and the `else` later dangles.
                    if is_control_flow || self.newline_before_current() {
                        break;
                    }
                    self.advance();
                    lhs = Expr::Try {
                        expr: Box::new(lhs),
                    };
                    continue;
                }
                // Postfix tensor ops: ⊤ (transpose), ⊥ (flatten)
                TokenKind::TensorTranspose => {
                    self.advance();
                    lhs = Expr::Unary {
                        op: "⊤".to_string(),
                        operand: Box::new(lhs),
                    };
                    continue;
                }
                TokenKind::TensorFlatten => {
                    self.advance();
                    lhs = Expr::Unary {
                        op: "⊥".to_string(),
                        operand: Box::new(lhs),
                    };
                    continue;
                }
                TokenKind::Dot => {
                    self.advance();
                    let field = self.expect_ident()?;
                    // Postfix `.await` (Rust-style). Contextual — `await` stays a
                    // plain identifier, so this never shadows a real field/var; we
                    // only treat it as await when it isn't a `.await(...)` call.
                    if field == "await" && self.peek() != TokenKind::LParen {
                        lhs = Expr::Await { expr: Box::new(lhs) };
                        continue;
                    }
                    // Optional turbofish `::<T, ...>` for generic
                    // method calls. Detect Colon-Colon-Lt (lexer
                    // emits `::` as two separate Colons).
                    let mut type_args: Vec<Type> = Vec::new();
                    if self.peek() == TokenKind::Colon
                        && self.peek_n(1) == TokenKind::Colon
                        && self.peek_n(2) == TokenKind::Lt
                    {
                        self.advance(); // :
                        self.advance(); // :
                        self.advance(); // <
                        while self.peek() != TokenKind::Gt && self.peek() != TokenKind::Eof {
                            type_args.push(self.parse_type()?);
                            if self.peek() == TokenKind::Comma {
                                self.advance();
                            }
                        }
                        self.expect(TokenKind::Gt)?;
                    }
                    if self.peek() == TokenKind::LParen {
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
                    } else if !type_args.is_empty() {
                        // Turbofish without call - unusual but treat
                        // as a field access on the typed method ref;
                        // drop the type args for now.
                        lhs = Expr::FieldAccess {
                            object: Box::new(lhs),
                            field,
                        };
                    } else {
                        lhs = Expr::FieldAccess {
                            object: Box::new(lhs),
                            field,
                        };
                    }
                    continue;
                }
                TokenKind::LBrack => {
                    self.advance();
                    // Range slicing inside index brackets — supports
                    //   arr[a..b]   arr[a..=b]   arr[a..]   arr[..b]   arr[..]
                    // Bare-`..` start / end use Expr::Range with a sentinel
                    // `Expr::Ident { name: "_" }` (downstream type-checker
                    // recognises `_` as unbounded; this keeps the AST shape
                    // uniform without inventing a new variant).
                    let open_sentinel =
                        || Expr::Ident { name: "_".to_string() };
                    let (start, has_start) =
                        if matches!(self.peek(), TokenKind::DotDot | TokenKind::DotDotEq) {
                            (open_sentinel(), false)
                        } else {
                            (self.parse_expr()?, true)
                        };
                    let index = if matches!(self.peek(), TokenKind::DotDot | TokenKind::DotDotEq) {
                        let inclusive = self.peek() == TokenKind::DotDotEq;
                        self.advance();
                        // End may be absent: `arr[a..]`.
                        let end = if self.peek() == TokenKind::RBrack {
                            open_sentinel()
                        } else {
                            self.parse_expr()?
                        };
                        Expr::Range {
                            start: Box::new(start),
                            end: Box::new(end),
                            inclusive,
                        }
                    } else if !has_start {
                        // shouldn't reach — `_` placeholder without a `..`
                        open_sentinel()
                    } else {
                        start
                    };
                    self.expect(TokenKind::RBrack)?;
                    lhs = Expr::Index {
                        object: Box::new(lhs),
                        index: Box::new(index),
                    };
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
                    lhs = Expr::Call {
                        func: Box::new(lhs),
                        args,
                    };
                    continue;
                }
                TokenKind::KwIs => {
                    self.advance();
                    let pattern = self.parse_pattern()?;
                    lhs = Expr::Is {
                        expr: Box::new(lhs),
                        pattern,
                    };
                    continue;
                }
                // `expr as Type` — Rust-style cast. The lexer leaves `as`
                // as a plain Ident (no KwAs), so we match on text.
                TokenKind::Ident if self.current().text == "as" => {
                    self.advance();
                    let ty = self.parse_type()?;
                    lhs = Expr::Cast {
                        expr: Box::new(lhs),
                        ty,
                    };
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
                TokenKind::Pipe => ("|>", 3, 4),
                _ => break,
            };

            if l_bp < min_bp {
                break;
            }

            self.advance();

            // Handle pipeline
            if op == "|>" {
                let rhs = self.parse_expr_bp(r_bp)?;
                lhs = Expr::Pipeline {
                    left: Box::new(lhs),
                    right: Box::new(rhs),
                };
                continue;
            }

            // Handle assignment
            if op == "=" {
                let rhs = self.parse_expr_bp(r_bp)?;
                lhs = Expr::Assign {
                    target: Box::new(lhs),
                    value: Box::new(rhs),
                };
                continue;
            }

            let rhs = self.parse_expr_bp(r_bp)?;
            lhs = Expr::Binary {
                op: op.to_string(),
                left: Box::new(lhs),
                right: Box::new(rhs),
            };
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
                Ok(Expr::Unary {
                    op,
                    operand: Box::new(operand),
                })
            }
            TokenKind::BitAnd => {
                let tok = self.advance();
                let op = tok.text.clone();
                let operand = self.parse_prefix_expr()?;
                Ok(Expr::Unary {
                    op,
                    operand: Box::new(operand),
                })
            }

            // Mutable borrow: `&!x` in expression position. Previously
            // only handled in type position (`&!T`); now also valid as
            // a prefix operator producing a mutable reference. Encoded
            // as Unary { op: "&!", ... } so downstream visitors can
            // distinguish from the shared-ref `&` form.
            TokenKind::AndNot => {
                self.advance();
                let operand = self.parse_prefix_expr()?;
                Ok(Expr::Unary {
                    op: "&!".to_string(),
                    operand: Box::new(operand),
                })
            }

            // Return
            TokenKind::KwRet => {
                self.advance();
                if matches!(
                    self.peek(),
                    TokenKind::Semi
                        | TokenKind::RBrace
                        | TokenKind::Comma
                        | TokenKind::RParen
                        | TokenKind::RBrack
                        | TokenKind::Eof
                ) {
                    Ok(Expr::Return { value: None })
                } else {
                    let val = self.parse_expr()?;
                    Ok(Expr::Return {
                        value: Some(Box::new(val)),
                    })
                }
            }

            // Break (KwBreak keyword — note: `!` as break is handled via
            // context in statement parsing; in expr prefix position `!` is unary-not)
            TokenKind::KwBreak => {
                self.advance();
                // `break` with no value when the next token can't start
                // an expression: `;`, `}`, `,` (match arm separator),
                // `)` / `]` (param/index close).
                if matches!(
                    self.peek(),
                    TokenKind::Semi
                        | TokenKind::RBrace
                        | TokenKind::Comma
                        | TokenKind::RParen
                        | TokenKind::RBrack
                        | TokenKind::Eof
                ) {
                    Ok(Expr::Break { value: None })
                } else {
                    let val = self.parse_expr()?;
                    Ok(Expr::Break {
                        value: Some(Box::new(val)),
                    })
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
                Ok(Expr::Match {
                    scrutinee: Some(Box::new(scrutinee)),
                    arms,
                })
            }

            // If: ? expr { ... } : { ... } or ? { match_arm, ... }
            TokenKind::Question => {
                // Sum-type literal sugar: `?Some(x)` / `?None` / `?Ok(v)` /
                // `?Err(e)`. The lexer emits Question then KwSome/etc.
                // Peek the next token before committing to if/match parsing.
                if matches!(
                    self.peek_n(1),
                    TokenKind::KwSome | TokenKind::KwNone | TokenKind::KwOk | TokenKind::KwErr
                ) {
                    self.advance(); // ?
                    let name = self.advance().text.clone();
                    if self.peek() == TokenKind::LParen {
                        self.advance();
                        let mut args = Vec::new();
                        while self.peek() != TokenKind::RParen
                            && self.peek() != TokenKind::Eof
                        {
                            args.push(self.parse_expr()?);
                            if self.peek() == TokenKind::Comma {
                                self.advance();
                            }
                        }
                        self.expect(TokenKind::RParen)?;
                        return Ok(Expr::Call {
                            func: Box::new(Expr::Ident { name }),
                            args,
                        });
                    }
                    return Ok(Expr::Ident { name });
                }
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
                    Ok(Expr::Match {
                        scrutinee: None,
                        arms,
                    })
                } else {
                    // If expression: ? cond { ... } : { ... }
                    // OR match expression: ? scrutinee { pat => body, ... }
                    let cond = self.parse_expr()?;
                    // Disambiguate by scanning the next `{ … }`: if it
                    // contains a `=>` at depth 1, treat as match arms.
                    if self.is_match_arm_body() {
                        self.advance(); // consume `{`
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
                        return Ok(Expr::Match {
                            scrutinee: Some(Box::new(cond)),
                            arms,
                        });
                    }
                    let then_block = self.parse_block()?;
                    let else_block = if self.peek() == TokenKind::Colon
                        || self.peek() == TokenKind::KwOr
                        || self.peek() == TokenKind::KwElse
                    {
                        self.advance();
                        // Else-if shorthand: `} : ? cond { ... }` wraps
                        // the nested if-expression in a synthetic block
                        // so the else-branch stays a Block (matching
                        // Rust's `} else if ...` desugaring).
                        if self.peek() == TokenKind::Question {
                            let nested_if = self.parse_prefix_expr()?;
                            Some(crate::ast::Block {
                                stmts: Vec::new(),
                                tail_expr: Some(Box::new(nested_if)),
                            })
                        } else {
                            Some(self.parse_block()?)
                        }
                    } else {
                        None
                    };
                    Ok(Expr::If {
                        cond: Box::new(cond),
                        then_block,
                        else_block,
                    })
                }
            }

            // @-prefixed expressions:
            //   @TypeName { field: value, ... }  → Arc-wrapped struct lit
            //                                       (we drop the Arc-wrap for parse;
            //                                        downstream gets a StructLit)
            //   @TypeName.method(...)            → Arc method-receiver path
            //   @ pattern : iter { ... }         → for-loop (canonical)
            //   each pattern of iter { ... }     → for-loop (human mode)
            TokenKind::At => {
                // Lookahead: is this `@ Ident {` (struct lit) or
                // `@ Ident .` (path expr starting with Arc)? Both are
                // expression-position uses; otherwise fall through to
                // for-loop parsing.
                if self.peek_n(1) == TokenKind::Ident
                    && (self.peek_n(2) == TokenKind::LBrace
                        || self.peek_n(2) == TokenKind::Dot)
                {
                    self.advance(); // consume @
                    let mut path = vec![self.expect_ident()?];
                    while self.peek() == TokenKind::Dot && self.peek_n(1) == TokenKind::Ident {
                        // Stop before `.method(` calls — those are postfix.
                        if self.peek_n(2) == TokenKind::LParen {
                            break;
                        }
                        self.advance();
                        path.push(self.expect_ident()?);
                    }
                    if self.peek() == TokenKind::LBrace {
                        // Struct literal: @Path { field: value, ... }
                        self.advance();
                        let mut fields = Vec::new();
                        while self.peek() != TokenKind::RBrace
                            && self.peek() != TokenKind::Eof
                        {
                            let name = self.expect_ident()?;
                            let value = if self.peek() == TokenKind::Colon {
                                self.advance();
                                Some(self.parse_expr()?)
                            } else {
                                None // shorthand: `{ name }` = `{ name: name }`
                            };
                            fields.push(FieldInit { name, value });
                            if self.peek() == TokenKind::Comma {
                                self.advance();
                            }
                        }
                        self.expect(TokenKind::RBrace)?;
                        return Ok(Expr::StructLit { path, fields });
                    }
                    // @Path.method(...) — emit as nested field access
                    // (the Arc-wrap is dropped at parse).
                    let mut expr = Expr::Ident { name: path.remove(0) };
                    for seg in path {
                        expr = Expr::FieldAccess { object: Box::new(expr), field: seg };
                    }
                    return Ok(expr);
                }
                self.advance(); // consume @ for for-loop
                let pattern = self.parse_pattern()?;
                if self.peek() == TokenKind::KwOf {
                    self.advance();
                } else {
                    self.expect(TokenKind::Colon)?;
                }
                // For-loop iter accepts a `start..end` or `start..=end`
                // range expression as a context-specific extension.
                // Top-level `..` is NOT a generic infix operator (it
                // conflicts with match-arm range patterns and struct
                // update syntax), so we special-case it here.
                let start_expr = self.parse_expr()?;
                let iter = if matches!(self.peek(), TokenKind::DotDot | TokenKind::DotDotEq) {
                    let inclusive = self.peek() == TokenKind::DotDotEq;
                    self.advance();
                    let end_expr = self.parse_expr()?;
                    Expr::Range {
                        start: Box::new(start_expr),
                        end: Box::new(end_expr),
                        inclusive,
                    }
                } else {
                    start_expr
                };
                let body = self.parse_block()?;
                Ok(Expr::For {
                    pattern,
                    iter: Box::new(iter),
                    body,
                })
            }

            // While loop: @w cond { ... }
            TokenKind::AtW => {
                self.advance();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Expr::While {
                    cond: Box::new(cond),
                    body,
                })
            }

            // Infinite loop: @@ { ... } (also legacy "loop" keyword)
            TokenKind::AtAt | TokenKind::KwLoop => {
                self.advance();
                let body = self.parse_block()?;
                Ok(Expr::Loop { body })
            }

            // Block expression OR map literal. Disambiguate by
            // peeking inside the braces:
            //   `{}`           - empty map literal (block returning ()
            //                    is rare standalone; corpus uses {} for maps)
            //   `{ key: val }` - map literal (at-depth-0 `:` before `;`)
            //   `{ stmt; ... }` - block (at-depth-0 `;` first)
            //   `{ expr }`     - block returning expr (no `:`, no `;`)
            TokenKind::LBrace if self.is_map_literal() => {
                self.advance(); // consume {
                let mut entries: Vec<(Expr, Expr)> = Vec::new();
                while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
                    let key = self.parse_expr()?;
                    self.expect(TokenKind::Colon)?;
                    let val = self.parse_expr()?;
                    entries.push((key, val));
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Expr::MapLit { entries })
            }

            // Block expression
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Expr::Block { block })
            }

            // Keywords that double as identifiers in expression-prefix
            // position. The lexer reserves these tokens, but the corpus
            // uses them as plain variable names / call targets (e.g.
            // `func(&!guard)`, `handle.spawn(...)`, `net.fetch(...)`).
            // Same shape as the sum-type-constructor arm below: parse
            // as a plain Ident, with an optional call-args suffix so
            // `guard(x)` works too.
            TokenKind::KwGuard
            | TokenKind::KwHandle
            | TokenKind::KwNet
            | TokenKind::KwDefer
            | TokenKind::KwQuery
            | TokenKind::KwRule
            | TokenKind::KwFact
            | TokenKind::KwSelect
            | TokenKind::KwYield
            | TokenKind::KwData
            | TokenKind::KwLayer
            | TokenKind::KwTensor
            | TokenKind::KwParam
            | TokenKind::KwForward
            | TokenKind::KwReward
            | TokenKind::KwPolicy
            | TokenKind::KwFitness
            | TokenKind::KwGenome
            | TokenKind::KwMutate
            // `v` and `m` are let-binding keywords in statement
            // position, but appear as plain identifiers in match-arm
            // bodies, call args, and similar (e.g.
            // `?Some(v) => v.clone()`). The block-stmt dispatcher
            // catches `v`/`m` first via is_let_statement when they
            // do start a let; falling through here handles the rest.
            | TokenKind::KwV
            | TokenKind::KwM
            | TokenKind::KwVal
            | TokenKind::KwVar => {
                let name = self.advance().text.clone();
                Ok(Expr::Ident { name })
            }

            // Sum-type constructors as expressions:
            //   Some(x), None, Ok(v), Err(e)
            // Lexer reserves these; parser was rejecting them in
            // expression-prefix position despite the pattern arm above.
            TokenKind::KwSome | TokenKind::KwNone | TokenKind::KwOk | TokenKind::KwErr => {
                let name = self.advance().text.clone();
                if self.peek() == TokenKind::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                        args.push(self.parse_expr()?);
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    Ok(Expr::Call {
                        func: Box::new(Expr::Ident { name }),
                        args,
                    })
                } else {
                    Ok(Expr::Ident { name })
                }
            }

            // Closure: fn(params) => expr   (params may be untyped)
            TokenKind::KwF
                if matches!(
                    self.tokens.get(self.pos + 1).map(|t| t.kind),
                    Some(TokenKind::LParen)
                ) =>
            {
                self.advance(); // fn
                self.advance(); // (
                let params = self.parse_closure_param_list()?;
                self.expect(TokenKind::RParen)?;
                // Body can be either `=> expr` (arrow form) or a
                // braced block (Rust-style closure body). The latter
                // is common when the closure has multiple statements.
                let body = if self.peek() == TokenKind::FatArrow {
                    self.advance();
                    self.parse_expr()?
                } else if self.peek() == TokenKind::LBrace {
                    let block = self.parse_block()?;
                    Expr::Block { block }
                } else {
                    return Err(
                        self.error("expected `=>` or `{` for closure body")
                    );
                };
                Ok(Expr::Closure {
                    params,
                    body: Box::new(body),
                })
            }

            // Array literal
            TokenKind::LBrack => {
                self.advance();
                if self.peek() == TokenKind::RBrack {
                    self.advance();
                    return Ok(Expr::ArrayLit {
                        elements: Vec::new(),
                    });
                }
                let first = self.parse_expr()?;
                if self.peek() == TokenKind::Semi {
                    // Array repeat: [expr; count]
                    self.advance();
                    let count = self.parse_expr()?;
                    self.expect(TokenKind::RBrack)?;
                    Ok(Expr::ArrayRepeat {
                        value: Box::new(first),
                        count: Box::new(count),
                    })
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
                    return Ok(Expr::TupleLit {
                        elements: Vec::new(),
                    });
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
                Ok(Expr::Literal {
                    value: tok.text.clone(),
                    kind,
                })
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
                Ok(Expr::Literal {
                    value: tok.text.clone(),
                    kind,
                })
            }
            TokenKind::CharLiteral => {
                let tok = self.advance();
                Ok(Expr::Literal {
                    value: tok.text.clone(),
                    kind: LiteralKind::Char,
                })
            }
            TokenKind::True | TokenKind::False => {
                let tok = self.advance();
                Ok(Expr::Literal {
                    value: tok.text.clone(),
                    kind: LiteralKind::Bool,
                })
            }

            // Identifiers — plus the keyword-as-ident tokens we
            // similarly permit in `expect_ident`. Lets `val`, `var`,
            // `async`, `unsafe` etc. flow through expression-prefix
            // when used as identifiers (variable names, args, etc.).
            TokenKind::Ident
            | TokenKind::KwAf
            | TokenKind::KwUf => {
                // (KwVal/KwVar/KwData/KwYield/KwRule/KwQuery/KwSelect as
                // identifiers are already handled by earlier arms.)
                let tok = self.advance();
                Ok(Expr::Ident {
                    name: tok.text.clone(),
                })
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(Expr::Ident {
                    name: "_".to_string(),
                })
            }
            TokenKind::UnderscoreT => {
                self.advance();
                Ok(Expr::Ident {
                    name: "_T".to_string(),
                })
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
                // Path pattern: Color.Red, R.Ok(x), some::path::Variant(args)
                let mut path = vec![name];
                while self.peek() == TokenKind::Dot {
                    self.advance();
                    path.push(self.expect_ident()?);
                }
                // Constructor pattern: Name(...) or Path.Variant(...)
                if self.peek() == TokenKind::LParen {
                    self.advance();
                    let mut elements = Vec::new();
                    while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                        elements.push(self.parse_pattern()?);
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    Ok(Pattern::Enum { path, elements })
                } else if path.len() == 1 {
                    Ok(Pattern::Ident { name: path.into_iter().next().unwrap() })
                } else {
                    // Multi-segment path without args = unit variant.
                    Ok(Pattern::Enum { path, elements: Vec::new() })
                }
            }
            // `?Some(x)` / `?None` / `?Ok(v)` / `?Err(e)` — same sum-type
            // sugar as in expressions. Skip the leading `?` and fall through
            // to the constructor-pattern arm below.
            TokenKind::Question if matches!(
                self.peek_n(1),
                TokenKind::KwSome | TokenKind::KwNone | TokenKind::KwOk | TokenKind::KwErr,
            ) => {
                self.advance(); // consume ?
                self.parse_pattern()
            }
            // Some/None/Ok/Err can appear as constructor patterns
            TokenKind::KwSome | TokenKind::KwNone | TokenKind::KwOk | TokenKind::KwErr => {
                let name = self.advance().text.clone();
                if self.peek() == TokenKind::LParen {
                    self.advance();
                    let mut elements = Vec::new();
                    while self.peek() != TokenKind::RParen && self.peek() != TokenKind::Eof {
                        elements.push(self.parse_pattern()?);
                        if self.peek() == TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    Ok(Pattern::Enum {
                        path: vec![name],
                        elements,
                    })
                } else {
                    Ok(Pattern::Enum {
                        path: vec![name],
                        elements: vec![],
                    })
                }
            }
            TokenKind::IntLiteral
            | TokenKind::FloatLiteral
            | TokenKind::StringLiteral
            | TokenKind::CharLiteral
            | TokenKind::True
            | TokenKind::False => {
                let tok = self.advance();
                Ok(Pattern::Literal {
                    value: tok.text.clone(),
                })
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
            // Slice patterns: `[a, b]` (exact), `[a, ..]` (prefix, anonymous
            // rest), `[head, ..tail]` (named rest binds the remaining elements).
            TokenKind::LBrack => {
                self.advance();
                let mut elements = Vec::new();
                let mut rest = false;
                let mut rest_name = None;
                while self.peek() != TokenKind::RBrack && self.peek() != TokenKind::Eof {
                    if self.peek() == TokenKind::DotDot {
                        self.advance();
                        rest = true;
                        if self.peek() == TokenKind::Ident {
                            rest_name = Some(self.advance().text.clone());
                        }
                    } else {
                        elements.push(self.parse_pattern()?);
                    }
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBrack)?;
                Ok(Pattern::Slice { elements, rest, rest_name })
            }
            // Struct pattern: `@Point { x, y }` / `@Point { x: px, y: 0 }`. The
            // `@` mirrors struct-literal syntax (`@Point { x: 3 }`) and makes the
            // pattern unambiguous — it can never be confused with a match-arm or
            // `is`-condition block, which the bare-`Name { … }` form would be.
            TokenKind::At => {
                self.advance(); // consume @
                let mut path = vec![self.expect_ident()?];
                while self.peek() == TokenKind::Dot {
                    self.advance();
                    path.push(self.expect_ident()?);
                }
                self.expect(TokenKind::LBrace)?;
                let mut fields = Vec::new();
                while self.peek() != TokenKind::RBrace && self.peek() != TokenKind::Eof {
                    let name = self.expect_ident()?;
                    let pattern = if self.peek() == TokenKind::Colon {
                        self.advance();
                        Some(self.parse_pattern()?)
                    } else {
                        None // shorthand `{ x }` binds the field to `x`
                    };
                    fields.push(FieldPattern { name, pattern });
                    if self.peek() == TokenKind::Comma {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Pattern::Struct { path, fields })
            }
            _ => {
                let tok = self.advance();
                Ok(Pattern::Ident {
                    name: tok.text.clone(),
                })
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

    fn try_parse(source: &str) -> Result<Module, ParseError> {
        parse(&lexer::lex(source))
    }

    // ── Offside-rule layout blocks (migration step 1b) ──────────────────
    // A block body on a new line, indented, with no braces. Braced code is
    // unchanged (the `{` path runs first); only newline-introduced bodies use
    // the column-tracked layout path.

    #[test]
    fn layout_braced_block_unchanged() {
        // Regression sentinel — explicit braces still parse.
        assert_eq!(parse_source("f sq(n) { n * n }").items.len(), 1);
    }

    #[test]
    fn layout_single_expr_body() {
        let m = parse_source("f sq(n)\n  n * n\n");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert_eq!(f.body.stmts.len(), 0);
            assert!(f.body.tail_expr.is_some(), "`n * n` is the tail expr");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn layout_multi_statement_body() {
        let m = parse_source("f area(w, h)\n  val a = w * h\n  a\n");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert_eq!(f.body.stmts.len(), 1, "one `val` statement");
            assert!(f.body.tail_expr.is_some(), "`a` is the tail");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn layout_two_functions_dedent_boundary() {
        // The first fn's layout body must end where the second fn begins.
        let m = parse_source("f sq(n)\n  n * n\nf cube(n)\n  n * n * n\n");
        assert_eq!(m.items.len(), 2);
    }

    #[test]
    fn layout_nested_if_else() {
        let m = parse_source("f sign(n)\n  if n > 0\n    1\n  else\n    2\n");
        assert_eq!(m.items.len(), 1);
    }

    #[test]
    fn layout_mixed_brace_inside_layout() {
        let m = parse_source("f sign(n)\n  if n > 0 { 1 } else { 2 }\n");
        assert_eq!(m.items.len(), 1);
    }

    #[test]
    fn layout_same_line_without_brace_still_errors() {
        // No brace and body on the SAME line is still a syntax error.
        assert!(try_parse("f sq(n) n * n").is_err());
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
            assert_eq!(
                f.contracts[0].message.as_deref(),
                Some("n must be positive")
            );
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
            let req_count = s
                .items
                .iter()
                .filter(|i| matches!(i, SpecItem::Require(_)))
                .count();
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
    fn test_parse_net_stack_combinator() {
        // `stack N { … }` expands at parse time to N copies with `_<i>` suffixes,
        // so a deep net costs ~one block at the surface but lowers to the full
        // layer list. Here: a 2-layer body × 3 = 6 layers.
        let src = r#"net Deep {
            stack 3 {
                layer attn: Attention(64, 4);
                layer norm: LayerNorm;
            }
            forward { attn_0 }
        }"#;
        let module = parse_source(src);
        if let ItemKind::Net(ref nd) = module.items[0].kind {
            assert_eq!(nd.layers.len(), 6, "3 × 2-layer body");
            assert_eq!(nd.layers[0].name, "attn_0");
            assert_eq!(nd.layers[1].name, "norm_0");
            assert_eq!(nd.layers[2].name, "attn_1");
            assert_eq!(nd.layers[5].name, "norm_2");
            // body args are preserved per copy
            assert_eq!(nd.layers[4].name, "attn_2");
        } else {
            panic!("expected net def");
        }
    }

    #[test]
    fn test_parse_block_macro_and_reference() {
        // A `block` macro is recorded (emits no item) and expands at its use
        // site with params substituted; `stack` repeats it.
        let src = r#"block TB(d, ff) {
            layer attn: Attention(d, 4);
            layer ff1: Linear(d, ff);
        }
        net GPT {
            stack 3 { TB(64, 256) }
            forward { attn_0 }
        }"#;
        let module = parse_source(src);
        // The block emits no module item — only the net remains.
        assert_eq!(module.items.len(), 1, "block macro should not emit an item");
        if let ItemKind::Net(ref nd) = module.items[0].kind {
            assert_eq!(nd.layers.len(), 6, "3 × 2-layer block");
            assert_eq!(nd.layers[0].name, "attn_0");
            assert_eq!(nd.layers[1].name, "ff1_0");
            assert_eq!(nd.layers[2].name, "attn_1");
            // params substituted: Linear(d, ff) → Linear(64, 256)
            if let Expr::Literal { value, .. } = &nd.layers[1].args[0] {
                assert_eq!(value, "64", "d → 64");
            } else {
                panic!("expected substituted literal for d");
            }
            if let Expr::Literal { value, .. } = &nd.layers[1].args[1] {
                assert_eq!(value, "256", "ff → 256");
            } else {
                panic!("expected substituted literal for ff");
            }
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

    // ── New language features ────────────────────────────────────────

    #[test]
    fn test_data_record() {
        let m = parse_source("data Point(x: f64, y: f64)");
        assert_eq!(m.items.len(), 1);
        if let ItemKind::Data(ref dd) = m.items[0].kind {
            assert_eq!(dd.name, "Point");
            match &dd.kind {
                DataKind::Record(fields) => {
                    assert_eq!(fields.len(), 2);
                    assert_eq!(fields[0].name, "x");
                    assert_eq!(fields[1].name, "y");
                }
                _ => panic!("expected record kind"),
            }
        } else {
            panic!("expected data def");
        }
    }

    #[test]
    fn test_data_sum() {
        let m = parse_source("data Tree = Leaf | Branch(Tree, Tree)");
        if let ItemKind::Data(ref dd) = m.items[0].kind {
            assert_eq!(dd.name, "Tree");
            match &dd.kind {
                DataKind::Sum(variants) => {
                    assert_eq!(variants.len(), 2);
                    assert_eq!(variants[0].name, "Leaf");
                    assert_eq!(variants[1].name, "Branch");
                    assert_eq!(variants[1].fields.len(), 2);
                }
                _ => panic!("expected sum kind"),
            }
        } else {
            panic!("expected data def");
        }
    }

    #[test]
    fn test_data_agent_alias() {
        // D is the agent-mode alias for data
        let m = parse_source("D Point(x: f64, y: f64)");
        assert!(matches!(m.items[0].kind, ItemKind::Data(_)));
    }

    #[test]
    fn test_extend_block() {
        let m = parse_source("extend Point { f distance() -> f64 { 0.0 } }");
        if let ItemKind::Extend(ref eb) = m.items[0].kind {
            assert!(
                matches!(&eb.target_type, Type::Path { segments, .. } if segments[0] == "Point")
            );
            assert_eq!(eb.items.len(), 1);
        } else {
            panic!("expected extend block");
        }
    }

    #[test]
    fn test_extend_agent_alias() {
        let m = parse_source("xd Point { f foo() -> i32 { 0 } }");
        assert!(matches!(m.items[0].kind, ItemKind::Extend(_)));
    }

    #[test]
    fn test_val_binding() {
        let m = parse_source("f main() { val x = 42; }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            if let Stmt::Let { mutable, .. } = &f.body.stmts[0] {
                assert!(!mutable, "val should not be mutable");
            } else {
                panic!("expected let stmt");
            }
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_var_binding() {
        let m = parse_source("f main() { var x = 0; }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            if let Stmt::Let { mutable, .. } = &f.body.stmts[0] {
                assert!(mutable, "var should be mutable");
            } else {
                panic!("expected let stmt");
            }
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_block_body_keeps_declared_effects() {
        // Regression: the block-body function path dropped `/ effect`
        // annotations (`effects: Vec::new()`), silently disabling effect
        // enforcement. They must survive into the AST.
        let m = parse_source("f x() / io { greet(); }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert_eq!(f.effects, vec!["io".to_string()], "declared effect lost");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_let_keyword_is_rejected() {
        // `let` was removed from MAGE — bindings use `val`/`var`. A stray
        // `let` must fail with an actionable diagnostic, not parse silently.
        let tokens = lexer::lex("f main() { let x = 0; }");
        let err = parse(&tokens).expect_err("`let` must be rejected");
        assert!(
            err.message.contains("val") && err.message.contains("var"),
            "diagnostic should point at val/var, got: {}",
            err.message
        );
    }

    #[test]
    fn test_guard_stmt() {
        let m = parse_source("f foo(x: i32) { guard x > 0 else { return 0; } }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert!(matches!(f.body.stmts[0], Stmt::Guard { .. }));
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_guard_agent_alias() {
        let m = parse_source("f foo(x: i32) { gd x > 0 else { return 0; } }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert!(matches!(f.body.stmts[0], Stmt::Guard { .. }));
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_defer_stmt() {
        let m = parse_source("f foo() { defer close(); }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert!(matches!(f.body.stmts[0], Stmt::Defer { .. }));
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_pipeline_operator() {
        let m = parse_source("f main() { x |> foo |> bar }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            if let Some(ref tail) = f.body.tail_expr {
                assert!(matches!(tail.as_ref(), Expr::Pipeline { .. }));
            } else {
                panic!("expected tail expression");
            }
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_is_pattern() {
        let m = parse_source("f check(x: ?i32) -> bool { x is Some(_) }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            if let Some(ref tail) = f.body.tail_expr {
                assert!(
                    matches!(tail.as_ref(), Expr::Is { .. }),
                    "expected Is, got {:?}",
                    tail
                );
            } else {
                panic!("expected tail expression, body: {:?}", f.body);
            }
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_expression_body_function() {
        let m = parse_source("f double(x: i32) -> i32 = x * 2");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert!(f.body_expr.is_some(), "should have body_expr");
            assert!(f.body.stmts.is_empty(), "block body should be empty");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_default_param() {
        let m = parse_source("f greet(name: String = \"world\") { }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert!(f.params[0].default.is_some(), "should have default value");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_error_union_type() {
        let m = parse_source("f read() -> String or IoError { }");
        if let ItemKind::Function(ref f) = m.items[0].kind {
            assert!(matches!(f.return_type, Some(Type::Result { .. })));
        } else {
            panic!("expected function");
        }
    }
}
