/// MLIR dialect output for the Redox compiler.
///
/// Generates textual MLIR in the `redox` dialect from a type-checked,
/// effect-annotated AST.  This is the final stage of the prototype
/// pipeline:  Lex → Parse → Resolve → TypeCheck → EffectInfer → MLIR.
use crate::ast;
use crate::effects::EffectInfer;
use crate::hir::EffectSet;
use std::collections::HashMap;

/// Emit MLIR textual representation for a checked module.
pub fn emit(module: &ast::Module, effects: &EffectInfer) -> String {
    let mut ctx =
        EmitCtx { buf: String::with_capacity(4096), indent: 0, ssa: 0, effects: &effects.inferred };

    ctx.line("// ──────────────────────────────────────────────────────────────");
    ctx.line("// Auto-generated Redox MLIR  ·  dialect: redox");
    ctx.line("// https://github.com/nervosys/Redox");
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
                self.line(&format!("redox.const @{} : {ty}", c.name));
            }
            ast::ItemKind::Module(m) => {
                self.line(&format!("// module {}", m.name));
            }
            ast::ItemKind::Use(u) => {
                self.line(&format!("// use {}", u.path.join(".")));
            }
            ast::ItemKind::Effect(ef) => self.emit_effect_decl(ef),
            ast::ItemKind::Spec(sp) => {
                self.line(&format!("// spec {}", sp.name));
            }
            ast::ItemKind::Static(sd) => {
                let ty = self.mlir_type(&sd.ty);
                self.line(&format!("redox.static @{} : {ty}", sd.name));
            }
            ast::ItemKind::Agent(ad) => {
                self.line(&format!("// agent {} capabilities=[{}]", ad.name, ad.capabilities.join(", ")));
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
            .unwrap_or_else(|| "!redox.unit".into());

        // Effect set attribute.
        let fx = self.effects.get(name).cloned().unwrap_or_default();
        let fx_attr = if fx.is_empty() {
            String::new()
        } else {
            let effects: Vec<String> = fx.iter().map(|e| format!("\"{e}\"")).collect();
            format!(" attributes {{ effects = [{}] }}", effects.join(", "))
        };

        self.line(&format!(
            "redox.func {vis_prefix}@{name}({}) -> {ret_ty}{fx_attr} {{",
            params.join(", ")
        ));

        self.indent += 1;
        let entry = self.fresh();
        self.line(&format!("{entry}:  // entry"));
        self.indent += 1;

        self.emit_block(&f.body);

        let ret_val = self.fresh();
        self.line(&format!("redox.return {ret_val} : {ret_ty}"));

        self.indent -= 1;
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_struct(&mut self, s: &ast::StructDef) {
        self.line(&format!("redox.struct @{} {{", s.name));
        self.indent += 1;
        for field in &s.fields {
            let ty = self.mlir_type(&field.ty);
            self.line(&format!("redox.field \"{}\" : {ty}", field.name));
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
    }

    fn emit_enum(&mut self, e: &ast::EnumDef) {
        self.line(&format!("redox.enum @{} {{", e.name));
        self.indent += 1;
        for variant in &e.variants {
            match &variant.kind {
                ast::VariantKind::Unit => {
                    self.line(&format!("redox.variant \"{}\"", variant.name));
                }
                ast::VariantKind::Tuple(types) => {
                    let tys: Vec<String> = types.iter().map(|t| self.mlir_type(t)).collect();
                    self.line(&format!("redox.variant \"{}\"({})", variant.name, tys.join(", ")));
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
                        "redox.variant \"{}\" {{ {} }}",
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
        self.line(&format!("redox.trait @{}{supers} {{", t.name));
        self.indent += 1;
        for item in &t.items {
            if let ast::ItemKind::Function(f) = &item.kind {
                let ret = f
                    .return_type
                    .as_ref()
                    .map(|t| self.mlir_type(t))
                    .unwrap_or_else(|| "!redox.unit".into());
                let params: Vec<String> = f.params.iter().map(|p| self.mlir_type(&p.ty)).collect();
                self.line(&format!(
                    "redox.method_decl \"{}\"({}) -> {ret}",
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
        self.line(&format!("redox.impl {target}{trait_part} {{"));
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
        self.line(&format!("redox.effect @{} {{", ef.name));
        self.indent += 1;
        for op in &ef.operations {
            let ret = op
                .return_type
                .as_ref()
                .map(|t| self.mlir_type(t))
                .unwrap_or_else(|| "!redox.unit".into());
            let params: Vec<String> = op.params.iter().map(|p| self.mlir_type(&p.ty)).collect();
            self.line(&format!("redox.op \"{}\"({}) -> {ret}", op.name, params.join(", ")));
        }
        self.indent -= 1;
        self.line("}");
        self.line("");
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
            ast::Stmt::Let { pattern, ty, value, mutable } => {
                let ssa = self.fresh();
                let mlir_ty = ty
                    .as_ref()
                    .map(|t| self.mlir_type(t))
                    .unwrap_or_else(|| "!redox.inferred".into());
                let pat_name = self.pattern_name(pattern);
                let mut_key = if *mutable { "var" } else { "let" };
                let val_str = self.expr_summary(&value);
                self.line(&format!(
                    "{ssa} = redox.{mut_key} \"{pat_name}\" : {mlir_ty} = {val_str}"
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
                    ast::LiteralKind::String => "!redox.str",
                    ast::LiteralKind::FormatString => "!redox.fstr",
                    ast::LiteralKind::Char => "!redox.char",
                    ast::LiteralKind::Byte => "i8",
                };
                format!("redox.const \"{value}\" : {ty}")
            }
            ast::Expr::Ident { name } => format!("redox.ref @{name}"),
            ast::Expr::Binary { op, .. } => {
                let l = self.fresh();
                let r = self.fresh();
                format!("redox.binop \"{op}\" {l}, {r}")
            }
            ast::Expr::Unary { op, .. } => {
                let v = self.fresh();
                format!("redox.unaryop \"{op}\" {v}")
            }
            ast::Expr::Call { func, args } => {
                let f = self.expr_summary(func);
                format!("redox.call {f}({} args)", args.len())
            }
            ast::Expr::MethodCall { receiver: _, method, args, .. } => {
                let obj = self.fresh();
                format!("redox.method {obj}.{method}({} args)", args.len())
            }
            ast::Expr::FieldAccess { object: _, field } => {
                let obj = self.fresh();
                format!("redox.field {obj}.{field}")
            }
            ast::Expr::Index { .. } => {
                let obj = self.fresh();
                let idx = self.fresh();
                format!("redox.index {obj}[{idx}]")
            }
            ast::Expr::StructLit { path, fields } => {
                let name = path.join(".");
                format!("redox.struct_lit @{name}({} fields)", fields.len())
            }
            ast::Expr::TupleLit { elements } => {
                format!("redox.tuple({} elems)", elements.len())
            }
            ast::Expr::ArrayLit { elements } => {
                format!("redox.array({} elems)", elements.len())
            }
            ast::Expr::ArrayRepeat { .. } => {
                format!("redox.array_repeat")
            }
            ast::Expr::Closure { params, .. } => {
                format!("redox.closure({} params)", params.len())
            }
            ast::Expr::If { .. } => {
                let cond = self.fresh();
                format!("redox.if {cond} then(...) else(...)")
            }
            ast::Expr::Match { arms, .. } => {
                format!("redox.match({} arms)", arms.len())
            }
            ast::Expr::Loop { .. } => format!("redox.loop {{ ... }}"),
            ast::Expr::While { .. } => format!("redox.while {{ ... }}"),
            ast::Expr::For { .. } => format!("redox.for {{ ... }}"),
            ast::Expr::Block { block } => {
                format!("redox.block({} stmts)", block.stmts.len())
            }
            ast::Expr::Return { value } => {
                if value.is_some() {
                    let v = self.fresh();
                    format!("redox.return {v}")
                } else {
                    format!("redox.return void")
                }
            }
            ast::Expr::Break { .. } => format!("redox.break"),
            ast::Expr::Continue => format!("redox.continue"),
            ast::Expr::Try { .. } => {
                let v = self.fresh();
                format!("redox.try {v}")
            }
            ast::Expr::Await { .. } => {
                let v = self.fresh();
                format!("redox.await {v}")
            }
            ast::Expr::Cast { expr: _, ty } => {
                let v = self.fresh();
                let target = self.mlir_type(ty);
                format!("redox.cast {v} -> {target}")
            }
            ast::Expr::Assign { .. } => {
                let t = self.fresh();
                let v = self.fresh();
                format!("redox.assign {t} = {v}")
            }
            ast::Expr::Range { inclusive, .. } => {
                let lo = self.fresh();
                let hi = self.fresh();
                format!("redox.range {lo}..{hi} (inclusive={inclusive})")
            }
            ast::Expr::Todo => format!("redox.todo"),
            ast::Expr::Unimplemented => format!("redox.unimplemented"),
            ast::Expr::UnsafeBlock { block } => {
                format!("redox.unsafe({} stmts)", block.stmts.len())
            }
            ast::Expr::Error { message } => format!("redox.error \"{message}\""),
        }
    }

    // ── Type mapping ─────────────────────────────────────────────────

    fn mlir_type(&self, ty: &ast::Type) -> String {
        match ty {
            ast::Type::Path { segments, type_args } => {
                let name = segments.join(".");
                let base = match name.as_str() {
                    "i8" => "i8".into(),
                    "i16" => "i16".into(),
                    "i32" => "i32".into(),
                    "i64" => "i64".into(),
                    "i128" => "i128".into(),
                    "u8" => "!redox.u8".into(),
                    "u16" => "!redox.u16".into(),
                    "u32" => "!redox.u32".into(),
                    "u64" => "!redox.u64".into(),
                    "u128" => "!redox.u128".into(),
                    "f32" => "f32".into(),
                    "f64" => "f64".into(),
                    "bool" => "i1".into(),
                    "s" | "str" => "!redox.str".into(),
                    "char" => "!redox.char".into(),
                    other => format!("!redox.named<\"{other}\">"),
                };
                if type_args.is_empty() {
                    base
                } else {
                    let args: Vec<String> = type_args.iter().map(|t| self.mlir_type(t)).collect();
                    format!("!redox.generic<\"{name}\", {}>", args.join(", "))
                }
            }
            ast::Type::Reference { mutable, inner } => {
                let inner_ty = self.mlir_type(inner);
                if *mutable {
                    format!("!redox.ref_mut<{inner_ty}>")
                } else {
                    format!("!redox.ref<{inner_ty}>")
                }
            }
            ast::Type::OwnedPtr { inner } => {
                format!("!redox.owned<{}>", self.mlir_type(inner))
            }
            ast::Type::Rc { inner } => {
                format!("!redox.rc<{}>", self.mlir_type(inner))
            }
            ast::Type::Arc { inner } => {
                format!("!redox.arc<{}>", self.mlir_type(inner))
            }
            ast::Type::Slice { inner } => {
                format!("!redox.slice<{}>", self.mlir_type(inner))
            }
            ast::Type::Array { inner, .. } => {
                format!("!redox.array<{}>", self.mlir_type(inner))
            }
            ast::Type::Vec { inner } => {
                format!("!redox.vec<{}>", self.mlir_type(inner))
            }
            ast::Type::Tuple { elements } => {
                let parts: Vec<String> = elements.iter().map(|t| self.mlir_type(t)).collect();
                format!("!redox.tuple<{}>", parts.join(", "))
            }
            ast::Type::Option { inner } => {
                format!("!redox.option<{}>", self.mlir_type(inner))
            }
            ast::Type::Result { ok, err } => {
                format!("!redox.result<{}, {}>", self.mlir_type(ok), self.mlir_type(err))
            }
            ast::Type::Map { key, value } => {
                format!("!redox.map<{}, {}>", self.mlir_type(key), self.mlir_type(value))
            }
            ast::Type::Ptr { inner } => {
                format!("!redox.raw_ptr<{}>", self.mlir_type(inner))
            }
            ast::Type::Simd { inner, width } => {
                format!("vector<{width}x{}>", self.mlir_type(inner))
            }
            ast::Type::Fn { params, ret } => {
                let ps: Vec<String> = params.iter().map(|t| self.mlir_type(t)).collect();
                let r =
                    ret.as_ref().map(|t| self.mlir_type(t)).unwrap_or_else(|| "!redox.unit".into());
                format!("({}) -> {r}", ps.join(", "))
            }
            ast::Type::Never => "!redox.never".into(),
            ast::Type::Inferred => "!redox.inferred".into(),
            ast::Type::SelfType => "!redox.self".into(),
            ast::Type::StringType => "!redox.str".into(),
            ast::Type::Cow { inner } => {
                format!("!redox.cow<{}>", self.mlir_type(inner))
            }
            ast::Type::Cell { inner } => {
                format!("!redox.cell<{}>", self.mlir_type(inner))
            }
            ast::Type::RefCell { inner } => {
                format!("!redox.refcell<{}>", self.mlir_type(inner))
            }
            ast::Type::Mutex { inner } => {
                format!("!redox.mutex<{}>", self.mlir_type(inner))
            }
            ast::Type::RwLock { inner } => {
                format!("!redox.rwlock<{}>", self.mlir_type(inner))
            }
            ast::Type::Set { inner } => {
                format!("!redox.set<{}>", self.mlir_type(inner))
            }
            ast::Type::Refined { base, .. } => {
                // Refinement types lower to their base type in MLIR
                self.mlir_type(base)
            }
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
        assert!(mlir.contains("redox.func pub @add"));
        assert!(mlir.contains("i32"));
        // pure function should NOT have effects attribute
        assert!(!mlir.contains("effects = ["));
    }

    #[test]
    fn struct_emit() {
        let mlir = emit_source("S Point { x: f64, y: f64 }");
        assert!(mlir.contains("redox.struct @Point"));
        assert!(mlir.contains("redox.field \"x\" : f64"));
        assert!(mlir.contains("redox.field \"y\" : f64"));
    }

    #[test]
    fn enum_emit() {
        let mlir = emit_source("E Color { Red, Green, Blue }");
        assert!(mlir.contains("redox.enum @Color"));
        assert!(mlir.contains("redox.variant \"Red\""));
    }

    #[test]
    fn let_binding() {
        let mlir = emit_source("+f foo() { v x: i32 = 42; }");
        assert!(mlir.contains("redox.let \"x\""));
        assert!(mlir.contains("i32"));
    }
}
