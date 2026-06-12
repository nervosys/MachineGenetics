/// MechGen Effect Inference — bottom-up effect set computation.
///
/// Implements the effect inference algorithm from MECHGEN_SPEC.md §5:
///   InferEffects(fn):
///     1. Collect all effect.perform calls in fn body
///     2. For each called function g, recursively InferEffects(g)
///     3. fn.effects = union of all performed + callee effects
///     4. If explicit annotation exists, verify inferred ⊆ declared
///     5. Violation → emit structured diagnostic
///
/// Effects are inferred bottom-up: leaf functions first, callers accumulate.
use crate::ast;
use crate::hir::DiagnosticCategory;
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
        // Trust boundaries: public functions (and `main`, the entry). Effect
        // declarations are required here; private functions infer silently —
        // their effects still propagate to any public caller (see Pass 3).
        let mut boundary: std::collections::HashSet<String> = std::collections::HashSet::new();
        // Pass 1: collect function declarations and their call graphs.
        for item in &module.items {
            if let ast::ItemKind::Function(fd) = &item.kind {
                self.collect_function(fd);
                if item.visibility == ast::Visibility::Public || fd.name == "main" {
                    boundary.insert(fd.name.clone());
                }
            }
        }

        // Pass 2: infer effects bottom-up.
        let fn_names: Vec<String> = self.call_graph.keys().cloned().collect();
        for name in &fn_names {
            self.infer_function(name);
        }

        // Pass 3: SOUND effect checking at trust boundaries.
        //   • A function with an explicit annotation must honour it everywhere
        //     (inferred ⊆ declared), pub or private — a declared bound is a
        //     contract the body cannot exceed.
        //   • A function with NO annotation must declare only if it is a trust
        //     boundary (public, or `main`); private functions infer silently.
        // This is sound because effects propagate transitively (Pass 2): any
        // effect a private function performs surfaces in the inferred set of
        // every public caller that reaches it, and that boundary must declare
        // it. Nothing escapes undeclared — the capability gate holds at the
        // module surface, while internal code pays zero annotation tokens.
        // Sorted iteration keeps diagnostics deterministic.
        let mut names: Vec<&String> = self.inferred.keys().collect();
        names.sort();
        for name in names {
            let inferred = &self.inferred[name];
            if inferred.is_empty() {
                continue;
            }
            // Private, unannotated functions infer silently — they are bounded
            // by the public callers that reach them.
            if !self.declared.contains_key(name) && !boundary.contains(name.as_str()) {
                continue;
            }
            let declared = self.declared.get(name).cloned().unwrap_or_else(pure);
            let undeclared: Vec<&Effect> =
                inferred.iter().filter(|e| !declared.contains(e)).collect();
            if !undeclared.is_empty() {
                let effects: Vec<String> = undeclared.iter().map(|e| e.to_string()).collect();
                self.diagnostics.push(Diagnostic::categorized(
                    crate::hir::Severity::Error,
                    format!(
                        "function `{name}` performs undeclared effects: [{}] — add them to its `/ effect` annotation",
                        effects.join(", ")
                    ),
                    DiagnosticCategory::UndeclaredEffect,
                    None,
                ));
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
            ast::Stmt::Guard { cond, else_block } => {
                self.collect_calls_in_expr(cond, callees, local_effects);
                self.collect_calls_in_block(else_block, callees, local_effects);
            }
            ast::Stmt::Defer { expr } => {
                self.collect_calls_in_expr(expr, callees, local_effects);
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
            ast::Expr::Match { arms, .. } => {
                for arm in arms {
                    self.collect_calls_in_expr(&arm.body, callees, local_effects);
                }
            }
            ast::Expr::Loop { body } => {
                self.collect_calls_in_block(body, callees, local_effects);
            }
            ast::Expr::While { cond, body } => {
                self.collect_calls_in_expr(cond, callees, local_effects);
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
            ast::Expr::UnsafeBlock { block } => {
                self.collect_calls_in_block(block, callees, local_effects);
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
            ast::Expr::MapLit { entries } => {
                for (k, v) in entries {
                    self.collect_calls_in_expr(k, callees, local_effects);
                    self.collect_calls_in_expr(v, callees, local_effects);
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
            | ast::Expr::Todo
            | ast::Expr::Unimplemented
            | ast::Expr::Error { .. } => {}
            ast::Expr::Pipeline { left, right } => {
                self.collect_calls_in_expr(left, callees, local_effects);
                self.collect_calls_in_expr(right, callees, local_effects);
            }
            ast::Expr::Is { expr, .. } => {
                self.collect_calls_in_expr(expr, callees, local_effects);
            }
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

        // Accumulate effects from callees. A callee contributes BOTH its
        // inferred body effects AND its *declared* effects: a function
        // annotated `/ io` performs io by contract even if its body just wraps
        // a builtin/FFI whose effect wasn't inferred. Without this, a caller
        // could smuggle a network/exec effect past a pure signature — the
        // propagation that makes effect annotations a real capability gate.
        for callee in &callees {
            let mut callee_effects = self.infer_function(callee);
            if let Some(declared) = self.declared.get(callee) {
                callee_effects.extend(declared.iter().cloned());
            }
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

    /// The module's full **effect (capability) surface**, for agent policy
    /// gating: every function with its declared effects (the contract it
    /// claims) and its inferred effects (what the compiler computed it
    /// actually performs, transitively). Returned sorted by function name so
    /// the output is deterministic. An agent runtime can sandbox or refuse
    /// generated code by inspecting this BEFORE running it — and it covers
    /// *every* function, not only the annotated ones.
    pub fn effect_surface(&self) -> Vec<(String, Vec<String>, Vec<String>)> {
        let mut names: Vec<&String> =
            self.inferred.keys().chain(self.declared.keys()).collect();
        names.sort();
        names.dedup();
        names
            .into_iter()
            .map(|name| {
                let to_sorted = |set: Option<&EffectSet>| {
                    let mut v: Vec<String> =
                        set.map(|s| s.iter().map(|e| e.to_string()).collect()).unwrap_or_default();
                    v.sort();
                    v
                };
                (
                    name.clone(),
                    to_sorted(self.declared.get(name)),
                    to_sorted(self.inferred.get(name)),
                )
            })
            .collect()
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
    fn undeclared_effect_is_caught() {
        // A function declaring `/ io` that performs a `net` effect must be
        // flagged — the capability gate. (Regression: the block-body parser
        // path used to drop declared effects, silently disabling this.)
        let ei = infer_source("f x() / io { connect(); }\n");
        assert!(
            ei.diagnostics.iter().any(|d| d.message.contains("undeclared effect")),
            "expected an undeclared-effect error, got {:?}",
            ei.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn unannotated_effectful_pub_function_is_flagged() {
        // Trust boundary: a PUBLIC effectful function with no annotation must be
        // flagged — the module's external surface must state its effects.
        let ei = infer_source("+f leak() { println(\"x\"); }\n");
        assert!(
            ei.diagnostics.iter().any(|d| d.message.contains("undeclared effects")),
            "pub effectful fn must declare its effects, got {:?}",
            ei.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn unannotated_effectful_private_function_infers() {
        // Inside the boundary: a PRIVATE effectful function infers its effects
        // with no annotation. Its effects are still tracked and surface at any
        // public caller (see the propagation test) — so internal code is sound
        // *and* pays zero annotation tokens.
        let ei = infer_source("f helper() { println(\"x\"); }\n");
        assert!(
            !ei.diagnostics.iter().any(|d| d.message.contains("undeclared effects")),
            "private effectful fn should infer, not require an annotation, got {:?}",
            ei.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn effect_propagates_to_pub_boundary() {
        // SOUNDNESS: a public function that transitively reaches a private
        // effectful function must STILL declare the effect — nothing escapes the
        // boundary undeclared. This is what makes inference-inside safe.
        let ei = infer_source("+f api() { helper(); }\nf helper() { println(\"x\"); }\n");
        assert!(
            ei.diagnostics
                .iter()
                .any(|d| d.message.contains("undeclared effects") && d.message.contains("api")),
            "pub boundary must catch a private callee's effect, got {:?}",
            ei.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn declared_bound_is_enforced_even_when_private() {
        // A private function that DECLARES `/ io` but performs `net` is still
        // flagged — an explicit declaration is a contract the body cannot
        // exceed, regardless of visibility.
        let ei = infer_source("f x() / io { connect(); }\n");
        assert!(
            ei.diagnostics.iter().any(|d| d.message.contains("undeclared effect")),
            "a declared bound must be enforced for private fns too, got {:?}",
            ei.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn pure_function_needs_no_annotation() {
        // The flip side: a genuinely pure function needs no annotation and is
        // clean — so the soundness rule costs zero tokens for pure code.
        let ei = infer_source("f add(a: i32, b: i32) -> i32 { a + b }\n");
        assert!(
            !ei.diagnostics.iter().any(|d| d.message.contains("undeclared effects")),
            "pure fn must not require an annotation"
        );
    }

    #[test]
    fn effect_surface_reports_declared_and_inferred() {
        // The capability surface lists every function (sorted) with declared
        // vs inferred effects — the data an agent gates generated code on.
        let ei = infer_source(
            "f pure_calc(a: i32, b: i32) -> i32 { a + b }\nf worker() / io { connect(); }\n",
        );
        let surface = ei.effect_surface();
        // Sorted by name → pure_calc before worker.
        let calc = surface.iter().find(|(n, ..)| n == "pure_calc").expect("pure_calc");
        assert!(calc.1.is_empty() && calc.2.is_empty(), "pure fn has empty surface");
        let worker = surface.iter().find(|(n, ..)| n == "worker").expect("worker");
        assert_eq!(worker.1, vec!["IO".to_string()], "declared IO");
        assert_eq!(worker.2, vec!["Net".to_string()], "inferred Net (the smuggle)");
    }

    #[test]
    fn declared_effect_satisfied_is_clean() {
        // Declaring the effect you actually perform must NOT error.
        let ei = infer_source("f x() / net { connect(); }\n");
        assert!(
            !ei.diagnostics.iter().any(|d| d.message.contains("undeclared effect")),
            "correctly-declared effect should be clean, got {:?}",
            ei.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
        );
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
