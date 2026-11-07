/// MAGE Formatter (magefmt) — bidirectional pretty-printer.
///
/// Two modes:
///   `agent`   — emit minimal-token MAGE canonical form (sigils: f, +f, S, v, m …)
///   `human`   — emit human-readable MAGE form (def, exp def, rec, val, var …)
///
/// Both modes round-trip losslessly through the AST: the semantic content is
/// identical; only the surface syntax differs.
use crate::ast::*;

// ── Public API ───────────────────────────────────────────────────────

/// Format a module in agent (minimal-token) MAGE syntax.
pub fn format_agent(module: &Module) -> String {
    let mut buf = String::new();
    for (i, item) in module.items.iter().enumerate() {
        if i > 0 {
            buf.push('\n');
        }
        emit_item(&mut buf, item, Mode::Agent, 0);
    }
    buf
}

/// Format a module in human (human-readable MAGE) syntax.
pub fn format_human(module: &Module) -> String {
    let mut buf = String::new();
    for (i, item) in module.items.iter().enumerate() {
        if i > 0 {
            buf.push('\n');
        }
        emit_item(&mut buf, item, Mode::Human, 0);
    }
    buf
}

// ── Mode ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Agent,
    Human,
}

// ── Indentation helper ───────────────────────────────────────────────

fn indent(buf: &mut String, depth: usize) {
    for _ in 0..depth {
        buf.push_str("    ");
    }
}

// ── Item emission ────────────────────────────────────────────────────

fn emit_item(buf: &mut String, item: &Item, mode: Mode, depth: usize) {
    for attr in &item.attributes {
        indent(buf, depth);
        buf.push('@');
        if attr.bang {
            buf.push('!');
        }
        buf.push_str(&attr.name);
        if !attr.args.is_empty() {
            buf.push('(');
            buf.push_str(&attr.args.join(", "));
            buf.push(')');
        }
        buf.push('\n');
    }

    match &item.kind {
        ItemKind::Function(f) => emit_function(buf, f, item.visibility, mode, depth),
        ItemKind::Struct(s) => emit_struct(buf, s, item.visibility, mode, depth),
        ItemKind::Enum(e) => emit_enum(buf, e, item.visibility, mode, depth),
        ItemKind::Trait(t) => emit_trait(buf, t, item.visibility, mode, depth),
        ItemKind::Impl(i) => emit_impl(buf, i, mode, depth),
        ItemKind::Module(m) => emit_module(buf, m, item.visibility, mode, depth),
        ItemKind::Use(u) => emit_use(buf, u, item.visibility, mode, depth),
        ItemKind::TypeAlias(ta) => emit_type_alias(buf, ta, item.visibility, mode, depth),
        ItemKind::Const(c) => emit_const(buf, c, item.visibility, mode, depth),
        ItemKind::Static(s) => emit_static(buf, s, item.visibility, mode, depth),
        ItemKind::Effect(e) => emit_effect(buf, e, mode, depth),
        ItemKind::Spec(s) => emit_spec(buf, s, mode, depth),
        ItemKind::Agent(a) => emit_agent(buf, a, depth),
        ItemKind::Net(n) => emit_net(buf, n, mode, depth),
        ItemKind::Kb(k) => emit_kb(buf, k, mode, depth),
        ItemKind::Evolve(e) => emit_evolve(buf, e, mode, depth),
        ItemKind::Train(t) => emit_train(buf, t, mode, depth),
        ItemKind::Swarm(s) => emit_swarm(buf, s, mode, depth),
        ItemKind::Data(d) => emit_data(buf, d, item.visibility, mode, depth),
        ItemKind::Extend(e) => emit_extend(buf, e, mode, depth),
    }
}

// ── Functions ────────────────────────────────────────────────────────

fn emit_function(buf: &mut String, f: &FunctionDef, vis: Visibility, mode: Mode, depth: usize) {
    for c in &f.contracts {
        indent(buf, depth);
        emit_contract(buf, c);
        buf.push('\n');
    }

    indent(buf, depth);
    // The parser takes one function keyword per item, and `async` (`af`) and
    // `uf` are alternatives to `f`, so an async function drops `unsafe` (the
    // parser cannot produce both flags). `unsafe` has no keyword of its own:
    // the parser reads `uf` only, so both modes print `uf`.
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push('+');
            }
            if f.is_async {
                buf.push_str("af ");
            } else if f.is_unsafe {
                buf.push_str("uf ");
            } else {
                buf.push_str("f ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            if f.is_async {
                // `async` is the function keyword itself: `async name(…)`,
                // with no `fn` before the name.
                buf.push_str("async ");
            } else if f.is_unsafe {
                buf.push_str("uf ");
            } else {
                buf.push_str("fn ");
            }
        }
    }

    buf.push_str(&f.name);
    emit_generics(buf, &f.generics);
    buf.push('(');
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 {
            buf.push_str(", ");
        }
        emit_param(buf, p, mode);
    }
    buf.push(')');

    if let Some(ref rt) = f.return_type {
        buf.push_str(" -> ");
        match (mode, rt) {
            // The parser reads `T or E` as a result only after `->`.
            (Mode::Human, Type::Result { ok, err }) => {
                emit_type(buf, ok, mode);
                buf.push_str(" or ");
                emit_type(buf, err, mode);
            }
            _ => emit_type(buf, rt, mode),
        }
    }

    emit_where_clause(buf, &f.where_clause, mode);

    if !f.effects.is_empty() {
        // Emit the `/ e1 + e2` form the parser accepts (round-trippable).
        // The old `@fx(...)` rendering did not re-parse as a function effect
        // annotation — caught by the formatter-idempotence property test.
        buf.push_str(" / ");
        buf.push_str(&f.effects.join(" + "));
    }

    if f.body.stmts.is_empty() && f.body.tail_expr.is_none() && f.body_expr.is_none() {
        buf.push_str(" {}");
    } else if let Some(ref expr) = f.body_expr {
        buf.push_str(" = ");
        emit_expr(buf, expr, mode);
    } else {
        buf.push_str(" {\n");
        emit_block_body(buf, &f.body, mode, depth + 1);
        indent(buf, depth);
        buf.push('}');
    }
    buf.push('\n');
}

fn emit_param(buf: &mut String, p: &Param, mode: Mode) {
    buf.push_str(&p.name);
    buf.push_str(": ");
    emit_type(buf, &p.ty, mode);
    if let Some(ref default) = p.default {
        buf.push_str(" = ");
        emit_expr(buf, default, mode);
    }
}

// ── Structs ──────────────────────────────────────────────────────────

fn emit_struct(buf: &mut String, s: &StructDef, vis: Visibility, mode: Mode, depth: usize) {
    for c in &s.contracts {
        indent(buf, depth);
        emit_contract(buf, c);
        buf.push('\n');
    }

    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+S ");
            } else {
                buf.push_str("S ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("struct ");
        }
    }
    buf.push_str(&s.name);
    emit_generics(buf, &s.generics);

    if s.fields.is_empty() {
        buf.push_str(" {}\n");
    } else {
        buf.push_str(" {\n");
        for field in &s.fields {
            indent(buf, depth + 1);
            if field.visibility == Visibility::Public {
                match mode {
                    Mode::Agent => buf.push('+'),
                    Mode::Human => buf.push_str("pub "),
                }
            }
            buf.push_str(&field.name);
            buf.push_str(": ");
            emit_type(buf, &field.ty, mode);
            buf.push_str(",\n");
        }
        indent(buf, depth);
        buf.push_str("}\n");
    }
}

// ── Enums ────────────────────────────────────────────────────────────

fn emit_enum(buf: &mut String, e: &EnumDef, vis: Visibility, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+E ");
            } else {
                buf.push_str("E ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("enum ");
        }
    }
    buf.push_str(&e.name);
    emit_generics(buf, &e.generics);
    buf.push_str(" {\n");
    for v in &e.variants {
        indent(buf, depth + 1);
        buf.push_str(&v.name);
        match &v.kind {
            VariantKind::Unit => {}
            VariantKind::Tuple(types) => {
                buf.push('(');
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    emit_type(buf, ty, mode);
                }
                buf.push(')');
            }
            VariantKind::Struct(fields) => {
                buf.push_str(" {\n");
                for f in fields {
                    indent(buf, depth + 2);
                    buf.push_str(&f.name);
                    buf.push_str(": ");
                    emit_type(buf, &f.ty, mode);
                    buf.push_str(",\n");
                }
                indent(buf, depth + 1);
                buf.push('}');
            }
        }
        buf.push_str(",\n");
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

// ── Traits ───────────────────────────────────────────────────────────

fn emit_trait(buf: &mut String, t: &TraitDef, vis: Visibility, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+T ");
            } else {
                buf.push_str("T ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("trait ");
        }
    }
    buf.push_str(&t.name);
    emit_generics(buf, &t.generics);
    if !t.super_traits.is_empty() {
        buf.push_str(": ");
        buf.push_str(&t.super_traits.join(" + "));
    }
    buf.push_str(" {\n");
    for item in &t.items {
        emit_item(buf, item, mode, depth + 1);
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

// ── Impl ─────────────────────────────────────────────────────────────

fn emit_impl(buf: &mut String, im: &ImplBlock, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("I "),
        Mode::Human => buf.push_str("impl "),
    }
    emit_generics(buf, &im.generics);
    if let Some(ref tp) = im.trait_path {
        buf.push_str(&tp.join("."));
        match mode {
            Mode::Agent => buf.push_str(" for "),
            Mode::Human => buf.push_str(" for "),
        }
    }
    emit_type(buf, &im.self_type, mode);
    buf.push_str(" {\n");
    for item in &im.items {
        emit_item(buf, item, mode, depth + 1);
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

// ── Modules ──────────────────────────────────────────────────────────

fn emit_module(buf: &mut String, m: &ModuleDef, vis: Visibility, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+M ");
            } else {
                buf.push_str("M ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("mod ");
        }
    }
    buf.push_str(&m.name);
    match &m.items {
        Some(items) => {
            buf.push_str(" {\n");
            for item in items {
                emit_item(buf, item, mode, depth + 1);
            }
            indent(buf, depth);
            buf.push_str("}\n");
        }
        None => buf.push_str(";\n"),
    }
}

// ── Use ──────────────────────────────────────────────────────────────

fn emit_use(buf: &mut String, u: &UseDef, vis: Visibility, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+u ");
            } else {
                buf.push_str("u ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("use ");
        }
    }
    buf.push_str(&u.path.join("."));
    if u.glob {
        buf.push_str("::*");
    }
    if let Some(ref alias) = u.alias {
        buf.push_str(" as ");
        buf.push_str(alias);
    }
    if !u.group.is_empty() {
        buf.push_str("::{");
        for (i, g) in u.group.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            buf.push_str(&g.path.join("."));
        }
        buf.push('}');
    }
    buf.push_str(";\n");
}

// ── Type alias ───────────────────────────────────────────────────────

fn emit_type_alias(buf: &mut String, ta: &TypeAlias, vis: Visibility, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+Y ");
            } else {
                buf.push_str("Y ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("type ");
        }
    }
    buf.push_str(&ta.name);
    emit_generics(buf, &ta.generics);
    buf.push_str(" = ");
    emit_type(buf, &ta.ty, mode);
    if let Some(ref r) = ta.refinement {
        buf.push_str(" ~> ");
        buf.push_str(r);
    }
    buf.push_str(";\n");
}

// ── Const ────────────────────────────────────────────────────────────

fn emit_const(buf: &mut String, c: &ConstDef, vis: Visibility, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+C ");
            } else {
                buf.push_str("C ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("const ");
        }
    }
    buf.push_str(&c.name);
    buf.push_str(": ");
    emit_type(buf, &c.ty, mode);
    buf.push_str(" = ");
    emit_expr(buf, &c.value, mode);
    buf.push_str(";\n");
}

// ── Static ───────────────────────────────────────────────────────────

fn emit_static(buf: &mut String, s: &StaticDef, vis: Visibility, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+Z ");
            } else {
                buf.push_str("Z ");
            }
            if s.mutable {
                buf.push_str("m ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("static ");
            if s.mutable {
                buf.push_str("mut ");
            }
        }
    }
    buf.push_str(&s.name);
    buf.push_str(": ");
    emit_type(buf, &s.ty, mode);
    buf.push_str(" = ");
    emit_expr(buf, &s.value, mode);
    buf.push_str(";\n");
}

// ── Effect ───────────────────────────────────────────────────────────

fn emit_effect(buf: &mut String, e: &EffectDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("fx "),
        Mode::Human => buf.push_str("effect "),
    }
    buf.push_str(&e.name);
    buf.push_str(" {\n");
    for op in &e.operations {
        // Each operation is a function signature: keyword, name, `(params)`,
        // optional `-> type`, and a `;`. The parser requires all of them.
        indent(buf, depth + 1);
        match mode {
            Mode::Agent => buf.push_str("f "),
            Mode::Human => buf.push_str("fn "),
        }
        buf.push_str(&op.name);
        buf.push('(');
        for (i, p) in op.params.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            emit_param(buf, p, mode);
        }
        buf.push(')');
        if let Some(ref rt) = op.return_type {
            buf.push_str(" -> ");
            emit_type(buf, rt, mode);
        }
        buf.push_str(";\n");
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

// ── Spec ─────────────────────────────────────────────────────────────

fn emit_spec(buf: &mut String, s: &SpecDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("sp "),
        Mode::Human => buf.push_str("spec "),
    }
    buf.push_str(&s.name);
    emit_generics(buf, &s.generics);
    if !s.params.is_empty() {
        buf.push('(');
        for (i, p) in s.params.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            emit_param(buf, p, mode);
        }
        buf.push(')');
    }
    if let Some(ref rt) = s.return_type {
        buf.push_str(" -> ");
        emit_type(buf, rt, mode);
    }
    buf.push_str(" {\n");
    for item in &s.items {
        indent(buf, depth + 1);
        match item {
            SpecItem::Require(c, _) => {
                buf.push_str("@req(");
                buf.push_str(c);
                buf.push(')');
            }
            SpecItem::Ensure(c, _) => {
                buf.push_str("@ens(");
                buf.push_str(c);
                buf.push(')');
            }
            SpecItem::Invariant(c, _) => {
                buf.push_str("@inv(");
                buf.push_str(c);
                buf.push(')');
            }
            SpecItem::Effect(effs) => {
                buf.push_str("@fx(");
                buf.push_str(&effs.join(", "));
                buf.push(')');
            }
            SpecItem::Performance(m, b) => {
                buf.push_str("@perf(");
                buf.push_str(m);
                buf.push_str(", ");
                buf.push_str(b);
                buf.push(')');
            }
        }
        buf.push('\n');
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

// ── Agent ────────────────────────────────────────────────────────────

fn emit_agent(buf: &mut String, a: &AgentDef, depth: usize) {
    indent(buf, depth);
    // `agent` in both modes: the parser has no rule for the `α` sigil.
    buf.push_str("agent ");
    buf.push_str(&a.name);
    buf.push_str(" {\n");
    indent(buf, depth + 1);
    buf.push_str("capabilities: [");
    buf.push_str(&a.capabilities.join(", "));
    buf.push_str("]\n");
    if !a.requires_approval.is_empty() {
        indent(buf, depth + 1);
        buf.push_str("requires_approval: [");
        buf.push_str(&a.requires_approval.join(", "));
        buf.push_str("]\n");
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

fn emit_net(buf: &mut String, n: &NetDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    // `net` in both modes: the parser has no rule for the `Ψ` sigil.
    buf.push_str("net ");
    buf.push_str(&n.name);
    emit_generics(buf, &n.generics);
    buf.push_str(" {\n");
    match &n.composition {
        None => {
            for l in &n.layers {
                emit_layer(buf, l, mode, depth + 1);
            }
        }
        Some(tree) => {
            // Layers the composition tree does not name are declared first, so
            // none is dropped. The parser never produces such a net.
            let mut named = Vec::new();
            compose_layer_names(tree, &mut named);
            for l in n.layers.iter().filter(|l| !named.contains(&l.name.as_str())) {
                emit_layer(buf, l, mode, depth + 1);
            }
            emit_compose(buf, tree, &n.layers, mode, depth + 1);
        }
    }
    indent(buf, depth + 1);
    buf.push_str("forward ");
    emit_braced_block(buf, &n.forward, mode, depth + 1);
    buf.push('\n');
    indent(buf, depth);
    buf.push_str("}\n");
}

/// One `layer name: Type(args);` declaration inside a `net`.
fn emit_layer(buf: &mut String, l: &LayerDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    // `layer` in both modes: the parser has no rule for the `λ` sigil.
    buf.push_str("layer ");
    buf.push_str(&l.name);
    buf.push_str(": ");
    emit_type(buf, &l.layer_type, mode);
    if !l.args.is_empty() {
        buf.push('(');
        emit_expr_list(buf, &l.args, mode);
        buf.push(')');
    }
    buf.push_str(";\n");
}

/// The dataflow tree of a `net` (`residual`, `wrap`, `branch`), with each
/// `Compose::Layer` leaf printed as the layer declaration it names. The parser
/// reads a declaration at a leaf's position, so the tree round-trips.
fn emit_compose(
    buf: &mut String,
    items: &[Compose],
    layers: &[LayerDef],
    mode: Mode,
    depth: usize,
) {
    for c in items {
        match c {
            Compose::Layer(name) => {
                if let Some(l) = layers.iter().find(|l| &l.name == name) {
                    emit_layer(buf, l, mode, depth);
                }
            }
            Compose::Residual(inner) => {
                indent(buf, depth);
                buf.push_str("residual ");
                emit_compose_block(buf, inner, layers, mode, depth);
                buf.push('\n');
            }
            Compose::Wrap(op, inner) => {
                indent(buf, depth);
                buf.push_str("wrap ");
                buf.push_str(op);
                buf.push(' ');
                emit_compose_block(buf, inner, layers, mode, depth);
                buf.push('\n');
            }
            Compose::Branch(paths) => {
                indent(buf, depth);
                buf.push_str("branch");
                for path in paths {
                    buf.push(' ');
                    emit_compose_block(buf, path, layers, mode, depth);
                }
                buf.push('\n');
            }
        }
    }
}

/// A `{ … }` combinator body: the opening brace on the current line, the
/// closing brace at `depth`. The parser reads one `{ … }` per `branch` path.
fn emit_compose_block(
    buf: &mut String,
    items: &[Compose],
    layers: &[LayerDef],
    mode: Mode,
    depth: usize,
) {
    if items.is_empty() {
        buf.push_str("{}");
        return;
    }
    buf.push_str("{\n");
    emit_compose(buf, items, layers, mode, depth + 1);
    indent(buf, depth);
    buf.push('}');
}

/// Collect the layer names a composition tree refers to.
fn compose_layer_names<'a>(items: &'a [Compose], out: &mut Vec<&'a str>) {
    for c in items {
        match c {
            Compose::Layer(name) => out.push(name.as_str()),
            Compose::Residual(inner) | Compose::Wrap(_, inner) => compose_layer_names(inner, out),
            Compose::Branch(paths) => {
                for path in paths {
                    compose_layer_names(path, out);
                }
            }
        }
    }
}

/// A block printed `{` … `}`, with `{}` for an empty one and the closing brace
/// at `depth`. The caller writes the line end.
fn emit_braced_block(buf: &mut String, block: &Block, mode: Mode, depth: usize) {
    if block.stmts.is_empty() && block.tail_expr.is_none() {
        buf.push_str("{}");
        return;
    }
    buf.push_str("{\n");
    emit_block_body(buf, block, mode, depth + 1);
    indent(buf, depth);
    buf.push('}');
}

/// Comma-separated expressions, as used for layer arguments and fact arguments.
fn emit_expr_list(buf: &mut String, exprs: &[Expr], mode: Mode) {
    for (i, e) in exprs.iter().enumerate() {
        if i > 0 {
            buf.push_str(", ");
        }
        emit_expr(buf, e, mode);
    }
}

fn emit_kb(buf: &mut String, k: &KbDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    // `kb` in both modes: the parser has no rule for the `κ` sigil.
    buf.push_str("kb ");
    buf.push_str(&k.name);
    buf.push_str(" {\n");
    for f in &k.facts {
        // `fact name(args);`. The parser requires the argument list, even if empty.
        indent(buf, depth + 1);
        buf.push_str("fact ");
        buf.push_str(&f.name);
        buf.push('(');
        emit_expr_list(buf, &f.args, mode);
        buf.push_str(");\n");
    }
    for r in &k.rules {
        // `rule name(params) [where conds] { body }`. The parser has no rule for
        // the `ρ` sigil.
        indent(buf, depth + 1);
        buf.push_str("rule ");
        buf.push_str(&r.name);
        buf.push('(');
        for (i, p) in r.params.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            emit_param(buf, p, mode);
        }
        buf.push(')');
        if !r.conditions.is_empty() {
            match mode {
                Mode::Agent => buf.push_str(" ~> "),
                Mode::Human => buf.push_str(" where "),
            }
            emit_expr_list(buf, &r.conditions, mode);
        }
        buf.push(' ');
        emit_braced_block(buf, &r.body, mode, depth + 1);
        buf.push('\n');
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

fn emit_evolve(buf: &mut String, e: &EvolveDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    // `evolve` in both modes: the parser has no rule for the `Ω` sigil.
    buf.push_str("evolve ");
    buf.push_str(&e.name);
    buf.push_str(" {\n");
    // The parser's default for an absent `genome` is `Type::Inferred`; that
    // default is not written, so it is not printed.
    if !matches!(e.genome_type, Type::Inferred) {
        indent(buf, depth + 1);
        buf.push_str("genome: ");
        emit_type(buf, &e.genome_type, mode);
        buf.push_str(";\n");
    }
    if let Some(ref p) = e.population_size {
        indent(buf, depth + 1);
        buf.push_str("population: ");
        emit_expr(buf, p, mode);
        buf.push_str(";\n");
    }
    if let Some(ref g) = e.generations {
        indent(buf, depth + 1);
        buf.push_str("generations: ");
        emit_expr(buf, g, mode);
        buf.push_str(";\n");
    }
    indent(buf, depth + 1);
    buf.push_str("fitness ");
    emit_braced_block(buf, &e.fitness, mode, depth + 1);
    buf.push('\n');
    for (keyword, block) in [
        ("mutate", &e.mutate_fn),
        ("crossover", &e.crossover_fn),
        ("select", &e.select_fn),
    ] {
        if let Some(b) = block {
            indent(buf, depth + 1);
            buf.push_str(keyword);
            buf.push(' ');
            emit_braced_block(buf, b, mode, depth + 1);
            buf.push('\n');
        }
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

fn emit_train(buf: &mut String, t: &TrainDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    // `train` in both modes: the parser has no rule for the `Θ` sigil.
    buf.push_str("train ");
    buf.push_str(&t.name);
    buf.push_str(" {\n");
    if !t.net.is_empty() {
        indent(buf, depth + 1);
        buf.push_str("net: ");
        buf.push_str(&t.net);
        buf.push_str(";\n");
    }
    // Every expression field is a `label: expr;` line, in the parser's labels.
    let fields: [(&str, &Option<Expr>); 23] = [
        ("optimizer", &t.optimizer),
        ("loss", &t.loss),
        ("epochs", &t.epochs),
        ("inputs", &t.inputs),
        ("targets", &t.targets),
        ("dataset", &t.dataset),
        ("val_split", &t.val_split),
        ("checkpoint", &t.checkpoint),
        ("batch_size", &t.batch_size),
        ("patience", &t.patience),
        ("prompt", &t.prompt),
        ("max_tokens", &t.max_tokens),
        ("temperature", &t.temperature),
        ("top_k", &t.top_k),
        ("top_p", &t.top_p),
        ("seed", &t.seed),
        ("clip_grad", &t.clip_grad),
        ("warmup_steps", &t.warmup_steps),
        ("lr_schedule", &t.lr_schedule),
        ("weight_decay", &t.weight_decay),
        ("tied_embeddings", &t.tied_embeddings),
        ("plateau_patience", &t.plateau_patience),
        ("lr_factor", &t.lr_factor),
    ];
    for (label, value) in fields {
        if let Some(e) = value {
            indent(buf, depth + 1);
            buf.push_str(label);
            buf.push_str(": ");
            emit_expr(buf, e, mode);
            buf.push_str(";\n");
        }
    }
    indent(buf, depth + 1);
    buf.push_str("body ");
    emit_braced_block(buf, &t.body, mode, depth + 1);
    buf.push('\n');
    indent(buf, depth);
    buf.push_str("}\n");
}

fn emit_swarm(buf: &mut String, s: &SwarmDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    // `swarm` in both modes: the parser has no rule for the `Σ` sigil.
    buf.push_str("swarm ");
    buf.push_str(&s.name);
    buf.push_str(" {\n");
    // agent type: `agent:` in both modes, since the parser has no `α` sigil.
    if !s.agent_type.is_empty() {
        indent(buf, depth + 1);
        buf.push_str("agent: ");
        buf.push_str(&s.agent_type);
        buf.push_str(";\n");
    }
    // size
    if let Some(ref size) = s.size {
        indent(buf, depth + 1);
        buf.push_str("size: ");
        emit_expr(buf, size, mode);
        buf.push_str(";\n");
    }
    // topology
    if let Some(ref topo) = s.topology {
        indent(buf, depth + 1);
        match mode {
            Mode::Agent => buf.push_str("topo: "),
            Mode::Human => buf.push_str("topology: "),
        }
        buf.push_str(topo);
        buf.push_str(";\n");
    }
    // consensus
    if let Some(ref cons) = s.consensus {
        indent(buf, depth + 1);
        match mode {
            Mode::Agent => buf.push_str("cons: "),
            Mode::Human => buf.push_str("consensus: "),
        }
        buf.push_str(cons);
        buf.push_str(";\n");
    }
    // transport
    if let Some(ref transport) = s.transport {
        indent(buf, depth + 1);
        buf.push_str("transport: ");
        buf.push_str(transport);
        buf.push_str(";\n");
    }
    // `on_dispatch`, `on_aggregate` and `on_failure` have no surface syntax:
    // the parser rejects `dispatch`/`aggregate`/`on_failure` blocks (see
    // `parse_swarm_def`), so they are only set by the bridge and are not printed.
    indent(buf, depth);
    buf.push_str("}\n");
}

// ── Data ─────────────────────────────────────────────────────────────

fn emit_data(buf: &mut String, d: &DataDef, vis: Visibility, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => {
            if vis == Visibility::Public {
                buf.push_str("+D ");
            } else {
                buf.push_str("D ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            buf.push_str("data ");
        }
    }
    buf.push_str(&d.name);
    emit_generics(buf, &d.generics);

    match &d.kind {
        DataKind::Record(fields) => {
            if fields.is_empty() {
                buf.push('\n');
            } else {
                buf.push('(');
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    buf.push_str(&f.name);
                    buf.push_str(": ");
                    emit_type(buf, &f.ty, mode);
                    if let Some(ref default) = f.default {
                        buf.push_str(" = ");
                        emit_expr(buf, default, mode);
                    }
                }
                buf.push_str(")\n");
            }
        }
        DataKind::Sum(variants) => {
            buf.push_str(" = ");
            for (i, v) in variants.iter().enumerate() {
                if i > 0 {
                    buf.push_str(" | ");
                }
                buf.push_str(&v.name);
                if !v.fields.is_empty() {
                    buf.push('(');
                    for (j, ty) in v.fields.iter().enumerate() {
                        if j > 0 {
                            buf.push_str(", ");
                        }
                        emit_type(buf, ty, mode);
                    }
                    buf.push(')');
                }
            }
            buf.push('\n');
        }
    }
}

// ── Extend ───────────────────────────────────────────────────────────

fn emit_extend(buf: &mut String, e: &ExtendBlock, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("xd "),
        Mode::Human => buf.push_str("extend "),
    }
    emit_type(buf, &e.target_type, mode);
    if e.items.is_empty() {
        buf.push_str(" {}\n");
    } else {
        buf.push_str(" {\n");
        for item in &e.items {
            emit_item(buf, item, mode, depth + 1);
        }
        indent(buf, depth);
        buf.push_str("}\n");
    }
}

// ── Contracts ────────────────────────────────────────────────────────

fn emit_contract(buf: &mut String, c: &ContractClause) {
    match c.kind {
        ContractClauseKind::Requires => {
            buf.push_str("@req(");
            buf.push_str(&c.condition);
            buf.push(')');
        }
        ContractClauseKind::Ensures => {
            buf.push_str("@ens(");
            buf.push_str(&c.condition);
            buf.push(')');
        }
        ContractClauseKind::Invariant => {
            buf.push_str("@inv(");
            buf.push_str(&c.condition);
            buf.push(')');
        }
    }
    if let Some(ref msg) = c.message {
        buf.push_str(" \"");
        buf.push_str(msg);
        buf.push('"');
    }
}

// ── Generics ─────────────────────────────────────────────────────────

fn emit_generics(buf: &mut String, generics: &[GenericParam]) {
    if generics.is_empty() {
        return;
    }
    buf.push('<');
    for (i, g) in generics.iter().enumerate() {
        if i > 0 {
            buf.push_str(", ");
        }
        buf.push_str(&g.name);
        if !g.bounds.is_empty() {
            buf.push_str(": ");
            buf.push_str(&g.bounds.join(" + "));
        }
    }
    buf.push('>');
}

fn emit_where_clause(buf: &mut String, wc: &[WherePredicate], mode: Mode) {
    if wc.is_empty() {
        return;
    }
    match mode {
        Mode::Agent => buf.push_str(" ~> "),
        Mode::Human => buf.push_str(" where "),
    }
    for (i, pred) in wc.iter().enumerate() {
        if i > 0 {
            buf.push_str(", ");
        }
        buf.push_str(&pred.type_param);
        buf.push_str(": ");
        buf.push_str(&pred.bounds.join(" + "));
    }
}

// ── Types ────────────────────────────────────────────────────────────

/// Render a type the way the formatter writes it.
///
/// `nl_engine`'s explanations used `{:?}` on the AST node, so asking
/// `nl/explain` about `+f add(a: i32, b: i32)` answered
/// "(a: Path { segments: [\"i32\"], type_args: [] }, …)" — Rust `Debug` output
/// in a natural-language answer, from the method an agent reaches for to
/// *understand* code.
pub fn type_to_string(ty: &Type, mode: Mode) -> String {
    let mut buf = String::new();
    emit_type(&mut buf, ty, mode);
    buf
}

fn emit_type(buf: &mut String, ty: &Type, mode: Mode) {
    match ty {
        Type::Path {
            segments,
            type_args,
        } => {
            buf.push_str(&segments.join("."));
            if !type_args.is_empty() {
                // `Path[A, B]` — the parser's type-argument form. `<…>` is
                // also accepted, but `Path<Inner<T>>` ends in `>>`, which the
                // lexer reads as one shift token.
                buf.push('[');
                for (i, a) in type_args.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    emit_type(buf, a, mode);
                }
                buf.push(']');
            }
        }
        Type::Reference { mutable, inner } => {
            // `&!T` is the only mutable-reference spelling the parser reads
            // (`&m T` and `&mut T` do not parse), in both modes.
            buf.push_str(if *mutable { "&!" } else { "&" });
            emit_type(buf, inner, mode);
        }
        Type::OwnedPtr { inner } => {
            buf.push('^');
            emit_type(buf, inner, mode);
        }
        Type::Rc { inner } => {
            buf.push('$');
            emit_type(buf, inner, mode);
        }
        Type::Arc { inner } => {
            buf.push('@');
            emit_type(buf, inner, mode);
        }
        Type::Cow { inner } => {
            buf.push_str("&~");
            emit_type(buf, inner, mode);
        }
        Type::Cell { inner } => {
            buf.push('%');
            emit_type(buf, inner, mode);
        }
        Type::RefCell { inner } => {
            buf.push_str("%!");
            emit_type(buf, inner, mode);
        }
        Type::Mutex { inner } => {
            buf.push('#');
            emit_type(buf, inner, mode);
        }
        Type::RwLock { inner } => {
            buf.push_str("#~");
            emit_type(buf, inner, mode);
        }
        Type::Slice { inner } => {
            buf.push('[');
            emit_type(buf, inner, mode);
            buf.push(']');
        }
        Type::Array { inner, size } => {
            buf.push('[');
            emit_type(buf, inner, mode);
            buf.push_str("; ");
            emit_expr(buf, size, mode);
            buf.push(']');
        }
        Type::Vec { inner } => {
            buf.push('[');
            emit_type(buf, inner, mode);
            buf.push_str("]~");
        }
        Type::Set { inner } => {
            buf.push('{');
            emit_type(buf, inner, mode);
            buf.push('}');
        }
        Type::Tuple { elements } => {
            buf.push('(');
            for (i, t) in elements.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_type(buf, t, mode);
            }
            buf.push(')');
        }
        Type::Option { inner } => {
            buf.push('?');
            emit_type(buf, inner, mode);
        }
        // `R[T, E]` in both modes. The human `T or E` spelling is read only as
        // a function's return type (see `emit_function`), so it cannot stand
        // for a result anywhere else.
        Type::Result { ok, err } => {
            buf.push_str("R[");
            emit_type(buf, ok, mode);
            buf.push_str(", ");
            emit_type(buf, err, mode);
            buf.push(']');
        }
        Type::Map { key, value } => {
            buf.push('{');
            emit_type(buf, key, mode);
            buf.push_str(": ");
            emit_type(buf, value, mode);
            buf.push('}');
        }
        Type::Ptr { inner } => {
            buf.push_str("Ptr[");
            emit_type(buf, inner, mode);
            buf.push(']');
        }
        Type::Simd { inner, width } => {
            buf.push_str("Simd[");
            emit_type(buf, inner, mode);
            buf.push_str(", ");
            buf.push_str(&width.to_string());
            buf.push(']');
        }
        Type::Fn { params, ret } => {
            match mode {
                Mode::Agent => buf.push_str("f("),
                Mode::Human => buf.push_str("fn("),
            }
            for (i, t) in params.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_type(buf, t, mode);
            }
            buf.push(')');
            if let Some(r) = ret {
                buf.push_str(" -> ");
                emit_type(buf, r, mode);
            }
        }
        Type::Never => buf.push('!'),
        Type::Inferred => buf.push('_'),
        // Each spelling below is the one the parser maps back to the same
        // variant. `Self` and `String` are plain identifiers to the parser,
        // so they would come back as paths.
        Type::SelfType => buf.push_str("_T"),
        // `s` is the only spelling the parser reads as the string type, in
        // both modes (`String` parses as a path).
        Type::StringType => buf.push('s'),
        Type::KnowledgeBase => buf.push_str("kb"),
        Type::LlmType => buf.push_str("LLM"),
        Type::Tensor { inner, shape } => {
            buf.push_str("tensor[");
            emit_type(buf, inner, mode);
            for d in shape {
                buf.push_str(", ");
                match d {
                    crate::ast::TensorDim::Lit(n) => buf.push_str(&n.to_string()),
                    crate::ast::TensorDim::Var(v) => buf.push_str(v),
                }
            }
            buf.push(']');
        }
        Type::ParamTy { inner, shape } => {
            buf.push_str("param[");
            emit_type(buf, inner, mode);
            for d in shape {
                buf.push_str(", ");
                match d {
                    crate::ast::TensorDim::Lit(n) => buf.push_str(&n.to_string()),
                    crate::ast::TensorDim::Var(v) => buf.push_str(v),
                }
            }
            buf.push(']');
        }
        // `tensor`, `param`, `genome` and `policy` are keywords to the lexer,
        // so only the lowercase spellings reach the parser's AI-type arms.
        Type::Genome { inner } => {
            buf.push_str("genome[");
            emit_type(buf, inner, mode);
            buf.push(']');
        }
        Type::Policy { state, action } => {
            buf.push_str("policy[");
            emit_type(buf, state, mode);
            buf.push_str(", ");
            emit_type(buf, action, mode);
            buf.push(']');
        }
        Type::Refined { base, predicate } => {
            emit_type(buf, base, mode);
            buf.push_str(" ~> ");
            buf.push_str(predicate);
        }
        // `f64[m/s]` in both modes: the unit is stored canonical.
        Type::Dimensioned { base, unit } => {
            emit_type(buf, base, mode);
            buf.push('[');
            buf.push_str(unit);
            buf.push(']');
        }
    }
}

// ── Operator precedence ──────────────────────────────────────────────
//
// `parser::parse_expr_bp` is a Pratt loop: an infix operator with left binding
// power `l` takes its right operand at `l + 1`, and a prefix operator takes
// its operand at a power above every infix one, so `-a + b` is `(-a) + b`.
// A child is parenthesised exactly when the loop would not take it at the
// power its parent gives it. Parentheses leave no node in the AST, so the
// parenthesised text reparses to the same tree.

/// Left binding power of an infix operator spelling, as the parser has it.
fn infix_bp(op: &str) -> u8 {
    match op {
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" => 1,
        "||" | "|>" | "▸" => 3,
        "&&" => 5,
        "==" | "!=" => 7,
        "<" | ">" | "<=" | ">=" => 9,
        "|" => 11,
        "^" => 13,
        "&" => 15,
        "<<" | ">>" => 17,
        "+" | "-" => 19,
        "*" | "/" | "%" => 21,
        "⊗" | "⊙" => 23,
        _ => 0,
    }
}

/// The binding power an expression carries as the operand of an infix
/// operator. `u8::MAX` means it binds tighter than every infix operator.
/// A closure, `ret` or `break` takes everything after it, so it is 0.
fn operand_bp(expr: &Expr) -> u8 {
    match expr {
        Expr::Binary { op, .. } => infix_bp(op),
        Expr::Assign { .. } => 1,
        Expr::Pipeline { .. } => 3,
        Expr::Closure { .. } | Expr::Return { .. } | Expr::Break { .. } => 0,
        _ => u8::MAX,
    }
}

/// True when the expression prints as one primary, so that a postfix operator
/// (`.f`, `(…)`, `[…]`, `?`, `.await`) may follow it unparenthesised. Prefix
/// and postfix operators, casts, ranges and control flow are excluded: a
/// postfix operator after a block-like `}` does not attach in the parser.
fn is_primary(expr: &Expr) -> bool {
    operand_bp(expr) == u8::MAX
        && !matches!(
            expr,
            Expr::Unary { .. }
                | Expr::Cast { .. }
                | Expr::Is { .. }
                | Expr::Range { .. }
                | Expr::Continue
                | Expr::If { .. }
                | Expr::Match { .. }
                | Expr::Loop { .. }
                | Expr::While { .. }
                | Expr::For { .. }
                | Expr::Block { .. }
        )
}

/// Emit `expr`, wrapped in parentheses when `parens` is set.
fn emit_child(buf: &mut String, expr: &Expr, mode: Mode, parens: bool) {
    if parens {
        buf.push('(');
        emit_expr(buf, expr, mode);
        buf.push(')');
    } else {
        emit_expr(buf, expr, mode);
    }
}

// ── Expressions ──────────────────────────────────────────────────────

fn emit_expr(buf: &mut String, expr: &Expr, mode: Mode) {
    match expr {
        Expr::Literal { value, .. } => buf.push_str(value),
        Expr::Ident { name } => buf.push_str(name),
        Expr::Binary { op, left, right } => {
            // Left-associative: the left operand may share the operator's
            // power, the right operand must bind tighter than it.
            let bp = infix_bp(op);
            emit_child(buf, left, mode, operand_bp(left) < bp);
            buf.push(' ');
            buf.push_str(op);
            buf.push(' ');
            emit_child(buf, right, mode, operand_bp(right) <= bp);
        }
        Expr::Unary { op, operand } if op == "⊤" || op == "⊥" => {
            // Tensor transpose and flatten are postfix in the parser.
            emit_child(buf, operand, mode, !is_primary(operand));
            buf.push_str(op);
        }
        Expr::Unary { op, operand } => {
            buf.push_str(op);
            // A prefix operand is parsed with its postfix chain but no infix
            // operator. Another prefix operator also needs parentheses: `&&x`
            // lexes as logical and, not as two `&`.
            let prefix_operand = matches!(
                &**operand,
                Expr::Unary { op, .. } if op != "⊤" && op != "⊥"
            );
            emit_child(buf, operand, mode, operand_bp(operand) != u8::MAX || prefix_operand);
        }
        Expr::Call { func, args } => {
            emit_child(buf, func, mode, !is_primary(func));
            buf.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr(buf, a, mode);
            }
            buf.push(')');
        }
        Expr::MethodCall {
            receiver,
            method,
            type_args,
            args,
        } => {
            emit_child(buf, receiver, mode, !is_primary(receiver));
            buf.push('.');
            buf.push_str(method);
            if !type_args.is_empty() {
                // Turbofish `.m::<T>(…)`, read by the parser's postfix loop.
                buf.push_str("::<");
                for (i, t) in type_args.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    emit_type(buf, t, mode);
                }
                buf.push('>');
            }
            buf.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr(buf, a, mode);
            }
            buf.push(')');
        }
        Expr::FieldAccess { object, field } => {
            emit_child(buf, object, mode, !is_primary(object));
            buf.push('.');
            buf.push_str(field);
        }
        Expr::Index { object, index } => {
            emit_child(buf, object, mode, !is_primary(object));
            buf.push('[');
            emit_expr(buf, index, mode);
            buf.push(']');
        }
        Expr::StructLit { path, fields } => {
            // Both modes spell a struct literal `@Path { … }`. Without the `@`
            // the parser reads `Path` as a name and `{ … }` as a block after
            // it, so a function tail `Dropout { rate: r }` split in two.
            buf.push('@');
            buf.push_str(&path.join("."));
            buf.push_str(" { ");
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                buf.push_str(&f.name);
                if let Some(ref v) = f.value {
                    buf.push_str(": ");
                    emit_expr(buf, v, mode);
                }
            }
            buf.push_str(" }");
        }
        Expr::TupleLit { elements } => {
            buf.push('(');
            for (i, e) in elements.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr(buf, e, mode);
            }
            // `(a)` is a parenthesised expression; a one-element tuple needs
            // its trailing comma.
            if elements.len() == 1 {
                buf.push(',');
            }
            buf.push(')');
        }
        Expr::ArrayLit { elements } => {
            buf.push('[');
            for (i, e) in elements.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr(buf, e, mode);
            }
            buf.push(']');
        }
        Expr::MapLit { entries } => {
            buf.push('{');
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr(buf, k, mode);
                buf.push_str(": ");
                emit_expr(buf, v, mode);
            }
            buf.push('}');
        }
        Expr::ArrayRepeat { value, count } => {
            buf.push('[');
            emit_expr(buf, value, mode);
            buf.push_str("; ");
            emit_expr(buf, count, mode);
            buf.push(']');
        }
        Expr::Closure { params, body } => {
            buf.push('|');
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_param(buf, p, mode);
            }
            buf.push_str("| ");
            emit_expr(buf, body, mode);
        }
        Expr::If {
            cond,
            then_block,
            else_block,
        } => {
            match mode {
                Mode::Agent => buf.push_str("? "),
                Mode::Human => buf.push_str("if "),
            }
            emit_expr(buf, cond, mode);
            buf.push_str(" {\n");
            emit_block_body(buf, then_block, mode, 1);
            buf.push('}');
            if let Some(eb) = else_block {
                match mode {
                    Mode::Agent => buf.push_str(" : {\n"),
                    Mode::Human => buf.push_str(" else {\n"),
                }
                emit_block_body(buf, eb, mode, 1);
                buf.push('}');
            }
        }
        Expr::Handle { body, effect, arms } => {
            match mode {
                Mode::Agent => buf.push_str("hx "),
                Mode::Human => buf.push_str("handle "),
            }
            buf.push_str("{\n");
            emit_block_body(buf, body, mode, 1);
            buf.push_str("} with ");
            buf.push_str(effect);
            buf.push_str(" {\n");
            for arm in arms {
                buf.push_str("    ");
                buf.push_str(&arm.op);
                buf.push('(');
                buf.push_str(&arm.params.join(", "));
                buf.push_str(") => ");
                emit_expr(buf, &arm.body, mode);
                buf.push_str(",\n");
            }
            buf.push('}');
        }
        Expr::Match { scrutinee, arms } => {
            // A match without a scrutinee has no `match` spelling: `?= {`
            // and `match {` do not parse. `? {` and `if {` do.
            match (mode, scrutinee) {
                (Mode::Agent, Some(_)) => buf.push_str("?= "),
                (Mode::Agent, None) => buf.push_str("? "),
                (Mode::Human, Some(_)) => buf.push_str("match "),
                (Mode::Human, None) => buf.push_str("if "),
            }
            if let Some(s) = scrutinee {
                emit_expr(buf, s, mode);
                buf.push(' ');
            }
            buf.push_str("{\n");
            for arm in arms {
                buf.push_str("    ");
                emit_pattern(buf, &arm.pattern);
                buf.push_str(" => ");
                emit_expr(buf, &arm.body, mode);
                buf.push_str(",\n");
            }
            buf.push('}');
        }
        Expr::Loop { body } => {
            match mode {
                Mode::Agent => buf.push_str("@@ "),
                Mode::Human => buf.push_str("loop "),
            }
            buf.push_str("{\n");
            emit_block_body(buf, body, mode, 1);
            buf.push('}');
        }
        Expr::While { cond, body } => {
            match mode {
                Mode::Agent => buf.push_str("@w "),
                Mode::Human => buf.push_str("while "),
            }
            emit_expr(buf, cond, mode);
            buf.push_str(" {\n");
            emit_block_body(buf, body, mode, 1);
            buf.push('}');
        }
        Expr::For {
            pattern,
            iter,
            body,
        } => {
            match mode {
                Mode::Agent => buf.push_str("@ "),
                Mode::Human => buf.push_str("for "),
            }
            emit_pattern(buf, pattern);
            match mode {
                Mode::Agent => buf.push_str(" : "),
                Mode::Human => buf.push_str(" in "),
            }
            emit_expr(buf, iter, mode);
            buf.push_str(" {\n");
            emit_block_body(buf, body, mode, 1);
            buf.push('}');
        }
        Expr::Block { block } => {
            buf.push_str("{\n");
            emit_block_body(buf, block, mode, 1);
            buf.push('}');
        }
        Expr::Return { value } => {
            match mode {
                Mode::Agent => buf.push_str("ret"),
                Mode::Human => buf.push_str("return"),
            }
            if let Some(v) = value {
                buf.push(' ');
                emit_expr(buf, v, mode);
            }
        }
        Expr::Break { value } => {
            // Bare `!` is `break` in agent mode, but `! x` lexes as logical
            // not, so a break with a value is spelled `break` in both modes.
            match (mode, value) {
                (Mode::Agent, None) => buf.push('!'),
                _ => buf.push_str("break"),
            }
            if let Some(v) = value {
                buf.push(' ');
                emit_expr(buf, v, mode);
            }
        }
        Expr::Continue => match mode {
            Mode::Agent => buf.push_str(">>"),
            Mode::Human => buf.push_str("continue"),
        },
        Expr::Try { expr } => {
            emit_child(buf, expr, mode, !is_primary(expr));
            buf.push('?');
        }
        Expr::Await { expr } => {
            emit_child(buf, expr, mode, !is_primary(expr));
            // `.w` is not read as await in agent mode: the parser takes it for
            // a field access. `.await` is accepted in both modes.
            buf.push_str(".await");
        }
        Expr::Cast { expr, ty } => {
            emit_child(buf, expr, mode, !is_primary(expr));
            buf.push_str(" as ");
            emit_type(buf, ty, mode);
        }
        Expr::Assign { target, value } => {
            // The parser reads `a = b = c` as `(a = b) = c`, so an assignment
            // used as the value is bracketed.
            emit_child(buf, target, mode, operand_bp(target) < 1);
            buf.push_str(" = ");
            emit_child(buf, value, mode, operand_bp(value) <= 1);
        }
        Expr::Range {
            start,
            end,
            inclusive,
        } => {
            emit_expr(buf, start, mode);
            if *inclusive {
                buf.push_str("..=");
            } else {
                buf.push_str("..");
            }
            emit_expr(buf, end, mode);
        }
        // `??` and `???` are the parser's todo and unimplemented tokens. A
        // Rust-style `todo!()` is a macro call, which MAGE does not have.
        Expr::Todo => buf.push_str("??"),
        Expr::Unimplemented => buf.push_str("???"),
        Expr::Error { message } => {
            buf.push_str("/* error: ");
            buf.push_str(message);
            buf.push_str(" */");
        }
        Expr::Pipeline { left, right } => {
            emit_child(buf, left, mode, operand_bp(left) < 3);
            buf.push_str(" |> ");
            emit_child(buf, right, mode, operand_bp(right) <= 3);
        }
        // `∇(e, w)` in agent mode, `grad(e, w)` in human mode — the
        // Gradient row of the sigil table (§3), which published both
        // spellings for a construct that had neither.
        Expr::Grad { value, wrt } => {
            buf.push_str(if mode == Mode::Agent { "\u{2207}(" } else { "grad(" });
            emit_expr(buf, value, mode);
            buf.push_str(", ");
            emit_expr(buf, wrt, mode);
            buf.push(')');
        }
        Expr::Is { expr, pattern } => {
            emit_child(buf, expr, mode, !is_primary(expr));
            buf.push_str(" is ");
            emit_pattern(buf, pattern);
        }
    }
}

// ── Patterns ─────────────────────────────────────────────────────────

fn emit_pattern(buf: &mut String, pat: &Pattern) {
    match pat {
        Pattern::Ident { name } => buf.push_str(name),
        Pattern::Literal { value } => buf.push_str(value),
        Pattern::Wildcard => buf.push('_'),
        Pattern::Tuple { elements } => {
            buf.push('(');
            for (i, p) in elements.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_pattern(buf, p);
            }
            buf.push(')');
        }
        Pattern::Struct { path, fields } => {
            // `@Path { … }` in both modes, as for struct literals.
            buf.push('@');
            buf.push_str(&path.join("."));
            buf.push_str(" { ");
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                buf.push_str(&f.name);
                if let Some(ref p) = f.pattern {
                    buf.push_str(": ");
                    emit_pattern(buf, p);
                }
            }
            buf.push_str(" }");
        }
        Pattern::Enum { path, elements } => {
            buf.push_str(&path.join("."));
            if !elements.is_empty() {
                buf.push('(');
                for (i, p) in elements.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    emit_pattern(buf, p);
                }
                buf.push(')');
            }
        }
        Pattern::Slice { elements, rest, rest_name } => {
            buf.push('[');
            for (i, p) in elements.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_pattern(buf, p);
            }
            if *rest {
                // `[..]` alone: a comma before the rest would read as a
                // pattern element.
                buf.push_str(if elements.is_empty() { ".." } else { ", .." });
                if let Some(name) = rest_name {
                    buf.push_str(name);
                }
            }
            buf.push(']');
        }
        Pattern::Or { patterns } => {
            for (i, p) in patterns.iter().enumerate() {
                if i > 0 {
                    buf.push_str(" | ");
                }
                emit_pattern(buf, p);
            }
        }
        Pattern::Ref { pattern } => {
            buf.push('&');
            emit_pattern(buf, pattern);
        }
    }
}

// ── Statements / Block ───────────────────────────────────────────────

fn emit_block_body(buf: &mut String, block: &Block, mode: Mode, depth: usize) {
    for stmt in &block.stmts {
        emit_stmt(buf, stmt, mode, depth);
    }
    if let Some(ref tail) = block.tail_expr {
        indent(buf, depth);
        emit_expr(buf, tail, mode);
        buf.push('\n');
    }
}

fn emit_stmt(buf: &mut String, stmt: &Stmt, mode: Mode, depth: usize) {
    match stmt {
        Stmt::Let {
            mutable,
            pattern,
            ty,
            value,
        } => {
            indent(buf, depth);
            match mode {
                // `v (a, b) = …` parses as a call of `v`: only the long
                // keywords start a destructuring binder. Name-only patterns
                // keep the short form.
                Mode::Agent if !matches!(pattern, Pattern::Ident { .. } | Pattern::Wildcard) => {
                    buf.push_str(if *mutable { "var " } else { "val " });
                }
                Mode::Agent => {
                    if *mutable {
                        buf.push_str("m ");
                    } else {
                        buf.push_str("v ");
                    }
                }
                Mode::Human => {
                    if *mutable {
                        buf.push_str("var ");
                    } else {
                        buf.push_str("val ");
                    }
                }
            }
            emit_pattern(buf, pattern);
            if let Some(t) = ty {
                buf.push_str(": ");
                emit_type(buf, t, mode);
            }
            buf.push_str(" = ");
            emit_expr(buf, value, mode);
            buf.push_str(";\n");
        }
        Stmt::Expr { expr } => {
            indent(buf, depth);
            emit_expr(buf, expr, mode);
            buf.push_str(";\n");
        }
        Stmt::Item { item } => {
            emit_item(buf, item, mode, depth);
        }
        Stmt::Guard { cond, else_block } => {
            indent(buf, depth);
            match mode {
                Mode::Agent => buf.push_str("gd "),
                Mode::Human => buf.push_str("guard "),
            }
            emit_expr(buf, cond, mode);
            match mode {
                Mode::Agent => buf.push_str(" : "),
                Mode::Human => buf.push_str(" else "),
            }
            buf.push_str("{\n");
            emit_block_body(buf, else_block, mode, depth + 1);
            indent(buf, depth);
            buf.push_str("}\n");
        }
        Stmt::Defer { expr } => {
            indent(buf, depth);
            match mode {
                Mode::Agent => buf.push_str("df "),
                Mode::Human => buf.push_str("defer "),
            }
            emit_expr(buf, expr, mode);
            buf.push_str(";\n");
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn parse_source(src: &str) -> Module {
        let tokens = lexer::lex(src);
        parser::parse(&tokens).expect("parse failed")
    }

    #[test]
    fn compact_function() {
        let m = parse_source("f greet(name: String) {}");
        let out = format_agent(&m);
        assert!(out.contains("f greet("), "got: {out}");
        assert!(!out.contains("fn "));
    }

    #[test]
    fn expand_function() {
        let m = parse_source("f greet(name: String) {}");
        let out = format_human(&m);
        assert!(out.contains("fn greet("), "got: {out}");
    }

    #[test]
    fn compact_pub_function() {
        let m = parse_source("+f main() {}");
        let out = format_agent(&m);
        assert!(out.contains("+f main("), "got: {out}");
    }

    #[test]
    fn expand_pub_function() {
        let m = parse_source("+f main() {}");
        let out = format_human(&m);
        assert!(out.contains("pub fn main("), "got: {out}");
    }

    #[test]
    fn compact_struct() {
        let m = parse_source("S Point { x: i32, y: i32 }");
        let out = format_agent(&m);
        assert!(out.starts_with("S Point"), "got: {out}");
        assert!(!out.contains("struct"));
    }

    #[test]
    fn expand_struct() {
        let m = parse_source("S Point { x: i32, y: i32 }");
        let out = format_human(&m);
        assert!(out.starts_with("struct Point"), "got: {out}");
    }

    #[test]
    fn compact_enum() {
        let m = parse_source("E Color { Red, Green, Blue }");
        let out = format_agent(&m);
        assert!(out.starts_with("E Color"), "got: {out}");
    }

    #[test]
    fn expand_enum() {
        let m = parse_source("E Color { Red, Green, Blue }");
        let out = format_human(&m);
        assert!(out.starts_with("enum Color"), "got: {out}");
    }

    #[test]
    fn compact_let_binding() {
        let m = parse_source("f main() { v x = 42; }");
        let out = format_agent(&m);
        assert!(out.contains("v x = 42;"), "got: {out}");
    }

    #[test]
    fn expand_let_binding() {
        let m = parse_source("f main() { v x = 42; }");
        let out = format_human(&m);
        assert!(out.contains("val x = 42;"), "got: {out}");
    }

    #[test]
    fn compact_let_mut() {
        let m = parse_source("f main() { m x = 0; }");
        let out = format_agent(&m);
        assert!(out.contains("m x = 0;"), "got: {out}");
    }

    #[test]
    fn expand_let_mut() {
        let m = parse_source("f main() { m x = 0; }");
        let out = format_human(&m);
        assert!(out.contains("var x = 0;"), "got: {out}");
    }

    #[test]
    fn compact_use() {
        let m = parse_source("u std.io;");
        let out = format_agent(&m);
        // MAGE path separator is `.` (no `::` token) — output must round-trip.
        assert!(out.contains("u std.io;"), "got: {out}");
    }

    #[test]
    fn expand_use() {
        let m = parse_source("u std.io;");
        let out = format_human(&m);
        assert!(out.contains("use std.io;"), "got: {out}");
    }

    #[test]
    fn compact_contracts() {
        let m = parse_source("@req(n > 0)\nf positive(n: i32) {}");
        let out = format_agent(&m);
        assert!(out.contains("@req(n > 0)"), "got: {out}");
        assert!(out.contains("f positive("), "got: {out}");
    }

    #[test]
    fn expand_contracts() {
        let m = parse_source("@req(n > 0)\nf positive(n: i32) {}");
        let out = format_human(&m);
        assert!(out.contains("@req(n > 0)"), "got: {out}");
        assert!(out.contains("fn positive("), "got: {out}");
    }

    #[test]
    fn compact_type_alias() {
        let m = parse_source("Y Id = u64;");
        let out = format_agent(&m);
        assert!(out.contains("Y Id = u64;"), "got: {out}");
    }

    #[test]
    fn expand_type_alias() {
        let m = parse_source("Y Id = u64;");
        let out = format_human(&m);
        assert!(out.contains("type Id = u64;"), "got: {out}");
    }

    #[test]
    fn compact_agent() {
        let m = parse_source("agent Bot { capabilities: [read_source] requires_approval: [] }");
        let out = format_agent(&m);
        // `α` is not a sigil the parser reads, so agent mode keeps the keyword.
        assert!(out.contains("agent Bot"), "got: {out}");
    }

    /// Parse `src`, print it in both modes, reparse each print, and check that
    /// the AST is unchanged and that printing the reparsed AST is a fixpoint.
    /// Replace every source span with `null`. A span is where a node sits in
    /// the text, so a reprint that moves it changes the JSON without changing
    /// the AST; the round trip compares the tree, not the positions.
    fn without_spans(v: &mut serde_json::Value) {
        use serde_json::Value;
        match v {
            Value::Object(map) => {
                let is_span = map.len() == 4
                    && ["col", "len", "line", "offset"].iter().all(|k| map.contains_key(*k));
                if is_span {
                    *v = Value::Null;
                } else {
                    map.values_mut().for_each(without_spans);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(without_spans),
            _ => {}
        }
    }

    fn assert_round_trips(src: &str) {
        let original = parse_source(src);
        let mut before = serde_json::to_value(&original).expect("the AST serialises");
        without_spans(&mut before);
        for (mode, printed) in [
            ("agent", format_agent(&original)),
            ("human", format_human(&original)),
        ] {
            let tokens = lexer::lex(&printed);
            let reparsed = parser::parse(&tokens).unwrap_or_else(|e| {
                panic!("{mode} print does not reparse: {e:?}\n{printed}")
            });
            let mut after = serde_json::to_value(&reparsed).expect("the AST serialises");
            without_spans(&mut after);
            assert_eq!(before, after, "{mode} print changed the AST:\n{printed}");
            let again = match mode {
                "agent" => format_agent(&reparsed),
                _ => format_human(&reparsed),
            };
            assert_eq!(printed, again, "{mode} print is not a fixpoint");
        }
    }

    #[test]
    fn round_trip_net_layers_and_forward() {
        // The sigils `Ψ`/`λ` are not read by the parser; a layer keeps its type and args.
        assert_round_trips(
            "net TinyLM {\n    layer tok: Embedding(8, 4);\n    layer head: Linear(4, 8, 1);\n    forward { tok }\n}\n",
        );
    }

    #[test]
    fn round_trip_net_composition() {
        // `residual` and `branch` keep their tree; the layers they name are printed in place.
        assert_round_trips(
            "net Block {\n    layer a: Linear(4, 4);\n    residual { layer b: Linear(4, 4); }\n    branch { layer c: Linear(4, 4); } { layer d: Linear(4, 4); }\n    forward { a }\n}\n",
        );
    }

    #[test]
    fn round_trip_kb_facts_and_rules() {
        // A fact keeps its arguments; a rule keeps its parameters, conditions and body.
        assert_round_trips(
            "kb Facts {\n    fact edge(1, 2);\n    rule path(x: i32) where x > 0 { x }\n}\n",
        );
    }

    #[test]
    fn round_trip_train_fields() {
        // Every `label: expr;` field and the `body` block survive the printer.
        assert_round_trips(
            "train Learn {\n    net: TinyLM;\n    epochs: 3;\n    seed: 7;\n    lr_factor: 0.5;\n    body { }\n}\n",
        );
    }

    #[test]
    fn round_trip_evolve_blocks() {
        // The genome, population, generations and the mutate/crossover/select blocks are all printed.
        assert_round_trips(
            "evolve Search {\n    genome: i32;\n    population: 10;\n    generations: 5;\n    fitness { 1 }\n    mutate { 2 }\n    select { 3 }\n}\n",
        );
    }

    #[test]
    fn round_trip_swarm_fields() {
        // `size` and `transport` are printed; neither was lost before.
        assert_round_trips(
            "swarm Crew {\n    agent: Bot;\n    size: 4;\n    topology: ring;\n    consensus: majority;\n    transport: local;\n}\n",
        );
    }

    #[test]
    fn round_trip_effect_operations() {
        // Each operation needs its `f` keyword and a `;` end, as the parser reads them.
        assert_round_trips("effect Audit {\n    f record(entry: i32) -> i32;\n}\n");
    }

    #[test]
    fn round_trip_async_and_unsafe_functions() {
        // `async` is the function keyword; `unsafe` is written `uf`, the one keyword the parser reads.
        assert_round_trips("af fetch(x: i32) -> i32 {\n    x\n}\nuf scrub(x: i32) {}\n");
    }

    #[test]
    fn round_trip_spec_generics() {
        // A spec's generic parameters are part of its AST and are printed.
        assert_round_trips("sp Pos[T] {\n    @req(x > 0)\n}\n");
    }

    #[test]
    fn roundtrip_compact_then_expand() {
        let src = "+f add(a: i32, b: i32) -> i32 { v result = a + b; }";
        let m = parse_source(src);
        let compact = format_agent(&m);
        assert!(compact.contains("+f add("), "compact: {compact}");
        let expand = format_human(&m);
        assert!(expand.contains("pub fn add("), "expand: {expand}");
    }

    // ── New syntax formatting tests ──────────────────────────────────

    #[test]
    fn expand_val_var() {
        let m = parse_source("f main() { v x = 1; m y = 2; }");
        let out = format_human(&m);
        assert!(out.contains("val x = 1;"), "val: {out}");
        assert!(out.contains("var y = 2;"), "var: {out}");
    }

    #[test]
    fn compact_val_var() {
        let m = parse_source("f main() { val x = 1; var y = 2; }");
        let out = format_agent(&m);
        assert!(out.contains("v x = 1;"), "v: {out}");
        assert!(out.contains("m y = 2;"), "m: {out}");
    }

    #[test]
    fn expand_guard() {
        let m = parse_source("f foo(x: i32) { gd x > 0 else { return 0; } }");
        let out = format_human(&m);
        assert!(out.contains("guard"), "should contain guard: {out}");
        assert!(out.contains("else"), "should contain else: {out}");
    }

    #[test]
    fn compact_guard() {
        let m = parse_source("f foo(x: i32) { guard x > 0 else { return 0; } }");
        let out = format_agent(&m);
        assert!(out.contains("gd"), "should contain gd: {out}");
    }

    #[test]
    fn expand_defer() {
        let m = parse_source("f foo() { df close(); }");
        let out = format_human(&m);
        assert!(out.contains("defer"), "should contain defer: {out}");
    }

    #[test]
    fn compact_defer() {
        let m = parse_source("f foo() { defer close(); }");
        let out = format_agent(&m);
        assert!(out.contains("df"), "should contain df: {out}");
    }

    #[test]
    fn expand_pipeline() {
        let m = parse_source("f main() { x |> foo }");
        let out = format_human(&m);
        assert!(out.contains("|>"), "should contain |>: {out}");
    }

    #[test]
    fn expand_data_record() {
        let m = parse_source("D Point(x: f64, y: f64)");
        let out = format_human(&m);
        assert!(
            out.contains("data Point"),
            "should contain data Point: {out}"
        );
    }

    #[test]
    fn compact_data_record() {
        let m = parse_source("data Point(x: f64, y: f64)");
        let out = format_agent(&m);
        assert!(out.contains("D Point"), "should contain D Point: {out}");
    }

    #[test]
    fn expand_extend() {
        let m = parse_source("xd Point { f foo() -> i32 { 0 } }");
        let out = format_human(&m);
        assert!(out.contains("extend"), "should contain extend: {out}");
    }

    #[test]
    fn compact_extend() {
        let m = parse_source("extend Point { f foo() -> i32 { 0 } }");
        let out = format_agent(&m);
        assert!(out.contains("xd"), "should contain xd: {out}");
    }

    #[test]
    fn expand_expr_body_fn() {
        let m = parse_source("f double(x: i32) -> i32 = x * 2");
        let out = format_human(&m);
        assert!(out.contains("= x * 2"), "should contain = expr: {out}");
    }

    #[test]
    fn expand_error_union() {
        let m = parse_source("f read() -> String or IoError { }");
        let out = format_human(&m);
        assert!(out.contains("or"), "human mode should use 'or': {out}");
    }

    #[test]
    fn compact_error_union() {
        let m = parse_source("f read() -> String or IoError { }");
        let out = format_agent(&m);
        assert!(out.contains("R["), "agent mode should use R[]: {out}");
    }

    // ── Round-trip (parse → print → parse) ───────────────────────────

    #[test]
    fn round_trip_struct_literals_and_patterns() {
        // A struct literal is `@Path { … }`; without the `@` it reads as a name
        // followed by a block, so a function tail split in two.
        assert_round_trips(
            "S Dropout { rate: f32 }
             f make(rate: f32) -> Dropout { @Dropout { rate: rate } }
             f pair() -> [Dropout] { [@Dropout { rate: 0.5 }, @Dropout { rate: 0.1 }] }
             f size_of(d: Dropout) -> f32 { ?= d { @Dropout { rate: r } => r, } }",
        );
    }

    #[test]
    fn round_trip_string_and_special_types() {
        // `s` is the string type; `String` would come back as a path.
        assert_round_trips(
            "S Person { name: s, tags: [s]~ }
             f greet(p: &Person) -> s { p.name }
             f same(x: _T) -> _T { x }
             f ask(k: kb) -> s { \"\" }
             f bump(c: &!i32) {}
             f nested(m: Map[s, List[i32]]) {}
             f shaped(t: tensor[f32, 2, n]) {}
             f learn(p: policy[s, i32], g: genome[f32], w: param[f32, 3]) {}",
        );
    }

    #[test]
    fn round_trip_result_types() {
        // `T or E` is read only as a return type; a result in any other position
        // is `R[T, E]`.
        assert_round_trips(
            "f read(path: s) -> s or IoError { path }
             f wrap(r: R[s, IoError]) -> R[s, IoError] { r }
             f hold() { v r: R[i32, s] = 0; }",
        );
    }

    #[test]
    fn round_trip_binary_precedence() {
        // Each parenthesised operand binds looser than its parent, so the
        // parentheses must survive printing.
        assert_round_trips(
            "f calc(a: i32, b: i32, c: i32) -> i32 {
                 v x = (a + b) * c;
                 v y = a - (b - c);
                 v z = a / (b * c);
                 v w = a + b * c;
                 v eq = a == (b < c);
                 v t = (a == b) < c;
                 v r = -(a + b) * c;
                 v p = a |> (b |> c);
                 x + y + z + w + eq + t + r
             }",
        );
    }

    #[test]
    fn round_trip_unary_and_postfix() {
        // A prefix or postfix operator needs its operand parenthesised when the
        // operand is an operator application or another prefix operator.
        assert_round_trips(
            "f post(a: i32, b: i32, xs: [i32]) -> i32 {
                 v n = (a + b).abs();
                 v k = (-a).abs();
                 v c = (a + b) as f64;
                 v d = -(-a);
                 v e = !(a == b);
                 v g = xs[a + 1];
                 v h = (a + b)?;
                 n + k + d + g + h
             }",
        );
    }

    #[test]
    fn round_trip_await_turbofish_and_patterns() {
        // `.await` (not `.w`, which reads as a field), a turbofish call, a
        // one-element tuple, and destructuring binders (which need `val`).
        assert_round_trips(
            "f fetch(url: s) -> s { url }
             f run(xs: [i32]) -> i32 {
                 v s = fetch(\"a\").await;
                 v n = xs.len::<i32>();
                 v t = (1,);
                 val (a, b) = (2, 3);
                 val [h, ..rest] = xs;
                 val [..] = xs;
                 n
             }",
        );
    }

    #[test]
    fn round_trip_break_todo_and_scrutinee_less_match() {
        // `break 3` is spelled `break` in agent mode too, since `! 3` is a
        // logical not. `??` is todo, and `? { … }` is a match with no scrutinee.
        assert_round_trips(
            "f spin() -> i32 {
                 v r = @@ { break 3; };
                 ??
             }
             f pick(x: i32) -> i32 { ? { 0 => 1, _ => x } }",
        );
    }

    #[test]
    fn round_trip_swarm_size_expression() {
        // `size:` is an expression. The printer wrote `size: ...`, which the
        // parser cannot read. Only the human spelling is checked here: the
        // agent-mode swarm sigil does not parse yet.
        let m = parse_source(
            "swarm ReviewTeam {
                 agent: Critic
                 size: 2 + 1
                 topology: mesh
             }",
        );
        let before = serde_json::to_value(&m).unwrap();
        let out = format_human(&m);
        let again = parser::parse(&lexer::lex(&out)).unwrap_or_else(|e| {
            panic!("human: reparse failed at {}:{}: {}\n{out}", e.line, e.col, e.message)
        });
        assert_eq!(serde_json::to_value(&again).unwrap(), before, "AST changed:\n{out}");
    }
}
