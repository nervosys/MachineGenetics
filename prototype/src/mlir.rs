/// MLIR dialect output for the MAGE compiler.
///
/// Generates textual MLIR in the `MAGE` dialect from a type-checked,
/// effect-annotated AST.  This is the final stage of the prototype
/// pipeline:  Lex → Parse → Resolve → TypeCheck → EffectInfer → MLIR.
use crate::ast;
use crate::effects::EffectInfer;
use crate::hir::EffectSet;
use std::collections::HashMap;

/// Emit MLIR textual representation for a checked module.
pub fn emit(module: &ast::Module, effects: &EffectInfer) -> String {
    let mut ctx = EmitCtx {
        buf: String::with_capacity(4096),
        indent: 0,
        ssa: 0,
        effects: &effects.inferred,
    };

    ctx.line("// ──────────────────────────────────────────────────────────────");
    ctx.line("// Auto-generated MAGE MLIR  ·  dialect: MAGE");
    ctx.line("// https://github.com/nervosys/MAGE");
    ctx.line("// ──────────────────────────────────────────────────────────────");
    ctx.line("");
    ctx.line("module {");
    ctx.indent += 1;

    for item in &module.items {
        ctx.emit_item(item);
    }

    ctx.indent -= 1;
    ctx.line("}");

    ctx.buf
}

// ── Emit context ─────────────────────────────────────────────────────

struct EmitCtx<'a> {
    buf: String,
    indent: usize,
    ssa: usize,
    effects: &'a HashMap<String, EffectSet>,
}

impl<'a> EmitCtx<'a> {
    fn fresh(&mut self) -> String {
        let n = self.ssa;
        self.ssa += 1;
        format!("%{n}")
    }

    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.buf.push_str("  ");
        }
        self.buf.push_str(s);
        self.buf.push('\n');
    }

    // ── Items ────────────────────────────────────────────────────────

    fn emit_item(&mut self, item: &ast::Item) {
        match &item.kind {
            ast::ItemKind::Function(f) => self.emit_function(f, &item.visibility),
            ast::ItemKind::Struct(s) => self.emit_struct(s),
            ast::ItemKind::Enum(e) => self.emit_enum(e),
            ast::ItemKind::Trait(t) => self.emit_trait(t),
            ast::ItemKind::Impl(i) => self.emit_impl(i),
            ast::ItemKind::TypeAlias(a) => {
                let ty = self.mlir_type(&a.ty);
                self.line(&format!("// type {} = {ty}", a.name));
            }
            ast::ItemKind::Const(c) => {
                let ty = self.mlir_type(&c.ty);
                self.line(&format!("MAGE.const @{} : {ty}", c.name));
            }
            ast::ItemKind::Module(m) => {
                self.line(&format!("// module {}", m.name));
            }
            ast::ItemKind::Use(u) => {
                self.line(&format!("// use {}", u.path.join(".")));
            }
            ast::ItemKind::Effect(ef) => self.emit_effect_decl(ef),
            ast::ItemKind::Spec(sp) => self.emit_spec(sp),
            ast::ItemKind::Static(sd) => {
                let ty = self.mlir_type(&sd.ty);
                self.line(&format!("MAGE.static @{} : {ty}", sd.name));
            }
            ast::ItemKind::Agent(ad) => self.emit_agent(ad),
            ast::ItemKind::Net(n) => self.emit_net(n),
            ast::ItemKind::Kb(k) => self.emit_kb(k),
            ast::ItemKind::Evolve(e) => self.emit_evolve(e),
            ast::ItemKind::Train(t) => self.emit_train(t),
            ast::ItemKind::Swarm(s) => self.emit_swarm(s),
            ast::ItemKind::Data(d) => {
                self.line(&format!("// data {}", d.name));
            }
            ast::ItemKind::Extend(e) => {
                self.line("// extend block");
                for item in &e.items {
                    self.emit_item(item);
                }
            }
        }
    }

    fn emit_function(&mut self, f: &ast::FunctionDef, vis: &ast::Visibility) {
        let name = &f.name;
        let vis_prefix = match vis {
            ast::Visibility::Public => "pub ",
            ast::Visibility::Private => "",
        };

        // Parameter list.
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| {
                let ty = self.mlir_type(&p.ty);
                let ssa = self.fresh();
                format!("{ssa}: {ty} /* {} */", p.name)
            })
            .collect();

        let ret_ty = f
            .return_type
            .as_ref()
            .map(|t| self.mlir_type(t))
            .unwrap_or_else(|| "!MAGE.unit".into());

        // Effect set attribute.
        let fx = self.effects.get(name).cloned().unwrap_or_default();
        let fx_attr = if fx.is_empty() {
            String::new()
        } else {
            let effects: Vec<String> = fx.iter().map(|e| format!("\"{e}\"")).collect();
            format!(" attributes {{ effects = [{}] }}", effects.join(", "))
        };

        self.line(&format!(
            "MAGE.func {vis_prefix}@{name}({}) -> {ret_ty}{fx_attr} {{",
            params.join(", ")
        ));

        self.indent += 1;
        let entry = self.fresh();
        self.line(&format!("{entry}:  // entry"));
        self.indent += 1;

        // Emit contract ops (requires/ensures) as first-class MLIR ops.
        for contract in &f.contracts {
            let kind = match contract.kind {
                ast::ContractClauseKind::Requires => "requires",
                ast::ContractClauseKind::Ensures => "ensures",
                ast::ContractClauseKind::Invariant => "invariant",
            };
            self.line(&format!(
                "MAGE.contract.{kind} \"{}\"",
                contract.condition
            ));
        }

        // Emit performance annotations from custom effects with "perf:" prefix.
        if let Some(fx) = self.effects.get(name) {
            for e in fx.iter() {
                let label = e.to_string();
                if label.starts_with("perf:") {
                    self.line(&format!("MAGE.perf \"{}\"", &label[5..]));
                }
            }
        }

        self.emit_block(&f.body);

        let ret_val = self.fresh();
        self.line(&format!("MAGE.return {ret_val} : {ret_ty}"));

        self.indent -= 1;
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_struct(&mut self, s: &ast::StructDef) {
        self.line(&format!("MAGE.struct @{} {{", s.name));
        self.indent += 1;
        for field in &s.fields {
            let ty = self.mlir_type(&field.ty);
            self.line(&format!("MAGE.field \"{}\" : {ty}", field.name));
        }
        for c in &s.contracts {
            let kind = match c.kind {
                ast::ContractClauseKind::Invariant => "invariant",
                ast::ContractClauseKind::Requires => "requires",
                ast::ContractClauseKind::Ensures => "ensures",
            };
            self.line(&format!("MAGE.contract.{kind} \"{}\"", c.condition));
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_enum(&mut self, e: &ast::EnumDef) {
        self.line(&format!("MAGE.enum @{} {{", e.name));
        self.indent += 1;
        for variant in &e.variants {
            match &variant.kind {
                ast::VariantKind::Unit => {
                    self.line(&format!("MAGE.variant \"{}\"", variant.name));
                }
                ast::VariantKind::Tuple(types) => {
                    let tys: Vec<String> = types.iter().map(|t| self.mlir_type(t)).collect();
                    self.line(&format!(
                        "MAGE.variant \"{}\"({})",
                        variant.name,
                        tys.join(", ")
                    ));
                }
                ast::VariantKind::Struct(fields) => {
                    let fs: Vec<String> = fields
                        .iter()
                        .map(|f| {
                            let ty = self.mlir_type(&f.ty);
                            format!("{}: {ty}", f.name)
                        })
                        .collect();
                    self.line(&format!(
                        "MAGE.variant \"{}\" {{ {} }}",
                        variant.name,
                        fs.join(", ")
                    ));
                }
            }
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_trait(&mut self, t: &ast::TraitDef) {
        let supers = if t.super_traits.is_empty() {
            String::new()
        } else {
            format!(" : {}", t.super_traits.join(" + "))
        };
        self.line(&format!("MAGE.trait @{}{supers} {{", t.name));
        self.indent += 1;
        for item in &t.items {
            if let ast::ItemKind::Function(f) = &item.kind {
                let ret = f
                    .return_type
                    .as_ref()
                    .map(|t| self.mlir_type(t))
                    .unwrap_or_else(|| "!MAGE.unit".into());
                let params: Vec<String> = f.params.iter().map(|p| self.mlir_type(&p.ty)).collect();
                self.line(&format!(
                    "MAGE.method_decl \"{}\"({}) -> {ret}",
                    f.name,
                    params.join(", ")
                ));
            }
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_impl(&mut self, i: &ast::ImplBlock) {
        let target = self.mlir_type(&i.self_type);
        let trait_part = match &i.trait_path {
            Some(path) => format!(" : @{}", path.join(".")),
            None => " (inherent)".into(),
        };
        self.line(&format!("MAGE.impl {target}{trait_part} {{"));
        self.indent += 1;
        for item in &i.items {
            if let ast::ItemKind::Function(f) = &item.kind {
                self.emit_function(f, &item.visibility);
            }
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_effect_decl(&mut self, ef: &ast::EffectDef) {
        self.line(&format!("MAGE.effect @{} {{", ef.name));
        self.indent += 1;
        for op in &ef.operations {
            let ret = op
                .return_type
                .as_ref()
                .map(|t| self.mlir_type(t))
                .unwrap_or_else(|| "!MAGE.unit".into());
            let params: Vec<String> = op.params.iter().map(|p| self.mlir_type(&p.ty)).collect();
            self.line(&format!(
                "MAGE.op \"{}\"({}) -> {ret}",
                op.name,
                params.join(", ")
            ));
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_spec(&mut self, sp: &ast::SpecDef) {
        self.line(&format!("MAGE.spec @{} {{", sp.name));
        self.indent += 1;
        for item in &sp.items {
            match item {
                ast::SpecItem::Require(cond) => {
                    self.line(&format!("MAGE.contract.requires \"{cond}\""));
                }
                ast::SpecItem::Ensure(cond) => {
                    self.line(&format!("MAGE.contract.ensures \"{cond}\""));
                }
                ast::SpecItem::Invariant(cond) => {
                    self.line(&format!("MAGE.contract.invariant \"{cond}\""));
                }
                ast::SpecItem::Performance(metric, bound) => {
                    self.line(&format!("MAGE.perf \"{metric}\" bound=\"{bound}\""));
                }
                ast::SpecItem::Effect(effects) => {
                    let fx: Vec<String> = effects.iter().map(|e| format!("\"{e}\"")).collect();
                    self.line(&format!("MAGE.effect.set [{}]", fx.join(", ")));
                }
            }
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_agent(&mut self, ad: &ast::AgentDef) {
        self.line(&format!("MAGE.agent @{} {{", ad.name));
        self.indent += 1;
        for cap in &ad.capabilities {
            self.line(&format!("MAGE.capability \"{cap}\""));
        }
        for approval in &ad.requires_approval {
            self.line(&format!("MAGE.requires_approval \"{approval}\""));
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_net(&mut self, n: &ast::NetDef) {
        self.line(&format!("MAGE.neural.net @{} {{", n.name));
        self.indent += 1;
        for layer in &n.layers {
            let ty = self.mlir_type(&layer.layer_type);
            self.line(&format!("MAGE.neural.layer @{} : {ty}", layer.name));
        }
        self.line("MAGE.neural.forward {");
        self.indent += 1;
        self.emit_block(&n.forward);
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_kb(&mut self, k: &ast::KbDef) {
        self.line(&format!("MAGE.kb @{} {{", k.name));
        self.indent += 1;
        for f in &k.facts {
            self.line(&format!("MAGE.kb.fact @{}", f.name));
        }
        for r in &k.rules {
            self.line(&format!("MAGE.kb.rule @{}", r.name));
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_evolve(&mut self, e: &ast::EvolveDef) {
        let gty = self.mlir_type(&e.genome_type);
        self.line(&format!("MAGE.evolve @{} : {gty} {{", e.name));
        self.indent += 1;
        self.line("MAGE.evolve.fitness {");
        self.indent += 1;
        self.emit_block(&e.fitness);
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_train(&mut self, t: &ast::TrainDef) {
        self.line(&format!("MAGE.train @{} {{", t.name));
        self.indent += 1;
        self.line(&format!("MAGE.train.net @{}", t.net));
        self.emit_block(&t.body);
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_swarm(&mut self, s: &ast::SwarmDef) {
        self.line(&format!("MAGE.swarm @{} {{", s.name));
        self.indent += 1;
        if !s.agent_type.is_empty() {
            self.line(&format!("MAGE.swarm.agent @{}", s.agent_type));
        }
        if let Some(ref topo) = s.topology {
            self.line(&format!("MAGE.swarm.topology \"{topo}\""));
        }
        if let Some(ref cons) = s.consensus {
            self.line(&format!("MAGE.swarm.consensus \"{cons}\""));
        }
        if let Some(ref dispatch) = s.on_dispatch {
            self.line("MAGE.swarm.dispatch {");
            self.indent += 1;
            self.emit_block(dispatch);
            self.indent -= 1;
            self.line("}");
        }
        if let Some(ref aggregate) = s.on_aggregate {
            self.line("MAGE.swarm.aggregate {");
            self.indent += 1;
            self.emit_block(aggregate);
            self.indent -= 1;
            self.line("}");
        }
        if let Some(ref on_failure) = s.on_failure {
            self.line("MAGE.swarm.on_failure {");
            self.indent += 1;
            self.emit_block(on_failure);
            self.indent -= 1;
            self.line("}");
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    // ── Ownership operations ─────────────────────────────────────────

    /// Emit ownership-related MLIR ops for ownership transfer expressions.
    fn emit_ownership_op(&mut self, expr: &ast::Expr) -> String {
        match expr {
            ast::Expr::Unary { op, .. } if op == "*" => {
                let v = self.fresh();
                let out = self.fresh();
                self.line(&format!("{out} = MAGE.ownership.deref {v}"));
                out
            }
            ast::Expr::Unary { op, .. } if op == "&" => {
                let v = self.fresh();
                let out = self.fresh();
                self.line(&format!("{out} = MAGE.ownership.borrow {v}"));
                out
            }
            ast::Expr::Unary { op, .. } if op == "&mut" => {
                let v = self.fresh();
                let out = self.fresh();
                self.line(&format!("{out} = MAGE.ownership.borrow_mut {v}"));
                out
            }
            _ => {
                // For non-ownership expressions, emit a move (default ownership transfer).
                let v = self.fresh();
                format!("MAGE.ownership.move {v}")
            }
        }
    }

    // ── Blocks & statements ──────────────────────────────────────────

    fn emit_block(&mut self, block: &ast::Block) {
        for stmt in &block.stmts {
            self.emit_stmt(stmt);
        }
        if let Some(tail) = &block.tail_expr {
            let ssa = self.fresh();
            let summary = self.expr_summary(tail);
            self.line(&format!("{ssa} = {summary}  // tail"));
        }
    }

    fn emit_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Let {
                pattern,
                ty,
                value,
                mutable,
            } => {
                let ssa = self.fresh();
                let mlir_ty = ty
                    .as_ref()
                    .map(|t| self.mlir_type(t))
                    .unwrap_or_else(|| "!MAGE.inferred".into());
                let pat_name = self.pattern_name(pattern);
                let mut_key = if *mutable { "var" } else { "let" };
                let val_str = self.expr_summary(&value);
                self.line(&format!(
                    "{ssa} = MAGE.{mut_key} \"{pat_name}\" : {mlir_ty} = {val_str}"
                ));
            }
            ast::Stmt::Expr { expr } => {
                let ssa = self.fresh();
                let summary = self.expr_summary(expr);
                self.line(&format!("{ssa} = {summary}"));
            }
            ast::Stmt::Item { item } => {
                self.emit_item(item);
            }
            ast::Stmt::Guard { cond, .. } => {
                let ssa = self.fresh();
                let c = self.expr_summary(cond);
                self.line(&format!("{ssa} = MAGE.guard {c} else {{ ... }}"));
            }
            ast::Stmt::Defer { expr } => {
                let ssa = self.fresh();
                let v = self.expr_summary(expr);
                self.line(&format!("{ssa} = MAGE.defer {v}"));
            }
        }
    }

    fn pattern_name(&self, pat: &ast::Pattern) -> String {
        match pat {
            ast::Pattern::Ident { name } => name.clone(),
            ast::Pattern::Wildcard => "_".into(),
            ast::Pattern::Tuple { elements } => {
                let parts: Vec<String> = elements.iter().map(|p| self.pattern_name(p)).collect();
                format!("({})", parts.join(", "))
            }
            _ => "_".into(),
        }
    }

    // ── Expressions ──────────────────────────────────────────────────

    fn expr_summary(&mut self, expr: &ast::Expr) -> String {
        match expr {
            ast::Expr::Literal { value, kind } => {
                let ty = match kind {
                    ast::LiteralKind::Int => "i64",
                    ast::LiteralKind::Float => "f64",
                    ast::LiteralKind::Bool => "i1",
                    ast::LiteralKind::String => "!MAGE.str",
                    ast::LiteralKind::FormatString => "!MAGE.fstr",
                    ast::LiteralKind::Char => "!MAGE.char",
                    ast::LiteralKind::Byte => "i8",
                };
                format!("MAGE.const \"{value}\" : {ty}")
            }
            ast::Expr::Ident { name } => format!("MAGE.ref @{name}"),
            ast::Expr::Binary { op, .. } => {
                let l = self.fresh();
                let r = self.fresh();
                format!("MAGE.binop \"{op}\" {l}, {r}")
            }
            ast::Expr::Unary { op, .. } => {
                let v = self.fresh();
                format!("MAGE.unaryop \"{op}\" {v}")
            }
            ast::Expr::Call { func, args } => {
                let f = self.expr_summary(func);
                format!("MAGE.call {f}({} args)", args.len())
            }
            ast::Expr::MethodCall {
                receiver: _,
                method,
                args,
                ..
            } => {
                let obj = self.fresh();
                format!("MAGE.method {obj}.{method}({} args)", args.len())
            }
            ast::Expr::FieldAccess { object: _, field } => {
                let obj = self.fresh();
                format!("MAGE.field {obj}.{field}")
            }
            ast::Expr::Index { .. } => {
                let obj = self.fresh();
                let idx = self.fresh();
                format!("MAGE.index {obj}[{idx}]")
            }
            ast::Expr::StructLit { path, fields } => {
                let name = path.join(".");
                format!("MAGE.struct_lit @{name}({} fields)", fields.len())
            }
            ast::Expr::TupleLit { elements } => {
                format!("MAGE.tuple({} elems)", elements.len())
            }
            ast::Expr::ArrayLit { elements } => {
                format!("MAGE.array({} elems)", elements.len())
            }
            ast::Expr::MapLit { entries } => {
                format!("MAGE.map({} entries)", entries.len())
            }
            ast::Expr::ArrayRepeat { .. } => {
                format!("MAGE.array_repeat")
            }
            ast::Expr::Closure { params, .. } => {
                format!("MAGE.closure({} params)", params.len())
            }
            ast::Expr::If { .. } => {
                let cond = self.fresh();
                format!("MAGE.if {cond} then(...) else(...)")
            }
            ast::Expr::Match { arms, .. } => {
                format!("MAGE.match({} arms)", arms.len())
            }
            ast::Expr::Loop { .. } => format!("MAGE.loop {{ ... }}"),
            ast::Expr::While { .. } => format!("MAGE.while {{ ... }}"),
            ast::Expr::For { .. } => format!("MAGE.for {{ ... }}"),
            ast::Expr::Block { block } => {
                format!("MAGE.block({} stmts)", block.stmts.len())
            }
            ast::Expr::Return { value } => {
                if value.is_some() {
                    let v = self.fresh();
                    format!("MAGE.return {v}")
                } else {
                    format!("MAGE.return void")
                }
            }
            ast::Expr::Break { .. } => format!("MAGE.break"),
            ast::Expr::Continue => format!("MAGE.continue"),
            ast::Expr::Try { .. } => {
                let v = self.fresh();
                format!("MAGE.try {v}")
            }
            ast::Expr::Await { .. } => {
                let v = self.fresh();
                format!("MAGE.await {v}")
            }
            ast::Expr::Cast { expr: _, ty } => {
                let v = self.fresh();
                let target = self.mlir_type(ty);
                format!("MAGE.cast {v} -> {target}")
            }
            ast::Expr::Assign { .. } => {
                let t = self.fresh();
                let v = self.fresh();
                format!("MAGE.assign {t} = {v}")
            }
            ast::Expr::Range { inclusive, .. } => {
                let lo = self.fresh();
                let hi = self.fresh();
                format!("MAGE.range {lo}..{hi} (inclusive={inclusive})")
            }
            ast::Expr::Todo => format!("MAGE.todo"),
            ast::Expr::Unimplemented => format!("MAGE.unimplemented"),
            ast::Expr::UnsafeBlock { block } => {
                format!("MAGE.unsafe({} stmts)", block.stmts.len())
            }
            ast::Expr::Error { message } => format!("MAGE.error \"{message}\""),
            ast::Expr::Pipeline { .. } => {
                let l = self.fresh();
                let r = self.fresh();
                format!("MAGE.pipeline {l} |> {r}")
            }
            ast::Expr::Is { .. } => {
                let v = self.fresh();
                format!("MAGE.is {v} pattern")
            }
        }
    }

    // ── Type mapping ─────────────────────────────────────────────────

    fn mlir_type(&self, ty: &ast::Type) -> String {
        match ty {
            ast::Type::Path {
                segments,
                type_args,
            } => {
                let name = segments.join(".");
                let base = match name.as_str() {
                    "i8" => "i8".into(),
                    "i16" => "i16".into(),
                    "i32" => "i32".into(),
                    "i64" => "i64".into(),
                    "i128" => "i128".into(),
                    "u8" => "!MAGE.u8".into(),
                    "u16" => "!MAGE.u16".into(),
                    "u32" => "!MAGE.u32".into(),
                    "u64" => "!MAGE.u64".into(),
                    "u128" => "!MAGE.u128".into(),
                    "f32" => "f32".into(),
                    "f64" => "f64".into(),
                    "bool" => "i1".into(),
                    "s" | "str" => "!MAGE.str".into(),
                    "char" => "!MAGE.char".into(),
                    other => format!("!MAGE.named<\"{other}\">"),
                };
                if type_args.is_empty() {
                    base
                } else {
                    let args: Vec<String> = type_args.iter().map(|t| self.mlir_type(t)).collect();
                    format!("!MAGE.generic<\"{name}\", {}>", args.join(", "))
                }
            }
            ast::Type::Reference { mutable, inner } => {
                let inner_ty = self.mlir_type(inner);
                if *mutable {
                    format!("!MAGE.ref_mut<{inner_ty}>")
                } else {
                    format!("!MAGE.ref<{inner_ty}>")
                }
            }
            ast::Type::OwnedPtr { inner } => {
                format!("!MAGE.owned<{}>", self.mlir_type(inner))
            }
            ast::Type::Rc { inner } => {
                format!("!MAGE.rc<{}>", self.mlir_type(inner))
            }
            ast::Type::Arc { inner } => {
                format!("!MAGE.arc<{}>", self.mlir_type(inner))
            }
            ast::Type::Slice { inner } => {
                format!("!MAGE.slice<{}>", self.mlir_type(inner))
            }
            ast::Type::Array { inner, .. } => {
                format!("!MAGE.array<{}>", self.mlir_type(inner))
            }
            ast::Type::Vec { inner } => {
                format!("!MAGE.vec<{}>", self.mlir_type(inner))
            }
            ast::Type::Tuple { elements } => {
                let parts: Vec<String> = elements.iter().map(|t| self.mlir_type(t)).collect();
                format!("!MAGE.tuple<{}>", parts.join(", "))
            }
            ast::Type::Option { inner } => {
                format!("!MAGE.option<{}>", self.mlir_type(inner))
            }
            ast::Type::Result { ok, err } => {
                format!(
                    "!MAGE.result<{}, {}>",
                    self.mlir_type(ok),
                    self.mlir_type(err)
                )
            }
            ast::Type::Map { key, value } => {
                format!(
                    "!MAGE.map<{}, {}>",
                    self.mlir_type(key),
                    self.mlir_type(value)
                )
            }
            ast::Type::Ptr { inner } => {
                format!("!MAGE.raw_ptr<{}>", self.mlir_type(inner))
            }
            ast::Type::Simd { inner, width } => {
                format!("vector<{width}x{}>", self.mlir_type(inner))
            }
            ast::Type::Fn { params, ret } => {
                let ps: Vec<String> = params.iter().map(|t| self.mlir_type(t)).collect();
                let r = ret
                    .as_ref()
                    .map(|t| self.mlir_type(t))
                    .unwrap_or_else(|| "!MAGE.unit".into());
                format!("({}) -> {r}", ps.join(", "))
            }
            ast::Type::Never => "!MAGE.never".into(),
            ast::Type::Inferred => "!MAGE.inferred".into(),
            ast::Type::SelfType => "!MAGE.self".into(),
            ast::Type::StringType => "!MAGE.str".into(),
            ast::Type::Cow { inner } => {
                format!("!MAGE.cow<{}>", self.mlir_type(inner))
            }
            ast::Type::Cell { inner } => {
                format!("!MAGE.cell<{}>", self.mlir_type(inner))
            }
            ast::Type::RefCell { inner } => {
                format!("!MAGE.refcell<{}>", self.mlir_type(inner))
            }
            ast::Type::Mutex { inner } => {
                format!("!MAGE.mutex<{}>", self.mlir_type(inner))
            }
            ast::Type::RwLock { inner } => {
                format!("!MAGE.rwlock<{}>", self.mlir_type(inner))
            }
            ast::Type::Set { inner } => {
                format!("!MAGE.set<{}>", self.mlir_type(inner))
            }
            ast::Type::Refined { base, .. } => {
                // Refinement types lower to their base type in MLIR
                self.mlir_type(base)
            }
            ast::Type::Tensor { inner, shape } => {
                let dims: Vec<String> = shape
                    .iter()
                    .map(|d| match d {
                        ast::TensorDim::Lit(n) => n.to_string(),
                        ast::TensorDim::Var(v) => format!("?/*{v}*/"),
                    })
                    .collect();
                format!(
                    "!MAGE.tensor<{}x{}>",
                    dims.join("x"),
                    self.mlir_type(inner)
                )
            }
            ast::Type::ParamTy { inner, shape } => {
                let dims: Vec<String> = shape
                    .iter()
                    .map(|d| match d {
                        ast::TensorDim::Lit(n) => n.to_string(),
                        ast::TensorDim::Var(v) => format!("?/*{v}*/"),
                    })
                    .collect();
                format!(
                    "!MAGE.param<{}x{}>",
                    dims.join("x"),
                    self.mlir_type(inner)
                )
            }
            ast::Type::Genome { inner } => {
                format!("!MAGE.genome<{}>", self.mlir_type(inner))
            }
            ast::Type::Policy { state, action } => {
                format!(
                    "!MAGE.policy<{}, {}>",
                    self.mlir_type(state),
                    self.mlir_type(action)
                )
            }
            ast::Type::KnowledgeBase => "!MAGE.kb".into(),
            ast::Type::LlmType => "!MAGE.llm".into(),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{effects, lexer, parser};

    fn emit_source(src: &str) -> String {
        let tokens = lexer::lex(src);
        let module = parser::parse(&tokens).unwrap();
        let fx = effects::infer_effects(&module);
        emit(&module, &fx)
    }

    #[test]
    fn empty_module() {
        let mlir = emit_source("");
        assert!(mlir.contains("module {"));
        assert!(mlir.contains("}"));
    }

    #[test]
    fn pure_function() {
        let mlir = emit_source("+f add(a: i32, b: i32) -> i32 { a }");
        assert!(mlir.contains("MAGE.func pub @add"));
        assert!(mlir.contains("i32"));
        // pure function should NOT have effects attribute
        assert!(!mlir.contains("effects = ["));
    }

    #[test]
    fn struct_emit() {
        let mlir = emit_source("S Point { x: f64, y: f64 }");
        assert!(mlir.contains("MAGE.struct @Point"));
        assert!(mlir.contains("MAGE.field \"x\" : f64"));
        assert!(mlir.contains("MAGE.field \"y\" : f64"));
    }

    #[test]
    fn enum_emit() {
        let mlir = emit_source("E Color { Red, Green, Blue }");
        assert!(mlir.contains("MAGE.enum @Color"));
        assert!(mlir.contains("MAGE.variant \"Red\""));
    }

    #[test]
    fn let_binding() {
        let mlir = emit_source("+f foo() { v x: i32 = 42; }");
        assert!(mlir.contains("MAGE.let \"x\""));
        assert!(mlir.contains("i32"));
    }

    // ── Step 41: Dialect ops for contracts, agents, specs, effects ──

    #[test]
    fn function_with_contract() {
        let mlir = emit_source("@req(a > 0) @ens(result > 0) +f add(a: i32, b: i32) -> i32 { a }");
        assert!(mlir.contains("MAGE.contract.requires \"a > 0\""));
        assert!(mlir.contains("MAGE.contract.ensures \"result > 0\""));
    }

    #[test]
    fn struct_with_invariant() {
        let mlir = emit_source("@inv(_.x >= 0) S Pos { x: f64 }");
        // The struct should contain an invariant contract op
        assert!(mlir.contains("MAGE.struct @Pos"));
        assert!(mlir.contains("MAGE.contract.invariant"));
        assert!(mlir.contains("x >= 0"));
    }

    #[test]
    fn agent_as_mlir_op() {
        let mlir = emit_source("agent Bot { capabilities: [read_source, net] }");
        assert!(mlir.contains("MAGE.agent @Bot"));
        assert!(mlir.contains("MAGE.capability \"read_source\""));
        assert!(mlir.contains("MAGE.capability \"net\""));
    }

    #[test]
    fn agent_with_approval() {
        let mlir = emit_source("agent Admin { capabilities: [fs] requires_approval: [exec] }");
        assert!(mlir.contains("MAGE.agent @Admin"));
        assert!(mlir.contains("MAGE.capability \"fs\""));
        assert!(mlir.contains("MAGE.requires_approval \"exec\""));
    }

    #[test]
    fn spec_as_mlir_op() {
        let mlir = emit_source("spec add_spec { @req(a > 0) @ens(result > a) }");
        assert!(mlir.contains("MAGE.spec @add_spec"));
        assert!(mlir.contains("MAGE.contract.requires \"a > 0\""));
        assert!(mlir.contains("MAGE.contract.ensures \"result > a\""));
    }

    #[test]
    fn effect_decl_as_mlir_op() {
        let mlir = emit_source("effect IO { f read() -> s; f write(data: s); }");
        assert!(mlir.contains("MAGE.effect @IO"));
        assert!(mlir.contains("MAGE.op \"read\""));
        assert!(mlir.contains("MAGE.op \"write\""));
    }

    #[test]
    fn ownership_types_in_mlir() {
        let mlir = emit_source("+f foo(a: ^i32, b: $i32, c: #i32) {}");
        assert!(mlir.contains("!MAGE.owned<i32>"));
        assert!(mlir.contains("!MAGE.rc<i32>"));
        assert!(mlir.contains("!MAGE.mutex<i32>"));
    }
}
