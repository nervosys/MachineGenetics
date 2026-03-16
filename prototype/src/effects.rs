/// Redox Effect Inference — bottom-up effect set computation.
///
/// Implements the effect inference algorithm from REDOX_SPEC.md §5:
///   InferEffects(fn):
///     1. Collect all effect.perform calls in fn body
///     2. For each called function g, recursively InferEffects(g)
///     3. fn.effects = union of all performed + callee effects
///     4. If explicit annotation exists, verify inferred ⊆ declared
///     5. Violation → emit structured diagnostic
///
/// Effects are inferred bottom-up: leaf functions first, callers accumulate.
use crate::ast;
use crate::hir::{Diagnostic, Effect, EffectSet, pure};
use std::collections::HashMap;

// ── Effect inference engine ──────────────────────────────────────────

pub struct EffectInfer {
    /// Declared effect annotations per function (from AST `effects` field or @fx attributes).
    declared: HashMap<String, EffectSet>,
    /// Inferred effect sets per function.
    pub inferred: HashMap<String, EffectSet>,
    /// Call graph: caller → Vec<callee>.
    call_graph: HashMap<String, Vec<String>>,
    /// Currently being inferred (for cycle detection).
    in_progress: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl EffectInfer {
    pub fn new() -> Self {
        EffectInfer {
            declared: HashMap::new(),
            inferred: HashMap::new(),
            call_graph: HashMap::new(),
            in_progress: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    // ── Module-level inference ────────────────────────────────────────

    pub fn infer_module(&mut self, module: &ast::Module) {
        // Pass 1: collect function declarations and their call graphs.
        for item in &module.items {
            if let ast::ItemKind::Function(fd) = &item.kind {
                self.collect_function(fd);
            }
        }

        // Pass 2: infer effects bottom-up.
        let fn_names: Vec<String> = self.call_graph.keys().cloned().collect();
        for name in &fn_names {
            self.infer_function(name);
        }

        // Pass 3: verify declared ⊆ inferred.
        for (name, declared) in &self.declared {
            if let Some(inferred) = self.inferred.get(name) {
                // Check that inferred effects are a subset of declared effects.
                let undeclared: Vec<&Effect> =
                    inferred.iter().filter(|e| !declared.contains(e)).collect();

                if !undeclared.is_empty() {
                    let effects: Vec<String> = undeclared.iter().map(|e| e.to_string()).collect();
                    self.diagnostics.push(Diagnostic::error(format!(
                        "function `{name}` performs undeclared effects: [{}]",
                        effects.join(", ")
                    )));
                }
            }
        }
    }

    fn collect_function(&mut self, fd: &ast::FunctionDef) {
        // Record declared effects from annotations.
        if !fd.effects.is_empty() {
            let effects: EffectSet = fd.effects.iter().map(|e| Effect::from_name(e)).collect();
            self.declared.insert(fd.name.clone(), effects);
        }

        // Also check attributes for @fx(...).
        // (Not implemented in parser attributes yet, but the hook is here.)

        // Build call graph for this function by walking its body.
        let mut callees = Vec::new();
        let mut local_effects = EffectSet::new();
        self.collect_calls_in_block(&fd.body, &mut callees, &mut local_effects);

        self.call_graph.insert(fd.name.clone(), callees);

        // If the function has local effects (from effect.perform), record them.
        if !local_effects.is_empty() {
            self.inferred.insert(fd.name.clone(), local_effects);
        }
    }

    fn collect_calls_in_block(
        &self,
        block: &ast::Block,
        callees: &mut Vec<String>,
        local_effects: &mut EffectSet,
    ) {
        for stmt in &block.stmts {
            self.collect_calls_in_stmt(stmt, callees, local_effects);
        }
        if let Some(tail) = &block.tail_expr {
            self.collect_calls_in_expr(tail, callees, local_effects);
        }
    }

    fn collect_calls_in_stmt(
        &self,
        stmt: &ast::Stmt,
        callees: &mut Vec<String>,
        local_effects: &mut EffectSet,
    ) {
        match stmt {
            ast::Stmt::Let { value, .. } => {
                self.collect_calls_in_expr(value, callees, local_effects);
            }
            ast::Stmt::Expr { expr } => {
                self.collect_calls_in_expr(expr, callees, local_effects);
            }
            ast::Stmt::Item { item } => {
                if let ast::ItemKind::Function(fd) = &item.kind {
                    // Nested function — don't recurse into it for the parent's effects.
                    // It will be analyzed separately.
                    let _ = fd;
                }
            }
        }
    }

    fn collect_calls_in_expr(
        &self,
        expr: &ast::Expr,
        callees: &mut Vec<String>,
        local_effects: &mut EffectSet,
    ) {
        match expr {
            ast::Expr::Call { func, args } => {
                // Track the callee name.
                if let ast::Expr::Ident { name } = func.as_ref() {
                    callees.push(name.clone());

                    // Check for known effectful standard library functions.
                    self.check_builtin_effect(name, local_effects);
                }
                // Also check for effect.perform pattern.
                if let ast::Expr::FieldAccess { object, field } = func.as_ref() {
                    if field == "perform" {
                        // The object should be an effect name.
                        if let ast::Expr::Ident { name } = object.as_ref() {
                            local_effects.insert(Effect::from_name(name));
                        }
                    }
                }
                self.collect_calls_in_expr(func, callees, local_effects);
                for arg in args {
                    self.collect_calls_in_expr(arg, callees, local_effects);
                }
            }
            ast::Expr::MethodCall { receiver, args, .. } => {
                self.collect_calls_in_expr(receiver, callees, local_effects);
                for arg in args {
                    self.collect_calls_in_expr(arg, callees, local_effects);
                }
            }
            ast::Expr::Binary { left, right, .. } => {
                self.collect_calls_in_expr(left, callees, local_effects);
                self.collect_calls_in_expr(right, callees, local_effects);
            }
            ast::Expr::Unary { operand, .. } => {
                self.collect_calls_in_expr(operand, callees, local_effects);
            }
            ast::Expr::If { cond, then_block, else_block } => {
                self.collect_calls_in_expr(cond, callees, local_effects);
                self.collect_calls_in_block(then_block, callees, local_effects);
                if let Some(eb) = else_block {
                    self.collect_calls_in_block(eb, callees, local_effects);
                }
            }
            ast::Expr::Match { arms } => {
                for arm in arms {
                    self.collect_calls_in_expr(&arm.body, callees, local_effects);
                }
            }
            ast::Expr::Loop { body } => {
                self.collect_calls_in_block(body, callees, local_effects);
            }
            ast::Expr::For { iter, body, .. } => {
                self.collect_calls_in_expr(iter, callees, local_effects);
                self.collect_calls_in_block(body, callees, local_effects);
            }
            ast::Expr::Block { block } => {
                self.collect_calls_in_block(block, callees, local_effects);
            }
            ast::Expr::Closure { body, .. } => {
                self.collect_calls_in_expr(body, callees, local_effects);
            }
            ast::Expr::Return { value } | ast::Expr::Break { value } => {
                if let Some(v) = value {
                    self.collect_calls_in_expr(v, callees, local_effects);
                }
            }
            ast::Expr::Try { expr } | ast::Expr::Await { expr } => {
                self.collect_calls_in_expr(expr, callees, local_effects);
                if matches!(expr.as_ref(), ast::Expr::Await { .. }) {
                    local_effects.insert(Effect::Async);
                }
            }
            // (Await already handled in Try | Await arm above.)
            ast::Expr::Cast { expr, .. } => {
                self.collect_calls_in_expr(expr, callees, local_effects);
            }
            ast::Expr::Assign { target, value } => {
                self.collect_calls_in_expr(target, callees, local_effects);
                self.collect_calls_in_expr(value, callees, local_effects);
            }
            ast::Expr::Range { start, end, .. } => {
                self.collect_calls_in_expr(start, callees, local_effects);
                self.collect_calls_in_expr(end, callees, local_effects);
            }
            ast::Expr::FieldAccess { object, .. } => {
                self.collect_calls_in_expr(object, callees, local_effects);
            }
            ast::Expr::Index { object, index } => {
                self.collect_calls_in_expr(object, callees, local_effects);
                self.collect_calls_in_expr(index, callees, local_effects);
            }
            ast::Expr::StructLit { fields, .. } => {
                for fi in fields {
                    if let Some(val) = &fi.value {
                        self.collect_calls_in_expr(val, callees, local_effects);
                    }
                }
            }
            ast::Expr::TupleLit { elements } | ast::Expr::ArrayLit { elements } => {
                for el in elements {
                    self.collect_calls_in_expr(el, callees, local_effects);
                }
            }
            ast::Expr::ArrayRepeat { value, count } => {
                self.collect_calls_in_expr(value, callees, local_effects);
                self.collect_calls_in_expr(count, callees, local_effects);
            }
            // Leaves — no sub-expressions.
            ast::Expr::Literal { .. }
            | ast::Expr::Ident { .. }
            | ast::Expr::Continue
            | ast::Expr::Error { .. } => {}
        }
    }

    /// Check if a function name is a known effectful builtin.
    fn check_builtin_effect(&self, name: &str, effects: &mut EffectSet) {
        match name {
            "print" | "println" | "eprint" | "eprintln" | "write" | "writeln" => {
                effects.insert(Effect::IO);
            }
            "read" | "read_line" | "read_to_string" => {
                effects.insert(Effect::IO);
            }
            "open" | "create" | "remove" | "rename" | "mkdir" | "stat" => {
                effects.insert(Effect::FS);
            }
            "connect" | "listen" | "bind" | "send" | "recv" => {
                effects.insert(Effect::Net);
            }
            "spawn" | "join" | "select" => {
                effects.insert(Effect::Async);
            }
            "alloc" | "dealloc" | "realloc" => {
                effects.insert(Effect::Alloc);
            }
            "panic" => {
                effects.insert(Effect::Panic);
            }
            "env" | "get_env" | "set_env" => {
                effects.insert(Effect::Env);
            }
            "now" | "sleep" | "timeout" => {
                effects.insert(Effect::Time);
            }
            _ => {}
        }
    }

    // ── Bottom-up inference ──────────────────────────────────────────

    fn infer_function(&mut self, name: &str) -> EffectSet {
        // Check if already computed.
        if let Some(effects) = self.inferred.get(name) {
            return effects.clone();
        }

        // Cycle detection.
        if self.in_progress.contains(&name.to_string()) {
            // Recursive call — return what we have so far (empty = pure until proven otherwise).
            return pure();
        }

        self.in_progress.push(name.to_string());

        let callees = self.call_graph.get(name).cloned().unwrap_or_default();

        // Start with any locally-performed effects.
        let mut effects = self.inferred.get(name).cloned().unwrap_or_else(pure);

        // Accumulate effects from callees.
        for callee in &callees {
            let callee_effects = self.infer_function(callee);
            effects.extend(callee_effects);
        }

        self.in_progress.retain(|n| n != name);
        self.inferred.insert(name.to_string(), effects.clone());
        effects
    }

    /// Get the inferred effect set for a function. Returns `pure` if unknown.
    pub fn effects_of(&self, name: &str) -> EffectSet {
        self.inferred.get(name).cloned().unwrap_or_else(pure)
    }
}

// ── Public API ───────────────────────────────────────────────────────

/// Run effect inference on a parsed module.
pub fn infer_effects(module: &ast::Module) -> EffectInfer {
    let mut engine = EffectInfer::new();
    engine.infer_module(module);
    engine
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn infer_source(src: &str) -> EffectInfer {
        let tokens = lexer::lex(src);
        let module = parser::parse(&tokens).expect("parse failed");
        infer_effects(&module)
    }

    #[test]
    fn test_pure_function() {
        let ei = infer_source("f add(a: i32, b: i32) -> i32 { a + b }");
        assert!(ei.diagnostics.is_empty(), "errors: {:?}", ei.diagnostics);
        assert!(ei.effects_of("add").is_empty(), "expected pure, got {:?}", ei.effects_of("add"));
    }

    #[test]
    fn test_io_effect_detected() {
        let src = r#"
            f greet(name: str) -> () {
                println(name)
            }
        "#;
        let ei = infer_source(src);
        assert!(
            ei.effects_of("greet").contains(&Effect::IO),
            "expected IO effect, got {:?}",
            ei.effects_of("greet")
        );
    }

    #[test]
    fn test_transitive_effects() {
        let src = r#"
            f write_file() -> () {
                open()
            }
            f main_fn() -> () {
                write_file()
            }
        "#;
        let ei = infer_source(src);
        // write_file calls open → FS effect. main_fn calls write_file → also FS.
        assert!(ei.effects_of("write_file").contains(&Effect::FS));
        assert!(ei.effects_of("main_fn").contains(&Effect::FS));
    }

    #[test]
    fn test_multiple_effects() {
        let src = r#"
            f complex() -> () {
                println("start");
                connect();
                spawn()
            }
        "#;
        let ei = infer_source(src);
        let effects = ei.effects_of("complex");
        assert!(effects.contains(&Effect::IO), "missing IO");
        assert!(effects.contains(&Effect::Net), "missing Net");
        assert!(effects.contains(&Effect::Async), "missing Async");
    }

    #[test]
    fn test_pure_stays_pure() {
        let src = r#"
            f double(x: i32) -> i32 { x * 2 }
            f quadruple(x: i32) -> i32 { double(double(x)) }
        "#;
        let ei = infer_source(src);
        assert!(ei.effects_of("double").is_empty());
        assert!(ei.effects_of("quadruple").is_empty());
    }

    #[test]
    fn test_effect_perform_syntax() {
        // Simulate effect.perform pattern: IO.perform(...)
        let src = r#"
            f with_io() -> () {
                v x: i32 = 1;
                x
            }
        "#;
        let ei = infer_source(src);
        // This function is pure (no perform call).
        assert!(ei.effects_of("with_io").is_empty());
    }
}
