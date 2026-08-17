/// MAGE Effect Inference — bottom-up effect set computation.
///
/// Implements the effect inference algorithm from MAGE_SPEC.md §11:
///   InferEffects(fn):
///     1. Collect all effect operations performed in fn body
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
    /// Per function, the `handle` regions found in its body.
    handled: HashMap<String, Vec<HandledRegion>>,
    /// Regions collected while walking the function currently being collected.
    /// Drained by `collect_function`; never meaningful between functions.
    pending_regions: Vec<HandledRegion>,
    /// Declared `effect` block names, lowercased.
    ///
    /// Lowercased because the two spellings of an effect are the declaration
    /// (`effect Audit`) and the annotation (`/ audit`), and they have to name
    /// the same thing. Keeping the fold here rather than at each use means the
    /// convention is stated once.
    effect_names: std::collections::HashSet<String>,
    /// Bare method name → the qualified `Type.method` keys that spell it.
    ///
    /// A method call has no receiver type at this stage, so a call is only
    /// attributed to a method when the name is unambiguous — one entry. With
    /// two implementations of `render`, neither is charged, which under-reports
    /// rather than blaming the wrong one.
    method_index: HashMap<String, Vec<String>>,
    pub diagnostics: Vec<Diagnostic>,
}

/// One `handle { … } with E { … }` occurrence.
///
/// The calls inside the handled block are kept *out* of the function's ordinary
/// callee list and recorded here instead, because their effects have to be
/// resolved and then have `effect` removed before they join the enclosing
/// function's set. Discharging by simply deleting the effect from the whole
/// function would be unsound — an unhandled call to the same operation
/// elsewhere in the body would be silenced along with it.
#[derive(Debug, Clone)]
struct HandledRegion {
    /// The effect this region discharges.
    effect: Effect,
    /// Functions called inside the handled block.
    callees: Vec<String>,
    /// Effects performed directly inside the handled block.
    local: EffectSet,
}

impl Default for EffectInfer {
    fn default() -> Self {
        Self::new()
    }
}

/// Every method in the module's `impl` and `extend` blocks, as
/// `(Type.method, definition, is_public)`.
///
/// `trait` blocks are skipped: their items are signatures, and a signature has
/// nothing to infer — the obligation belongs to the `impl` that supplies a body.
fn collect_methods(module: &ast::Module) -> Vec<(String, ast::FunctionDef, bool)> {
    let mut out = Vec::new();
    for item in &module.items {
        let (target, items) = match &item.kind {
            ast::ItemKind::Impl(ib) => (&ib.self_type, &ib.items),
            ast::ItemKind::Extend(eb) => (&eb.target_type, &eb.items),
            _ => continue,
        };
        let Some(type_name) = crate::eval::type_head_name(target) else {
            continue;
        };
        for member in items {
            if let ast::ItemKind::Function(fd) = &member.kind {
                out.push((
                    format!("{type_name}.{}", fd.name),
                    fd.clone(),
                    member.visibility == ast::Visibility::Public,
                ));
            }
        }
    }
    out
}

impl EffectInfer {
    pub fn new() -> Self {
        EffectInfer {
            declared: HashMap::new(),
            inferred: HashMap::new(),
            call_graph: HashMap::new(),
            in_progress: Vec::new(),
            handled: HashMap::new(),
            pending_regions: Vec::new(),
            effect_names: std::collections::HashSet::new(),
            method_index: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    // ── Module-level inference ────────────────────────────────────────

    pub fn infer_module(&mut self, module: &ast::Module) {
        // Trust boundaries: public functions (and `main`, the entry). Effect
        // declarations are required here; private functions infer silently —
        // their effects still propagate to any public caller (see Pass 3).
        let mut boundary: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Pass 0: the declared effects. Needed before anything else, because an
        // operation call introduces its effect and an annotation naming no
        // declared effect is an error — both need the full set up front.
        for item in &module.items {
            if let ast::ItemKind::Effect(ed) = &item.kind {
                self.effect_names.insert(ed.name.to_lowercase());
            }
        }

        // Pass 0.5: index the methods before any call graph is built, so a call
        // inside one function can name a method defined further down the file.
        //
        // `impl` and `extend` bodies were not collected at all: `--check` on a
        // module of nothing but methods printed "Functions analyzed: 0", and a
        // `pub fn` inside one could `fs.read_to_string(…)` while declaring
        // nothing. Every capability in the language is reached through exactly
        // the call shape that was unchecked.
        let methods = collect_methods(module);
        for (key, _, _) in &methods {
            let bare = key.split('.').next_back().unwrap_or(key).to_string();
            self.method_index.entry(bare).or_default().push(key.clone());
        }

        // Pass 1: collect function declarations and their call graphs.
        for item in &module.items {
            if let ast::ItemKind::Function(fd) = &item.kind {
                self.collect_function(&fd.name, fd);
                if item.visibility == ast::Visibility::Public || fd.name == "main" {
                    boundary.insert(fd.name.clone());
                }
            }
        }

        // Pass 1a: the methods themselves. A method is keyed `Type.method`, so
        // two types implementing one trait keep separate effect sets and no
        // method shadows a free function of the same name.
        for (key, fd, is_pub) in &methods {
            self.collect_function(key, fd);
            if *is_pub {
                boundary.insert(key.clone());
            }
        }

        // Pass 1.5: every effect named in an annotation must exist — a built-in
        // kind, or an `effect` block in this module.
        //
        // Anything else used to become `Effect::Custom` silently, so `/ nte`
        // was not a misspelling of `/ net` but a *different effect*, enforced
        // perfectly consistently and matching nothing. A typo invented an
        // effect rather than failing, which is the most expensive way for a
        // capability system to be wrong.
        let mut named: Vec<(&String, &EffectSet)> = self.declared.iter().collect();
        named.sort_by_key(|(n, _)| n.as_str());
        for (func, effects) in named {
            for effect in effects {
                let Effect::Custom(name) = effect else { continue };
                if self.effect_names.contains(&name.to_lowercase()) {
                    continue;
                }
                self.diagnostics.push(Diagnostic::categorized(
                    crate::hir::Severity::Error,
                    format!(
                        "function `{func}` declares unknown effect `{name}` — it is not \
                         a built-in kind, and no `effect {name} {{ … }}` declares it"
                    ),
                    DiagnosticCategory::UndeclaredEffect,
                    None,
                ));
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

    /// Collect one function under `key` — its own name for a free function,
    /// `Type.method` for a method.
    fn collect_function(&mut self, key: &str, fd: &ast::FunctionDef) {
        // Record declared effects from annotations.
        if !fd.effects.is_empty() {
            let effects: EffectSet = fd.effects.iter().map(|e| Effect::from_name(e)).collect();
            self.declared.insert(key.to_string(), effects);
        }

        // Also check attributes for @fx(...).
        // (Not implemented in parser attributes yet, but the hook is here.)

        // Build call graph for this function by walking its body.
        let mut callees = Vec::new();
        let mut local_effects = EffectSet::new();
        self.pending_regions.clear();
        self.collect_calls_in_block(&fd.body, &mut callees, &mut local_effects);
        let regions = std::mem::take(&mut self.pending_regions);
        if !regions.is_empty() {
            self.handled.insert(key.to_string(), regions);
        }

        self.call_graph.insert(key.to_string(), callees);

        // If the function performs effects directly, record them.
        if !local_effects.is_empty() {
            self.inferred.insert(key.to_string(), local_effects);
        }
    }

    fn collect_calls_in_block(
        &mut self,
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
        &mut self,
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
        &mut self,
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
                self.collect_calls_in_expr(func, callees, local_effects);
                for arg in args {
                    self.collect_calls_in_expr(arg, callees, local_effects);
                }
            }
            ast::Expr::MethodCall { receiver, method, args, .. } => {
                // `p.norm2()` reaches whatever `norm2` performs. The receiver's
                // type is not known here, so the edge is only drawn when the
                // method name is unambiguous in the module — otherwise nothing
                // is charged, which under-reports rather than blaming the
                // wrong implementation.
                if let Some(keys) = self.method_index.get(method)
                    && let [only] = keys.as_slice()
                {
                    callees.push(only.clone());
                }
                // `Audit.record(x)` performs `audit`. This is the introduction
                // rule: without it an `effect` block declared operations that
                // no analysis attributed to anyone, so a function calling one
                // inferred `pure` while claiming `/ audit`.
                if let ast::Expr::Ident { name } = receiver.as_ref() {
                    let lowered = name.to_lowercase();
                    if self.effect_names.contains(&lowered) {
                        let _ = method;
                        local_effects.insert(Effect::from_name(&lowered));
                    } else if let Some((_, Some(effect))) = crate::hir::CAPABILITY_NAMESPACES
                        .iter()
                        .find(|(ns, _)| *ns == name)
                    {
                        // `io.println(x)` performs `io`, by the same rule one
                        // line up: the receiver names the capability, so the
                        // capability is what gets attributed.
                        //
                        // This is the seam the effect system was missing. The
                        // capability handles are documented as *the* way to
                        // perform a side effect, and they were the one call
                        // shape that attributed nothing — a `pub` function
                        // could `net.connect(…)` or `llm.generate(…)` and
                        // still typecheck as pure, while the bare `println(…)`
                        // beside it was caught. A gate open at the documented
                        // entrance is not a gate.
                        //
                        // A *declared* effect wins the name: `effect Io { … }`
                        // in the module is that module's `io`, checked against
                        // its own operation list.
                        local_effects.insert(effect.clone());
                    }
                }
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
            // The handled block's calls go into a *separate* bucket, so the
            // effect can be removed from them alone. The arms are different:
            // they really do run in the enclosing function, so whatever they
            // perform belongs to it — handling `audit` by writing a file is
            // honestly reported as `/ fs`.
            ast::Expr::Handle { body, effect, arms } => {
                let mut inner_callees = Vec::new();
                let mut inner_local = EffectSet::new();
                self.collect_calls_in_block(body, &mut inner_callees, &mut inner_local);
                self.pending_regions.push(HandledRegion {
                    effect: Effect::from_name(&effect.to_lowercase()),
                    callees: inner_callees,
                    local: inner_local,
                });
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
        // The standard vocabulary is capability-pure by declaration
        // (`resolve::VOCABULARY`, "SINGLE SOURCE OF TRUTH"), and this table
        // matches on a bare name — so any collision silently outranked it.
        // `join` collided: the vocabulary's `([str], str) -> str` string join
        // was read as a thread join and attributed `Async`, which made every
        // caller of a *pure* documented function fail the effect check unless
        // it declared `/ async`. Deferring to the vocabulary fixes that case
        // and the next collision anyone adds to either list.
        if crate::resolve::VOCABULARY.iter().any(|(v, _, _)| *v == name) {
            return;
        }
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

        // Handled regions, resolved the same way and then discharged. The
        // subtraction happens per region rather than over the whole function,
        // so a second, unhandled call to the same effect elsewhere in the body
        // still surfaces.
        for region in self.handled.get(name).cloned().unwrap_or_default() {
            let mut region_effects = region.local.clone();
            for callee in &region.callees {
                let mut callee_effects = self.infer_function(callee);
                if let Some(declared) = self.declared.get(callee) {
                    callee_effects.extend(declared.iter().cloned());
                }
                region_effects.extend(callee_effects);
            }
            region_effects.remove(&region.effect);
            effects.extend(region_effects);
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
    fn standard_vocabulary_is_pure() {
        // `join` sat in both `VOCABULARY` (`([str], str) -> str`, pure) and the
        // Async builtin list (thread join). The bare-name match meant the
        // string join inherited `Async`, so any caller of a documented-pure
        // function failed the effect check. Assert over the whole vocabulary,
        // not just `join`: the bug was a collision, and the next one added to
        // either list should fail here rather than in someone's example.
        for (name, sig, _) in crate::resolve::VOCABULARY {
            let ei = infer_source(&format!("f t() {{ {name} }}"));
            assert!(
                ei.effects_of("t").is_empty(),
                "vocabulary function `{name}` ({sig}) is documented pure but \
                 inferred {:?} — a name collision in check_builtin_effect",
                ei.effects_of("t")
            );
        }
    }

    #[test]
    fn effectful_builtins_are_still_effectful() {
        // The vocabulary deference must not blanket-silence the builtin table:
        // names outside the vocabulary keep their effects.
        assert!(infer_source("f t() { spawn(1) }").effects_of("t").contains(&Effect::Async));
        assert!(infer_source("f t() { println(1) }").effects_of("t").contains(&Effect::IO));
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

#[cfg(test)]
mod handler_tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn infer_source(src: &str) -> EffectInfer {
        let tokens = lexer::lex(src);
        let module = parser::parse(&tokens).expect("parse failed");
        infer_effects(&module)
    }

    fn errors(src: &str) -> Vec<String> {
        infer_source(src)
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect()
    }

    const DECL: &str = "effect Audit { f record(e: str) -> usize; }\n";

    /// The introduction rule. An `effect` block used to declare operations that
    /// no analysis attributed to anyone, so a function calling one inferred
    /// `pure` while claiming `/ audit`.
    #[test]
    fn performing_an_operation_introduces_its_effect() {
        let src = format!("{DECL}+f w(e: str) -> usize {{ Audit.record(e) }}");
        let msgs = errors(&src);
        assert!(
            msgs.iter().any(|m| m.contains("undeclared effects") && m.contains("audit")),
            "expected `audit` to be attributed to `w`, got {msgs:?}"
        );
    }

    /// The elimination rule: the whole point. `main` is pure despite calling
    /// something that performs `audit`.
    #[test]
    fn handle_discharges_the_effect_it_names() {
        let src = format!(
            "{DECL}f w(e: str) -> usize / audit {{ Audit.record(e) }}\n\
             +f main() -> usize {{ handle {{ w(\"x\") }} with Audit {{ record(e) => len(chars(e)) }} }}"
        );
        assert!(errors(&src).is_empty(), "errors: {:?}", errors(&src));
    }

    /// The soundness property. Discharging by deleting the effect from the
    /// whole function would silence this second, *unhandled* call — so the
    /// subtraction is per handled block instead.
    #[test]
    fn an_unhandled_call_beside_a_handled_one_still_reports() {
        let src = format!(
            "{DECL}f w(e: str) -> usize / audit {{ Audit.record(e) }}\n\
             +f main() -> usize {{ v a = handle {{ w(\"x\") }} with Audit {{ record(e) => 1 }}\n\
             v b = w(\"y\")\n a + b }}"
        );
        let msgs = errors(&src);
        assert!(
            msgs.iter().any(|m| m.contains("audit")),
            "the unhandled call must still surface, got {msgs:?}"
        );
    }

    /// A handler is not free: handling `audit` by touching the filesystem makes
    /// the handling function perform `fs`.
    #[test]
    fn a_handlers_own_effects_are_attributed_to_the_handling_function() {
        let src = format!(
            "{DECL}f w(e: str) -> usize / audit {{ Audit.record(e) }}\n\
             f to_disk(e: str) -> usize / fs {{ 1 }}\n\
             +f main() -> usize {{ handle {{ w(\"x\") }} with Audit {{ record(e) => to_disk(e) }} }}"
        );
        let msgs = errors(&src);
        assert!(
            msgs.iter().any(|m| m.contains("FS")),
            "expected the handler's `fs` to surface, got {msgs:?}"
        );
    }

    /// An effect annotation naming nothing used to be accepted, so `/ nte` was
    /// a different effect rather than a misspelling of `/ net`.
    #[test]
    fn an_effect_annotation_naming_nothing_is_an_error() {
        let msgs = errors("+f a() -> i32 / nte { 1 }");
        assert!(
            msgs.iter().any(|m| m.contains("unknown effect")),
            "expected an unknown-effect diagnostic, got {msgs:?}"
        );
    }

    /// A misspelled operation. The effect analysis attributes the effect from
    /// the *receiver* alone, so this was counted as performing `audit`, checked
    /// clean, and then died at run time with `unknown function` — the same bug
    /// one level down from the one this feature exists to fix.
    #[test]
    fn a_misspelled_operation_on_a_declared_effect_is_rejected() {
        let tokens = crate::lexer::lex(&format!("{DECL}+f a() -> i32 / audit {{ Audit.recrod(1) }}"));
        let module = crate::parser::parse(&tokens).expect("parse failed");
        let tc = crate::types::check(&module);
        assert!(
            tc.diagnostics.iter().any(|d| d.message.contains("declares no operation")),
            "expected a misspelled-operation diagnostic, got {:?}",
            tc.diagnostics
        );
    }

    #[test]
    fn a_declared_custom_effect_is_accepted() {
        assert!(errors("effect Db {}\n+f a() -> i32 / db { 1 }").is_empty());
    }

    /// Built-in kinds need no declaration — the rule is about names that mean
    /// nothing, not about forcing boilerplate for `fs`.
    #[test]
    fn builtin_effect_kinds_need_no_declaration() {
        assert!(errors("+f a() -> i32 / fs, net, rng { 1 }").is_empty());
    }

    /// Every effect `MAGE_SPEC.md` §11.2 documents, written the way the spec
    /// says to write it.
    ///
    /// `agent` was in that table and was a **parse error**: `agent` lexes as
    /// the keyword introducing an `agent` item, so `/ agent` never reached the
    /// checker, and it was not a built-in kind either — two independent
    /// failures on the one row. The spec had advertised an effect nobody could
    /// write. Nothing had ever run the other sixteen to find out.
    ///
    /// This is the pin: a row added to §11.2 that the compiler does not accept
    /// fails here, and so does a name quietly dropped from `Effect::from_name`.
    #[test]
    fn every_effect_documented_in_the_spec_parses_and_checks() {
        const SPEC_11_2: [&str; 17] = [
            "io", "net", "fs", "async", "alloc", "panic", "ffi", "env", "time", "gpu", "npu",
            "llm", "evolve", "learn", "rng", "agent", "proc",
        ];
        for effect in SPEC_11_2 {
            let msgs = errors(&format!("+f a() -> i32 / {effect} {{ 1 }}"));
            assert!(
                msgs.is_empty(),
                "`/ {effect}` is documented in MAGE_SPEC.md §11.2 but rejected: {msgs:?}"
            );
        }
    }

    /// The builtin names `MAGE_SPEC.md` §11.2 says are attributed on call, and
    /// the documented names it says are *not*.
    ///
    /// §11.2 used to present its middle column as "Operations", which read as a
    /// table of callable operations. Running all 41 of them found 22 that
    /// perform nothing: `dispatch`, `generate`, `lifecycle` and the rest have
    /// no `effect` block behind them and are attributed by nothing, so a
    /// function calling them is pure. The spec now says domain, and lists the
    /// names that really are attributed — this is what holds it to that.
    #[test]
    fn the_builtin_names_attributed_on_call_are_the_documented_ones() {
        let attributed: &[(&str, &str)] = &[
            ("println", "IO"), ("read_to_string", "IO"), ("write", "IO"),
            ("mkdir", "FS"), ("stat", "FS"), ("rename", "FS"),
            ("connect", "Net"), ("recv", "Net"), ("bind", "Net"),
            ("spawn", "Async"), ("select", "Async"),
            ("realloc", "Alloc"), ("panic", "Panic"),
            ("set_env", "Env"), ("timeout", "Time"),
        ];
        for (name, effect) in attributed {
            let inferred = infer_source(&format!("f a() -> i32 {{ {name}(); 0 }}"))
                .inferred
                .get("a")
                .cloned()
                .unwrap_or_else(pure);
            assert!(
                inferred.iter().any(|e| e.to_string() == *effect),
                "`{name}()` should perform {effect} per §11.2, inferred {inferred:?}"
            );
        }

        // Documented in §11.2's domain column, attributed by nothing. If one of
        // these starts inferring an effect, §11.2's second table is now wrong.
        // `join` is here for a different reason: the vocabulary's pure string
        // join outranks the thread join, and §11.2 says so.
        let pure_names = [
            "join", "seek", "close", "catch_panic", "call_foreign", "get_var", "set_var",
            "dispatch", "synchronize", "generate", "embed", "analyze", "evaluate", "mutate",
            "forward", "backward", "step", "random", "seed", "sample", "lifecycle", "message",
            "lease",
        ];
        for name in pure_names {
            let inferred = infer_source(&format!("f a() -> i32 {{ {name}(); 0 }}"))
                .inferred
                .get("a")
                .cloned()
                .unwrap_or_else(pure);
            assert!(
                inferred.is_empty(),
                "`{name}()` is documented as attributing nothing, but inferred {inferred:?}"
            );
        }
    }

    /// A capability handle performs its capability's effect.
    ///
    /// This is the hole this table exists to close. `resolve.rs` registered the
    /// capability namespaces and its comment said their "use is tracked by the
    /// effect system"; nothing tracked them. A `pub` function declared pure
    /// could call `net.connect(…)`, `llm.generate(…)` or `process.spawn(…)` and
    /// check clean — while the bare `println(…)` beside it was caught. The gate
    /// was open at precisely the seam the language documents as the way
    /// through it, which is the worst place for a capability system to be
    /// wrong: the safe-looking code is the code that isn't checked.
    #[test]
    fn a_capability_handle_performs_its_effect() {
        for (namespace, effect) in crate::hir::CAPABILITY_NAMESPACES {
            let src = format!("+f a(x: str) -> i32 {{ {namespace}.op(x); 0 }}");
            let msgs = errors(&src);
            match effect {
                Some(e) => assert!(
                    msgs.iter().any(|m| m.contains(&e.to_string())),
                    "`{namespace}.op(…)` must perform {e}, got {msgs:?}"
                ),
                // Deliberately unattributed — see the table's own comment.
                None => assert!(
                    msgs.is_empty(),
                    "`{namespace}` is recorded as performing nothing, but got {msgs:?}"
                ),
            }
        }
    }

    /// A method body is checked like any other body.
    ///
    /// It was not checked at all: `impl` and `extend` items never reached the
    /// collector, so `--check` on a module of methods reported "Functions
    /// analyzed: 0" and a `pub fn` inside one could read the filesystem while
    /// declaring nothing. Both block forms, because they are separate AST nodes
    /// and fixing one would leave the other open.
    #[test]
    fn a_method_must_declare_what_it_performs() {
        for block in ["extend P", "impl P"] {
            let src = format!(
                "+S P {{ x: f64 }}\n\
                 {block} {{\n\
                 +f leak(self, p: str) -> str {{ fs.read_to_string(p) }}\n\
                 }}"
            );
            let msgs = errors(&src);
            assert!(
                msgs.iter().any(|m| m.contains("P.leak") && m.contains("FS")),
                "`{block}` method performing fs must be reported, got {msgs:?}"
            );
        }
        // And declaring it satisfies the check.
        assert!(
            errors(
                "+S P { x: f64 }\n\
                 extend P {\n\
                 +f read(self, p: str) -> str / fs { fs.read_to_string(p) }\n\
                 }"
            )
            .is_empty()
        );
    }

    /// Calling a method reaches what the method performs.
    #[test]
    fn a_method_call_propagates_the_methods_effects() {
        let msgs = errors(
            "+S P { x: f64 }\n\
             extend P {\n\
             f read(self, p: str) -> str / fs { fs.read_to_string(p) }\n\
             }\n\
             +f main() -> str { v q = @P { x: 1.0 }\n q.read(\"a\") }",
        );
        assert!(
            msgs.iter().any(|m| m.contains("`main`") && m.contains("FS")),
            "main reaches fs through the method, got {msgs:?}"
        );
    }

    /// Declaring the effect satisfies the check — the gate is a gate, not a ban.
    #[test]
    fn a_declared_capability_handle_checks_clean() {
        assert!(errors("+f a(s: str) -> i32 / io { io.println(s); 0 }").is_empty());
        assert!(errors("+f a(p: str) -> i32 / llm { llm.generate(p); 0 }").is_empty());
        assert!(errors("+f a(c: str) -> i32 / proc { process.spawn(c); 0 }").is_empty());
    }

    /// A module's own `effect Io { … }` outranks the built-in capability, the
    /// same way it already outranks a builtin function name.
    #[test]
    fn a_declared_effect_block_wins_the_namespace() {
        let msgs = errors(
            "effect Io { f emit(s: str) -> i32; }\n+f a(s: str) -> i32 / io { Io.emit(s) }",
        );
        assert!(msgs.is_empty(), "declared `effect Io` should win, got {msgs:?}");
    }

    /// Every capability namespace `resolve.rs` registers must be in the table
    /// that decides its effect — they are one list precisely so a namespace
    /// cannot be added without that decision being made.
    #[test]
    fn every_capability_namespace_resolves_as_a_name() {
        for (namespace, _) in crate::hir::CAPABILITY_NAMESPACES {
            let src = format!("+f a(x: str) -> i32 / proc, io, net, llm, agent, alloc, \
                               env, time, rng, gpu, fs {{ {namespace}.op(x); 0 }}");
            let tokens = crate::lexer::lex(&src);
            let module = crate::parser::parse(&tokens)
                .unwrap_or_else(|e| panic!("`{namespace}.op(…)` failed to parse: {e:?}"));
            let diags = crate::resolve::resolve(&module).diagnostics;
            assert!(
                !diags.iter().any(|d| d.message.contains(namespace)),
                "`{namespace}` is registered but does not resolve: {:?}",
                diags.iter().map(|d| &d.message).collect::<Vec<_>>()
            );
        }
    }

    /// A keyword is accepted as an effect *name*, not as an effect *kind*: the
    /// annotation position stopped being the gate, so the unknown-effect check
    /// is what must still reject a name that means nothing.
    #[test]
    fn a_keyword_that_is_not_a_builtin_effect_is_still_rejected() {
        let msgs = errors("+f a() -> i32 / trait { 1 }");
        assert!(
            msgs.iter().any(|m| m.contains("unknown effect")),
            "expected an unknown-effect diagnostic, got {msgs:?}"
        );
    }
}
