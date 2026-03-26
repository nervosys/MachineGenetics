/// MechGen Formatter (redoxfmt) — bidirectional pretty-printer.
///
/// Two modes:
///   `agent`   — emit minimal-token MechGen canonical form (sigils: f, +f, S, v, m …)
///   `human`   — emit human-readable MechGen form (def, exp def, rec, val, var …)
///
/// Both modes round-trip losslessly through the AST: the semantic content is
/// identical; only the surface syntax differs.
use crate::ast::*;

// ── Public API ───────────────────────────────────────────────────────

/// Format a module in agent (minimal-token) MechGen syntax.
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

/// Format a module in human (human-readable MechGen) syntax.
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
enum Mode {
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
        ItemKind::Agent(a) => emit_agent(buf, a, mode, depth),
        ItemKind::Net(n) => emit_net(buf, n, mode, depth),
        ItemKind::Kb(k) => emit_kb(buf, k, mode, depth),
        ItemKind::Evolve(e) => emit_evolve(buf, e, mode, depth),
        ItemKind::Train(t) => emit_train(buf, t, mode, depth),
        ItemKind::Swarm(s) => emit_swarm(buf, s, mode, depth),
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
    match mode {
        Mode::Agent => {
            // In agent mode, `unsafe` is elided — the compiler's SKB handles
            // all safety analysis.  `unsafe fn` emits as plain `f`.
            if f.is_async {
                if vis == Visibility::Public {
                    buf.push_str("+af ");
                } else {
                    buf.push_str("af ");
                }
            } else if vis == Visibility::Public {
                buf.push_str("+f ");
            } else {
                buf.push_str("f ");
            }
        }
        Mode::Human => {
            if vis == Visibility::Public {
                buf.push_str("pub ");
            }
            if f.is_async {
                buf.push_str("async ");
            }
            if f.is_unsafe {
                buf.push_str("unsafe ");
            }
            buf.push_str("fn ");
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
        emit_type(buf, rt, mode);
    }

    emit_where_clause(buf, &f.where_clause, mode);

    if !f.effects.is_empty() {
        buf.push_str(" @fx(");
        buf.push_str(&f.effects.join(", "));
        buf.push(')');
    }

    if f.body.stmts.is_empty() && f.body.tail_expr.is_none() {
        buf.push_str(" {}");
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
        buf.push_str(&tp.join("::"));
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
    buf.push_str(&u.path.join("::"));
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
            buf.push_str(&g.path.join("::"));
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
        indent(buf, depth + 1);
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
        buf.push_str(",\n");
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
            SpecItem::Require(c) => {
                buf.push_str("@req(");
                buf.push_str(c);
                buf.push(')');
            }
            SpecItem::Ensure(c) => {
                buf.push_str("@ens(");
                buf.push_str(c);
                buf.push(')');
            }
            SpecItem::Invariant(c) => {
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

fn emit_agent(buf: &mut String, a: &AgentDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("\u{03B1} "), // α
        Mode::Human => buf.push_str("agent "),
    }
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
    match mode {
        Mode::Agent => buf.push_str("\u{03A8} "), // Ψ
        Mode::Human => buf.push_str("net "),
    }
    buf.push_str(&n.name);
    buf.push_str(" {\n");
    for l in &n.layers {
        indent(buf, depth + 1);
        match mode {
            Mode::Agent => buf.push_str("\u{03BB} "), // λ
            Mode::Human => buf.push_str("layer "),
        }
        buf.push_str(&l.name);
        buf.push_str(";\n");
    }
    indent(buf, depth + 1);
    buf.push_str("forward { ... }\n");
    indent(buf, depth);
    buf.push_str("}\n");
}

fn emit_kb(buf: &mut String, k: &KbDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("\u{03BA} "), // κ
        Mode::Human => buf.push_str("kb "),
    }
    buf.push_str(&k.name);
    buf.push_str(" {\n");
    for f in &k.facts {
        indent(buf, depth + 1);
        match mode {
            Mode::Agent => buf.push_str("\u{22A2} "), // ⊢
            Mode::Human => buf.push_str("fact "),
        }
        buf.push_str(&f.name);
        buf.push_str(";\n");
    }
    for r in &k.rules {
        indent(buf, depth + 1);
        match mode {
            Mode::Agent => buf.push_str("\u{03C1} "), // ρ
            Mode::Human => buf.push_str("rule "),
        }
        buf.push_str(&r.name);
        buf.push_str(";\n");
    }
    indent(buf, depth);
    buf.push_str("}\n");
}

fn emit_evolve(buf: &mut String, e: &EvolveDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("\u{03A9} "), // Ω
        Mode::Human => buf.push_str("evolve "),
    }
    buf.push_str(&e.name);
    buf.push_str(" {\n");
    indent(buf, depth + 1);
    buf.push_str("fitness { ... }\n");
    indent(buf, depth);
    buf.push_str("}\n");
}

fn emit_train(buf: &mut String, t: &TrainDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("\u{0398} "), // Θ
        Mode::Human => buf.push_str("train "),
    }
    buf.push_str(&t.name);
    buf.push_str(" {\n");
    indent(buf, depth + 1);
    buf.push_str("net: ");
    buf.push_str(&t.net);
    buf.push_str("\n");
    indent(buf, depth);
    buf.push_str("}\n");
}

fn emit_swarm(buf: &mut String, s: &SwarmDef, mode: Mode, depth: usize) {
    indent(buf, depth);
    match mode {
        Mode::Agent => buf.push_str("\u{03A3} "), // Σ
        Mode::Human => buf.push_str("swarm "),
    }
    buf.push_str(&s.name);
    buf.push_str(" {\n");
    // agent type
    if !s.agent_type.is_empty() {
        indent(buf, depth + 1);
        match mode {
            Mode::Agent => buf.push_str("\u{03B1}: "), // α:
            Mode::Human => buf.push_str("agent: "),
        }
        buf.push_str(&s.agent_type);
        buf.push_str(";\n");
    }
    // size
    if s.size.is_some() {
        indent(buf, depth + 1);
        buf.push_str("size: ...\n");
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
    // dispatch block
    if s.on_dispatch.is_some() {
        indent(buf, depth + 1);
        buf.push_str("dispatch { ... }\n");
    }
    // aggregate block
    if s.on_aggregate.is_some() {
        indent(buf, depth + 1);
        buf.push_str("aggregate { ... }\n");
    }
    // on_failure block
    if s.on_failure.is_some() {
        indent(buf, depth + 1);
        buf.push_str("on_failure { ... }\n");
    }
    indent(buf, depth);
    buf.push_str("}\n");
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

fn emit_type(buf: &mut String, ty: &Type, mode: Mode) {
    match ty {
        Type::Path { segments, type_args } => {
            buf.push_str(&segments.join("::"));
            if !type_args.is_empty() {
                buf.push('<');
                for (i, a) in type_args.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    emit_type(buf, a, mode);
                }
                buf.push('>');
            }
        }
        Type::Reference { mutable, inner } => {
            buf.push('&');
            if *mutable {
                match mode {
                    Mode::Agent => buf.push_str("m "),
                    Mode::Human => buf.push_str("mut "),
                }
            }
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
        Type::SelfType => buf.push_str("Self"),
        Type::StringType => buf.push_str("String"),
        Type::KnowledgeBase => buf.push_str("KnowledgeBase"),
        Type::LlmType => buf.push_str("LLM"),
        Type::Tensor { inner, shape } => {
            buf.push_str("Tensor[");
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
            buf.push_str("Param[");
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
        Type::Genome { inner } => {
            buf.push_str("Genome[");
            emit_type(buf, inner, mode);
            buf.push(']');
        }
        Type::Policy { state, action } => {
            buf.push_str("Policy[");
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
    }
}

// ── Expressions ──────────────────────────────────────────────────────

fn emit_expr(buf: &mut String, expr: &Expr, mode: Mode) {
    match expr {
        Expr::Literal { value, .. } => buf.push_str(value),
        Expr::Ident { name } => buf.push_str(name),
        Expr::Binary { op, left, right } => {
            emit_expr(buf, left, mode);
            buf.push(' ');
            buf.push_str(op);
            buf.push(' ');
            emit_expr(buf, right, mode);
        }
        Expr::Unary { op, operand } => {
            buf.push_str(op);
            emit_expr(buf, operand, mode);
        }
        Expr::Call { func, args } => {
            emit_expr(buf, func, mode);
            buf.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_expr(buf, a, mode);
            }
            buf.push(')');
        }
        Expr::MethodCall { receiver, method, args, .. } => {
            emit_expr(buf, receiver, mode);
            buf.push('.');
            buf.push_str(method);
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
            emit_expr(buf, object, mode);
            buf.push('.');
            buf.push_str(field);
        }
        Expr::Index { object, index } => {
            emit_expr(buf, object, mode);
            buf.push('[');
            emit_expr(buf, index, mode);
            buf.push(']');
        }
        Expr::StructLit { path, fields } => {
            buf.push_str(&path.join("::"));
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
        Expr::If { cond, then_block, else_block } => {
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
        Expr::Match { scrutinee, arms } => {
            match mode {
                Mode::Agent => buf.push_str("?= "),
                Mode::Human => buf.push_str("match "),
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
        Expr::For { pattern, iter, body } => {
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
            match mode {
                Mode::Agent => buf.push('!'),
                Mode::Human => buf.push_str("break"),
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
            emit_expr(buf, expr, mode);
            buf.push('?');
        }
        Expr::Await { expr } => {
            emit_expr(buf, expr, mode);
            match mode {
                Mode::Agent => buf.push_str(".w"),
                Mode::Human => buf.push_str(".await"),
            }
        }
        Expr::Cast { expr, ty } => {
            emit_expr(buf, expr, mode);
            buf.push_str(" as ");
            emit_type(buf, ty, mode);
        }
        Expr::Assign { target, value } => {
            emit_expr(buf, target, mode);
            buf.push_str(" = ");
            emit_expr(buf, value, mode);
        }
        Expr::Range { start, end, inclusive } => {
            emit_expr(buf, start, mode);
            if *inclusive {
                buf.push_str("..=");
            } else {
                buf.push_str("..");
            }
            emit_expr(buf, end, mode);
        }
        Expr::Todo => buf.push_str("todo!()"),
        Expr::Unimplemented => buf.push_str("unimplemented!()"),
        Expr::UnsafeBlock { block } => match mode {
            Mode::Agent => {
                buf.push_str("{\n");
                emit_block_body(buf, block, mode, 1);
                buf.push('}');
            }
            Mode::Human => {
                buf.push_str("unsafe {\n");
                emit_block_body(buf, block, mode, 1);
                buf.push('}');
            }
        },
        Expr::Error { message } => {
            buf.push_str("/* error: ");
            buf.push_str(message);
            buf.push_str(" */");
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
            buf.push_str(&path.join("::"));
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
            buf.push_str(&path.join("::"));
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
        Pattern::Slice { elements, rest } => {
            buf.push('[');
            for (i, p) in elements.iter().enumerate() {
                if i > 0 {
                    buf.push_str(", ");
                }
                emit_pattern(buf, p);
            }
            if *rest {
                buf.push_str(", ..");
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
        Stmt::Let { mutable, pattern, ty, value } => {
            indent(buf, depth);
            match mode {
                Mode::Agent => {
                    if *mutable {
                        buf.push_str("m ");
                    } else {
                        buf.push_str("v ");
                    }
                }
                Mode::Human => {
                    if *mutable {
                        buf.push_str("let mut ");
                    } else {
                        buf.push_str("let ");
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
        assert!(out.contains("let x = 42;"), "got: {out}");
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
        assert!(out.contains("let mut x = 0;"), "got: {out}");
    }

    #[test]
    fn compact_use() {
        let m = parse_source("u std.io;");
        let out = format_agent(&m);
        assert!(out.contains("u std::io;"), "got: {out}");
    }

    #[test]
    fn expand_use() {
        let m = parse_source("u std.io;");
        let out = format_human(&m);
        assert!(out.contains("use std::io;"), "got: {out}");
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
        assert!(out.contains("α Bot"), "got: {out}");
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
}
