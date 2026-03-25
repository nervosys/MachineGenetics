/// SKB — Safety Knowledge Base (proposal §15).
///
/// Eight rule databases:
///   1. Ownership Rules  (OWN-*)
///   2. Borrow Patterns  (BOR-*)
///   3. Lifetime Rules   (LIF-*)
///   4. Type Safety      (TYP-*)
///   5. Concurrency      (CON-*)
///   6. FFI Safety       (FFI-*)
///   7. Agent Elision    (AEL-*) — rules for safety constructs the compiler
///      handles automatically in agent mode so they can be elided from syntax.
///   8. Swarm Safety     (SWM-*) — rules for multi-agent swarm coordination,
///      consensus, topology, fault tolerance, and deadlock prevention.
///
/// Plus the original symbol metadata entries used by the query API.
///
/// SKB-QL syntax (simplified):
///   SELECT effects FROM std.io.read_file
///   SELECT cost WHERE construct = "Vec::push" AND target = "x86_64"
///   MATCH UseAfterMove IN function("process_data")
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════════════
//  Rule Data Model (proposal §15.1)
// ══════════════════════════════════════════════════════════════════════

/// Which of the seven rule databases a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleDatabase {
    Ownership,
    Borrow,
    Lifetime,
    TypeSafety,
    Concurrency,
    FFI,
    /// Agent elision — rules the compiler enforces so agent mode doesn't
    /// need explicit safety syntax.
    AgentElision,
    /// Swarm safety — rules for multi-agent coordination, consensus,
    /// topology, fault tolerance, and deadlock prevention.
    SwarmSafety,
}

/// Severity of a rule violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A single safety rule in the SKB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique identifier, e.g. "OWN-0001".
    pub id: String,
    /// Which database this rule belongs to.
    pub database: RuleDatabase,
    /// Short category label, e.g. "use-after-move".
    pub category: String,
    /// How severe a violation is.
    pub severity: RuleSeverity,
    /// Human/agent-readable description.
    pub description: String,
    /// Why this rule exists.
    pub rationale: String,
    /// Auto-fix template (if applicable).
    pub fix_template: Option<String>,
    /// Confidence in the auto-fix (0.0–1.0).
    pub fix_confidence: f64,
    /// Searchable tags.
    pub tags: Vec<String>,
}

// ── Rule construction helper ─────────────────────────────────────────

fn rule(
    id: &str,
    db: RuleDatabase,
    cat: &str,
    sev: RuleSeverity,
    desc: &str,
    rationale: &str,
    fix: Option<&str>,
    conf: f64,
    tags: &[&str],
) -> Rule {
    Rule {
        id: id.into(),
        database: db,
        category: cat.into(),
        severity: sev,
        description: desc.into(),
        rationale: rationale.into(),
        fix_template: fix.map(Into::into),
        fix_confidence: conf,
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

// ══════════════════════════════════════════════════════════════════════
//  Built-in Rule Databases — 255 rules across 8 databases
// ══════════════════════════════════════════════════════════════════════

fn builtin_rules() -> Vec<Rule> {
    let mut rules = Vec::with_capacity(255);

    // ── Ownership Rules (OWN-0001 .. OWN-0040) ──────────────────────
    rules.extend(ownership_rules());
    // ── Borrow Patterns (BOR-0001 .. BOR-0040) ──────────────────────
    rules.extend(borrow_rules());
    // ── Lifetime Rules (LIF-0001 .. LIF-0035) ───────────────────────
    rules.extend(lifetime_rules());
    // ── Type Safety (TYP-0001 .. TYP-0040) ──────────────────────────
    rules.extend(type_safety_rules());
    // ── Concurrency (CON-0001 .. CON-0035) ──────────────────────────
    rules.extend(concurrency_rules());
    // ── FFI Safety (FFI-0001 .. FFI-0020) ────────────────────────────
    rules.extend(ffi_rules());
    // ── Agent Elision (AEL-0001 .. AEL-0030) ────────────────────────
    rules.extend(agent_elision_rules());
    // ── Swarm Safety (SWM-0001 .. SWM-0015) ──────────────────────────
    rules.extend(swarm_safety_rules());

    rules
}

fn ownership_rules() -> Vec<Rule> {
    use RuleDatabase::Ownership as DB;
    use RuleSeverity::*;
    vec![
        rule(
            "OWN-0001",
            DB,
            "use-after-move",
            Error,
            "Use of value after move",
            "Moved values have undefined state",
            Some("Clone the value before the move, or restructure to use a reference"),
            0.85,
            &["ownership", "move", "use-after-move"],
        ),
        rule(
            "OWN-0002",
            DB,
            "double-move",
            Error,
            "Value moved twice",
            "A value can only be moved once",
            Some("Clone before first move"),
            0.90,
            &["ownership", "move", "double-move"],
        ),
        rule(
            "OWN-0003",
            DB,
            "move-in-loop",
            Error,
            "Value moved inside loop iteration",
            "Loop body runs multiple times; value consumed on first iteration",
            Some("Clone inside loop or move value before loop"),
            0.80,
            &["ownership", "move", "loop"],
        ),
        rule(
            "OWN-0004",
            DB,
            "partial-move",
            Warning,
            "Partial move out of struct",
            "Field moved but struct still referenced",
            Some("Move entire struct or clone the field"),
            0.75,
            &["ownership", "partial-move", "struct"],
        ),
        rule(
            "OWN-0005",
            DB,
            "move-closure",
            Error,
            "Value moved into closure but used after",
            "Closure takes ownership; value unavailable after",
            Some("Use reference in closure or clone"),
            0.85,
            &["ownership", "closure", "move"],
        ),
        rule(
            "OWN-0006",
            DB,
            "drop-while-borrowed",
            Error,
            "Value dropped while still borrowed",
            "Reference outlives its referent",
            Some("Extend the value's lifetime or shorten the borrow"),
            0.70,
            &["ownership", "drop", "borrow"],
        ),
        rule(
            "OWN-0007",
            DB,
            "copy-large-struct",
            Warning,
            "Implicit copy of large struct (>256 bytes)",
            "Large copies are expensive; prefer reference or clone explicitly",
            Some("Pass by reference instead"),
            0.65,
            &["ownership", "copy", "performance"],
        ),
        rule(
            "OWN-0008",
            DB,
            "unused-owned-value",
            Warning,
            "Owned value created but never used",
            "Unnecessary allocation",
            Some("Remove the unused binding"),
            0.95,
            &["ownership", "unused", "lint"],
        ),
        rule(
            "OWN-0009",
            DB,
            "return-local-ref",
            Error,
            "Returning reference to local variable",
            "Local variable dropped at end of function",
            Some("Return owned value instead"),
            0.90,
            &["ownership", "return", "reference", "dangling"],
        ),
        rule(
            "OWN-0010",
            DB,
            "move-behind-ref",
            Error,
            "Cannot move out of a shared reference",
            "Shared references don't grant ownership",
            Some("Clone the value instead"),
            0.90,
            &["ownership", "move", "reference"],
        ),
        rule(
            "OWN-0011",
            DB,
            "box-unnecessary",
            Info,
            "Unnecessary Box allocation for small type",
            "Value fits on stack — heap allocation is overhead",
            Some("Remove Box wrapper"),
            0.80,
            &["ownership", "box", "heap", "performance"],
        ),
        rule(
            "OWN-0012",
            DB,
            "rc-cycle",
            Warning,
            "Potential Rc reference cycle detected",
            "Rc cycles cause memory leaks",
            Some("Use Weak for back-references"),
            0.60,
            &["ownership", "rc", "cycle", "leak"],
        ),
        rule(
            "OWN-0013",
            DB,
            "arc-single-thread",
            Info,
            "Arc used in single-threaded context",
            "Arc overhead unnecessary without concurrency",
            Some("Replace with Rc"),
            0.75,
            &["ownership", "arc", "rc", "performance"],
        ),
        rule(
            "OWN-0014",
            DB,
            "cow-never-cloned",
            Info,
            "Cow never clones — always borrows",
            "Cow overhead unnecessary if never owned",
            Some("Replace with a plain reference"),
            0.70,
            &["ownership", "cow", "performance"],
        ),
        rule(
            "OWN-0015",
            DB,
            "leak-forget",
            Warning,
            "mem::forget called — value leaked intentionally?",
            "Forgetting values skips Drop; may leak resources",
            None,
            0.0,
            &["ownership", "forget", "leak"],
        ),
        rule(
            "OWN-0016",
            DB,
            "double-free",
            Error,
            "Potential double-free via manual drop",
            "Drop called explicitly then scope-end drop runs again",
            Some("Remove explicit drop call"),
            0.90,
            &["ownership", "drop", "double-free"],
        ),
        rule(
            "OWN-0017",
            DB,
            "move-out-of-option",
            Warning,
            "Move out of Option without take()",
            "Leaves Option in undefined state",
            Some("Use .take() instead"),
            0.80,
            &["ownership", "option", "take"],
        ),
        rule(
            "OWN-0018",
            DB,
            "self-referential",
            Error,
            "Self-referential struct detected",
            "Moving the struct invalidates internal pointers",
            Some("Use Pin or indirect references"),
            0.50,
            &["ownership", "self-referential", "pin"],
        ),
        rule(
            "OWN-0019",
            DB,
            "vec-into-iter-reuse",
            Error,
            "Vec reused after into_iter() consumes it",
            "into_iter moves the Vec",
            Some("Use iter() for borrowing iteration"),
            0.90,
            &["ownership", "vec", "iterator", "move"],
        ),
        rule(
            "OWN-0020",
            DB,
            "string-move-concat",
            Warning,
            "String moved during concatenation",
            "format! or + operator may consume the string",
            Some("Use &str reference or clone"),
            0.75,
            &["ownership", "string", "concatenation"],
        ),
        rule(
            "OWN-0021",
            DB,
            "generic-move",
            Info,
            "Generic parameter requires move semantics",
            "T without Copy bound will be moved",
            Some("Add Copy bound or use references"),
            0.65,
            &["ownership", "generics", "copy"],
        ),
        rule(
            "OWN-0022",
            DB,
            "destructure-move",
            Warning,
            "Destructuring moves fields out of struct",
            "Pattern match can partially move struct fields",
            Some("Use ref in pattern or clone"),
            0.70,
            &["ownership", "destructure", "pattern"],
        ),
        rule(
            "OWN-0023",
            DB,
            "temporary-lifetime",
            Warning,
            "Temporary value dropped too early",
            "Expression result dropped before reference used",
            Some("Bind temporary to a variable"),
            0.80,
            &["ownership", "temporary", "lifetime"],
        ),
        rule(
            "OWN-0024",
            DB,
            "swap-idiom",
            Hint,
            "Manual swap can use std::mem::swap",
            "Swap idiom is clearer and avoids temporaries",
            Some("Replace with mem::swap(&a, &b)"),
            0.90,
            &["ownership", "swap", "idiom"],
        ),
        rule(
            "OWN-0025",
            DB,
            "clone-on-copy",
            Info,
            "clone() called on Copy type",
            "Copy types are bitwise-copied; .clone() is redundant",
            Some("Remove .clone()"),
            0.95,
            &["ownership", "clone", "copy", "lint"],
        ),
        rule(
            "OWN-0026",
            DB,
            "take-from-default",
            Hint,
            "Use mem::take to replace value with default",
            "mem::take is cleaner than manual replace with Default",
            Some("Replace with mem::take(&v)"),
            0.85,
            &["ownership", "take", "default", "idiom"],
        ),
        rule(
            "OWN-0027",
            DB,
            "move-semantic-closure-env",
            Info,
            "Closure captures environment by move",
            "All referenced variables will be moved into the closure",
            None,
            0.0,
            &["ownership", "closure", "environment"],
        ),
        rule(
            "OWN-0028",
            DB,
            "map-entry-api",
            Hint,
            "Use entry API instead of contains-then-insert",
            "Entry API avoids double lookup",
            Some("Replace with map.entry(k).or_insert(v)"),
            0.90,
            &["ownership", "hashmap", "entry", "idiom"],
        ),
        rule(
            "OWN-0029",
            DB,
            "drain-vs-clear",
            Hint,
            "Use drain() when values should be processed before clearing",
            "drain() yields ownership of elements",
            Some("Replace clear() with drain()"),
            0.70,
            &["ownership", "drain", "clear", "vec"],
        ),
        rule(
            "OWN-0030",
            DB,
            "iter-ownership",
            Info,
            "iter() borrows, into_iter() moves, iter_mut() borrows mutably",
            "Choose iterator method based on ownership needs",
            None,
            0.0,
            &["ownership", "iterator", "reference"],
        ),
        rule(
            "OWN-0031",
            DB,
            "phantom-data-usage",
            Info,
            "PhantomData marks type as owning T for drop-check",
            "Used for variance and drop-check",
            None,
            0.0,
            &["ownership", "phantomdata", "variance"],
        ),
        rule(
            "OWN-0032",
            DB,
            "manually-drop",
            Warning,
            "ManuallyDrop used — ensure value is properly handled",
            "Value will not be dropped automatically",
            None,
            0.0,
            &["ownership", "manuallydrop", "unsafe"],
        ),
        rule(
            "OWN-0033",
            DB,
            "pin-unpin",
            Info,
            "Pinned values cannot be moved unless Unpin",
            "Pin guarantees address stability",
            None,
            0.0,
            &["ownership", "pin", "unpin"],
        ),
        rule(
            "OWN-0034",
            DB,
            "to-owned-vs-clone",
            Hint,
            "Prefer to_owned() on borrowed types, clone() on owned",
            "to_owned() is idiomatic for &str → String",
            Some("Replace .clone() with .to_owned()"),
            0.80,
            &["ownership", "clone", "to_owned", "idiom"],
        ),
        rule(
            "OWN-0035",
            DB,
            "into-conversion",
            Hint,
            "Use .into() for type conversions instead of explicit constructors",
            "Into trait provides idiomatic conversions",
            Some("Replace constructor with .into()"),
            0.75,
            &["ownership", "into", "conversion"],
        ),
        rule(
            "OWN-0036",
            DB,
            "vec-capacity",
            Info,
            "Vec::with_capacity reduces reallocations when size is known",
            "Pre-allocating avoids repeated heap allocations",
            Some("Use Vec::with_capacity(n)"),
            0.80,
            &["ownership", "vec", "capacity", "performance"],
        ),
        rule(
            "OWN-0037",
            DB,
            "string-capacity",
            Info,
            "String::with_capacity reduces reallocations when length is known",
            "Pre-allocating avoids repeated heap allocations",
            Some("Use String::with_capacity(n)"),
            0.80,
            &["ownership", "string", "capacity", "performance"],
        ),
        rule(
            "OWN-0038",
            DB,
            "box-leak",
            Warning,
            "Box::leak creates a 'static reference — memory never freed",
            "Intentional leak; ensure this is desired",
            None,
            0.0,
            &["ownership", "box", "leak", "static"],
        ),
        rule(
            "OWN-0039",
            DB,
            "transmute-ownership",
            Error,
            "mem::transmute changes type but not ownership semantics",
            "Transmute may violate ownership invariants",
            None,
            0.0,
            &["ownership", "transmute", "unsafe"],
        ),
        rule(
            "OWN-0040",
            DB,
            "drop-order",
            Info,
            "Struct fields dropped in declaration order",
            "Drop order can matter for resource cleanup",
            None,
            0.0,
            &["ownership", "drop", "order"],
        ),
    ]
}

fn borrow_rules() -> Vec<Rule> {
    use RuleDatabase::Borrow as DB;
    use RuleSeverity::*;
    vec![
        rule(
            "BOR-0001",
            DB,
            "double-mutable-borrow",
            Error,
            "Two mutable borrows of same value active simultaneously",
            "Rust/MechGen allows only one &mut at a time",
            Some("Restructure to avoid overlapping mutable borrows"),
            0.85,
            &["borrow", "mutable", "aliasing"],
        ),
        rule(
            "BOR-0002",
            DB,
            "mutable-and-shared",
            Error,
            "Mutable and shared borrow active simultaneously",
            "Cannot have &mut and & at the same time",
            Some("Use shared borrow only, or restructure"),
            0.85,
            &["borrow", "mutable", "shared", "aliasing"],
        ),
        rule(
            "BOR-0003",
            DB,
            "borrow-in-loop-mutate",
            Error,
            "Immutable borrow in loop body, mutable usage outside",
            "Borrow persists across loop iteration",
            Some("Clone or restructure borrow scope"),
            0.70,
            &["borrow", "loop", "mutable"],
        ),
        rule(
            "BOR-0004",
            DB,
            "borrow-escapes-scope",
            Error,
            "Borrow escapes the scope of the referent",
            "Reference outlives the value it points to",
            Some("Shorten borrow or extend referent lifetime"),
            0.80,
            &["borrow", "escape", "scope"],
        ),
        rule(
            "BOR-0005",
            DB,
            "iterator-invalidation",
            Error,
            "Collection mutated while iterator is active",
            "Iterators hold borrows; mutation invalidates them",
            Some("Collect indices first, then mutate"),
            0.75,
            &["borrow", "iterator", "invalidation", "collection"],
        ),
        rule(
            "BOR-0006",
            DB,
            "reborrow-implicit",
            Info,
            "Implicit reborrow of &mut to &",
            "Compiler inserts reborrow automatically",
            None,
            0.0,
            &["borrow", "reborrow", "implicit"],
        ),
        rule(
            "BOR-0007",
            DB,
            "borrow-split",
            Hint,
            "Borrow can be split into disjoint fields",
            "Borrowing different struct fields is allowed",
            Some("Borrow specific fields instead of whole struct"),
            0.80,
            &["borrow", "split", "disjoint", "struct"],
        ),
        rule(
            "BOR-0008",
            DB,
            "nll-scope",
            Info,
            "Non-lexical lifetime: borrow ends at last use",
            "Borrows end at last use, not at block end",
            None,
            0.0,
            &["borrow", "nll", "scope"],
        ),
        rule(
            "BOR-0009",
            DB,
            "temporary-borrow",
            Warning,
            "Borrowing a temporary expression",
            "Temporary dropped at end of statement",
            Some("Bind expression to variable first"),
            0.85,
            &["borrow", "temporary", "drop"],
        ),
        rule(
            "BOR-0010",
            DB,
            "closure-borrow-conflict",
            Error,
            "Closure borrows value conflicting with outer code",
            "Closure captures borrow; outer code can't use value",
            Some("Move value into closure or restructure"),
            0.70,
            &["borrow", "closure", "conflict"],
        ),
        rule(
            "BOR-0011",
            DB,
            "ref-to-moved",
            Error,
            "Reference to moved value",
            "Value moved; reference is dangling",
            Some("Clone before moving, or take reference before move"),
            0.90,
            &["borrow", "move", "dangling"],
        ),
        rule(
            "BOR-0012",
            DB,
            "cell-shared-mutation",
            Info,
            "Cell/RefCell allow interior mutability through shared reference",
            "Shared mutation via runtime checks instead of &mut",
            None,
            0.0,
            &["borrow", "cell", "interior-mutability"],
        ),
        rule(
            "BOR-0013",
            DB,
            "refcell-runtime-panic",
            Warning,
            "RefCell borrow can panic at runtime if already borrowed mutably",
            "borrow() panics if borrow_mut() is active",
            Some("Use try_borrow() to handle gracefully"),
            0.80,
            &["borrow", "refcell", "panic", "runtime"],
        ),
        rule(
            "BOR-0014",
            DB,
            "index-borrow",
            Info,
            "Indexing operation implicitly borrows the collection",
            "Index creates a shared or mutable borrow",
            None,
            0.0,
            &["borrow", "index", "implicit"],
        ),
        rule(
            "BOR-0015",
            DB,
            "deref-coercion",
            Info,
            "Deref coercion converts &Box<T> to &T automatically",
            "Deref trait enables transparent borrowing",
            None,
            0.0,
            &["borrow", "deref", "coercion"],
        ),
        rule(
            "BOR-0016",
            DB,
            "as-ref-as-mut",
            Hint,
            "Use AsRef/AsMut for generic borrowing in function parameters",
            "AsRef enables accepting &T, &String, &str, etc.",
            Some("Change parameter to impl AsRef<T>"),
            0.75,
            &["borrow", "asref", "generic"],
        ),
        rule(
            "BOR-0017",
            DB,
            "borrow-guard",
            Info,
            "MutexGuard/RwLockGuard holds borrow until dropped",
            "Lock guard lifetime = borrow lifetime",
            None,
            0.0,
            &["borrow", "guard", "lock", "scope"],
        ),
        rule(
            "BOR-0018",
            DB,
            "slice-borrow",
            Info,
            "Slices borrow the underlying collection",
            "&[T] is a borrowed view into Vec or array",
            None,
            0.0,
            &["borrow", "slice", "view"],
        ),
        rule(
            "BOR-0019",
            DB,
            "string-str-borrow",
            Hint,
            "Use &str instead of &String in function parameters",
            "&str is more general; accepts both String and &str",
            Some("Change parameter type to &str"),
            0.90,
            &["borrow", "string", "str", "parameter"],
        ),
        rule(
            "BOR-0020",
            DB,
            "vec-slice-borrow",
            Hint,
            "Use &[T] instead of &Vec<T> in function parameters",
            "&[T] is more general; accepts arrays and slices too",
            Some("Change parameter type to &[T]"),
            0.90,
            &["borrow", "vec", "slice", "parameter"],
        ),
        rule(
            "BOR-0021",
            DB,
            "entry-borrow-conflict",
            Warning,
            "HashMap entry API avoids double borrow on lookup+insert",
            "get() then insert() borrows immutably then mutably",
            Some("Use entry().or_insert()"),
            0.85,
            &["borrow", "hashmap", "entry"],
        ),
        rule(
            "BOR-0022",
            DB,
            "match-borrow",
            Info,
            "Match arms borrow the scrutinee",
            "Pattern matching creates borrows of matched value",
            None,
            0.0,
            &["borrow", "match", "pattern"],
        ),
        rule(
            "BOR-0023",
            DB,
            "nested-borrow",
            Warning,
            "Nested mutable borrows through struct fields",
            "Borrowing field of &mut struct creates nested borrow",
            Some("Split borrows by field"),
            0.70,
            &["borrow", "nested", "struct", "field"],
        ),
        rule(
            "BOR-0024",
            DB,
            "method-receiver",
            Info,
            "&self borrows immutably, &mut self borrows mutably, self moves",
            "Method receiver determines borrow kind",
            None,
            0.0,
            &["borrow", "method", "receiver", "self"],
        ),
        rule(
            "BOR-0025",
            DB,
            "two-phase-borrow",
            Info,
            "Two-phase borrows allow &mut in method call arguments",
            "Compiler splits activation of mutable borrows",
            None,
            0.0,
            &["borrow", "two-phase", "method-call"],
        ),
        rule(
            "BOR-0026",
            DB,
            "autoref",
            Info,
            "Compiler auto-references method receivers",
            ". operator adds & or &mut as needed",
            None,
            0.0,
            &["borrow", "autoref", "method"],
        ),
        rule(
            "BOR-0027",
            DB,
            "pin-borrow",
            Info,
            "Pin<&mut T> prevents moving the borrowed value",
            "Pin ensures address stability through borrow",
            None,
            0.0,
            &["borrow", "pin", "move"],
        ),
        rule(
            "BOR-0028",
            DB,
            "unsafe-alias",
            Error,
            "Unsafe code creates aliased &mut references",
            "Undefined behavior: two &mut to same memory",
            None,
            0.0,
            &["borrow", "unsafe", "aliasing", "ub"],
        ),
        rule(
            "BOR-0029",
            DB,
            "return-borrow",
            Warning,
            "Returned reference must outlive function",
            "Returned &T must live at least as long as caller needs",
            Some("Return owned value or tie lifetime to input"),
            0.75,
            &["borrow", "return", "lifetime"],
        ),
        rule(
            "BOR-0030",
            DB,
            "async-borrow-across-await",
            Error,
            "Borrow held across .await point",
            "Borrow must be valid across suspension; may cause issues",
            Some("Clone value or restructure to not hold borrow across await"),
            0.65,
            &["borrow", "async", "await", "suspension"],
        ),
        rule(
            "BOR-0031",
            DB,
            "fn-once-borrow",
            Info,
            "FnOnce captures by value; FnMut by &mut; Fn by &",
            "Closure trait hierarchy determines capture mode",
            None,
            0.0,
            &["borrow", "closure", "fn-trait"],
        ),
        rule(
            "BOR-0032",
            DB,
            "static-borrow",
            Info,
            "'static borrows live for entire program duration",
            "Only constants and leaked values have 'static lifetime",
            None,
            0.0,
            &["borrow", "static", "lifetime"],
        ),
        rule(
            "BOR-0033",
            DB,
            "borrow-as-ptr",
            Warning,
            "Converting borrow to raw pointer bypasses borrow checker",
            "Raw pointers are unchecked; ensure safety",
            None,
            0.0,
            &["borrow", "raw-pointer", "unsafe"],
        ),
        rule(
            "BOR-0034",
            DB,
            "coerce-unsized",
            Info,
            "&Box<T> coerces to &T via Deref; &Vec<T> to &[T]",
            "Compiler applies deref coercions automatically",
            None,
            0.0,
            &["borrow", "coercion", "unsized"],
        ),
        rule(
            "BOR-0035",
            DB,
            "iter-borrow-kind",
            Info,
            ".iter() yields &T, .iter_mut() yields &mut T",
            "Choose iterator to match needed borrow kind",
            None,
            0.0,
            &["borrow", "iterator", "reference-kind"],
        ),
        rule(
            "BOR-0036",
            DB,
            "map-values-borrow",
            Hint,
            "Use .values() / .values_mut() to iterate only map values",
            "Avoids borrowing keys when only values needed",
            Some("Replace .iter().map(|(_, v)| v) with .values()"),
            0.85,
            &["borrow", "hashmap", "values", "idiom"],
        ),
        rule(
            "BOR-0037",
            DB,
            "windows-chunks",
            Hint,
            "Use .windows() or .chunks() for sliding/fixed slices",
            "Avoids manual index borrowing",
            Some("Replace manual loop with .windows(n) or .chunks(n)"),
            0.80,
            &["borrow", "slice", "windows", "chunks"],
        ),
        rule(
            "BOR-0038",
            DB,
            "collect-borrow-end",
            Info,
            "collect() consumes iterator — ends iterator's borrow",
            "After collect(), original collection is no longer borrowed",
            None,
            0.0,
            &["borrow", "collect", "iterator"],
        ),
        rule(
            "BOR-0039",
            DB,
            "to-vec-clone",
            Info,
            ".to_vec() clones a slice into a new Vec",
            "Creates owned copy; original borrow ends",
            None,
            0.0,
            &["borrow", "to_vec", "clone"],
        ),
        rule(
            "BOR-0040",
            DB,
            "splitn-borrow",
            Info,
            "split / splitn borrows the string or slice",
            "Split iterators keep borrow alive",
            None,
            0.0,
            &["borrow", "split", "string", "slice"],
        ),
    ]
}

fn lifetime_rules() -> Vec<Rule> {
    use RuleDatabase::Lifetime as DB;
    use RuleSeverity::*;
    vec![
        rule(
            "LIF-0001",
            DB,
            "dangling-reference",
            Error,
            "Reference outlives its referent",
            "Accessing freed memory is undefined behavior",
            Some("Return owned value or ensure referent lives long enough"),
            0.85,
            &["lifetime", "dangling", "reference"],
        ),
        rule(
            "LIF-0002",
            DB,
            "lifetime-mismatch",
            Error,
            "Lifetime of reference does not satisfy constraint",
            "Function signature requires longer lifetime than provided",
            Some("Adjust lifetime annotation or restructure"),
            0.70,
            &["lifetime", "mismatch", "constraint"],
        ),
        rule(
            "LIF-0003",
            DB,
            "elision-rules",
            Info,
            "Lifetime elision applies: single input lifetime → output",
            "Compiler infers output lifetime from single input ref",
            None,
            0.0,
            &["lifetime", "elision", "inference"],
        ),
        rule(
            "LIF-0004",
            DB,
            "elision-self",
            Info,
            "Lifetime elision: &self lifetime used for output references",
            "Methods returning references use &self's lifetime",
            None,
            0.0,
            &["lifetime", "elision", "self"],
        ),
        rule(
            "LIF-0005",
            DB,
            "static-lifetime",
            Info,
            "'static means lives for entire program; string literals are 'static",
            "Only constants and leaked values are truly 'static",
            None,
            0.0,
            &["lifetime", "static", "string"],
        ),
        rule(
            "LIF-0006",
            DB,
            "hrtb",
            Info,
            "Higher-ranked trait bounds: for<'a> Fn(&'a T)",
            "Function works for any lifetime of the argument",
            None,
            0.0,
            &["lifetime", "hrtb", "higher-ranked"],
        ),
        rule(
            "LIF-0007",
            DB,
            "struct-lifetime",
            Info,
            "Struct holding references needs lifetime parameter",
            "Struct cannot outlive its borrowed fields",
            None,
            0.0,
            &["lifetime", "struct", "parameter"],
        ),
        rule(
            "LIF-0008",
            DB,
            "impl-lifetime",
            Info,
            "impl block may need lifetime parameter if struct has one",
            "impl<'a> Foo<'a> matches struct Foo<'a>",
            None,
            0.0,
            &["lifetime", "impl", "parameter"],
        ),
        rule(
            "LIF-0009",
            DB,
            "closure-lifetime",
            Warning,
            "Closure borrows may require explicit lifetime annotation",
            "Closure captures can create complex lifetime requirements",
            Some("Move values into closure or annotate lifetime"),
            0.60,
            &["lifetime", "closure", "capture"],
        ),
        rule(
            "LIF-0010",
            DB,
            "variance-covariant",
            Info,
            "Covariant lifetime: &'long T can be used where &'short T expected",
            "Subtyping allows shrinking lifetimes",
            None,
            0.0,
            &["lifetime", "variance", "covariant"],
        ),
        rule(
            "LIF-0011",
            DB,
            "variance-contravariant",
            Info,
            "Contravariant lifetime: fn(&'short T) can be used where fn(&'long T) expected",
            "Function parameters are contravariant",
            None,
            0.0,
            &["lifetime", "variance", "contravariant"],
        ),
        rule(
            "LIF-0012",
            DB,
            "variance-invariant",
            Info,
            "&mut T is invariant in T — cannot change the type",
            "Mutable references require exact type match",
            None,
            0.0,
            &["lifetime", "variance", "invariant"],
        ),
        rule(
            "LIF-0013",
            DB,
            "reborrow-lifetime",
            Info,
            "Reborrow creates a shorter-lived reference from &mut",
            "Temporary reborrow allows interleaving uses",
            None,
            0.0,
            &["lifetime", "reborrow"],
        ),
        rule(
            "LIF-0014",
            DB,
            "trait-object-lifetime",
            Warning,
            "Trait objects have implicit lifetime bound",
            "Box<dyn Trait> defaults to Box<dyn Trait + 'static>",
            Some("Add explicit lifetime bound if needed"),
            0.70,
            &["lifetime", "trait-object", "dyn"],
        ),
        rule(
            "LIF-0015",
            DB,
            "lifetime-subtyping",
            Info,
            "'a: 'b means 'a outlives 'b",
            "If 'a outlives 'b, &'a T can be used where &'b T expected",
            None,
            0.0,
            &["lifetime", "subtyping", "outlives"],
        ),
        rule(
            "LIF-0016",
            DB,
            "multiple-lifetimes",
            Info,
            "Functions can have multiple lifetime parameters for different borrows",
            "Separate lifetimes when inputs have different origins",
            None,
            0.0,
            &["lifetime", "multiple", "parameter"],
        ),
        rule(
            "LIF-0017",
            DB,
            "anonymous-lifetime",
            Info,
            "'_ is the anonymous/elided lifetime placeholder",
            "Used in impls and function signatures for readability",
            None,
            0.0,
            &["lifetime", "anonymous", "placeholder"],
        ),
        rule(
            "LIF-0018",
            DB,
            "bound-lifetime",
            Warning,
            "Lifetime bound on generic: T: 'a means T must outlive 'a",
            "Type must not contain references shorter than 'a",
            None,
            0.0,
            &["lifetime", "bound", "generic"],
        ),
        rule(
            "LIF-0019",
            DB,
            "early-vs-late-bound",
            Info,
            "Early-bound lifetimes are set at call site; late-bound during use",
            "Affects when the lifetime is determined",
            None,
            0.0,
            &["lifetime", "early-bound", "late-bound"],
        ),
        rule(
            "LIF-0020",
            DB,
            "nll-two-phase",
            Info,
            "NLL two-phase borrows allow nested method calls",
            "Activation of mutable borrow deferred to first mutation",
            None,
            0.0,
            &["lifetime", "nll", "two-phase"],
        ),
        rule(
            "LIF-0021",
            DB,
            "polonius",
            Info,
            "Polonius is an experimental borrow checker with better precision",
            "May accept programs the current checker rejects",
            None,
            0.0,
            &["lifetime", "polonius", "borrow-checker"],
        ),
        rule(
            "LIF-0022",
            DB,
            "drop-check",
            Warning,
            "Drop-check may require PhantomData to assert ownership",
            "Compiler checks drop doesn't use dangling references",
            None,
            0.0,
            &["lifetime", "drop-check", "phantomdata"],
        ),
        rule(
            "LIF-0023",
            DB,
            "self-referential-pin",
            Info,
            "Pin enables self-referential structures by preventing moves",
            "Required for async state machines",
            None,
            0.0,
            &["lifetime", "self-referential", "pin", "async"],
        ),
        rule(
            "LIF-0024",
            DB,
            "async-lifetime",
            Warning,
            "Async functions capture all parameter lifetimes",
            "Returned Future must satisfy all input lifetimes",
            Some("Use owned values or explicit lifetime bounds"),
            0.60,
            &["lifetime", "async", "future"],
        ),
        rule(
            "LIF-0025",
            DB,
            "stacked-borrows",
            Info,
            "Stacked Borrows: formal model for reference aliasing rules",
            "Miri uses this to detect UB in unsafe code",
            None,
            0.0,
            &["lifetime", "stacked-borrows", "miri"],
        ),
        rule(
            "LIF-0026",
            DB,
            "tree-borrows",
            Info,
            "Tree Borrows: newer alternative model to Stacked Borrows",
            "More permissive; may become the standard model",
            None,
            0.0,
            &["lifetime", "tree-borrows", "model"],
        ),
        rule(
            "LIF-0027",
            DB,
            "ref-counted-lifetime",
            Info,
            "Rc/Arc extend lifetime via reference counting",
            "Value lives until last Rc/Arc is dropped",
            None,
            0.0,
            &["lifetime", "rc", "arc", "refcount"],
        ),
        rule(
            "LIF-0028",
            DB,
            "leaked-lifetime",
            Warning,
            "Box::leak converts owned to &'static",
            "Value lives forever; potential memory leak",
            None,
            0.0,
            &["lifetime", "leak", "static"],
        ),
        rule(
            "LIF-0029",
            DB,
            "scope-guard",
            Info,
            "Scope guards ensure cleanup at end of scope — ties lifetime to scope",
            "RAII pattern: resource released when guard drops",
            None,
            0.0,
            &["lifetime", "scope", "raii", "guard"],
        ),
        rule(
            "LIF-0030",
            DB,
            "temp-lifetime-extension",
            Info,
            "Temporaries bound to let have extended lifetime",
            "let x = &temp extends temp's life to match x",
            None,
            0.0,
            &["lifetime", "temporary", "extension"],
        ),
        rule(
            "LIF-0031",
            DB,
            "type-inference-lifetime",
            Info,
            "Compiler infers lifetimes during type inference",
            "Explicit annotations only needed when ambiguous",
            None,
            0.0,
            &["lifetime", "inference", "type-check"],
        ),
        rule(
            "LIF-0032",
            DB,
            "gat",
            Info,
            "Generic Associated Types allow lifetime-parameterized associated types",
            "GATs enable lending iterators and related patterns",
            None,
            0.0,
            &["lifetime", "gat", "associated-type"],
        ),
        rule(
            "LIF-0033",
            DB,
            "rpitit",
            Info,
            "Return-position impl Trait in trait: returned type captures lifetimes",
            "Returned opaque type may hold borrows from inputs",
            None,
            0.0,
            &["lifetime", "rpitit", "impl-trait"],
        ),
        rule(
            "LIF-0034",
            DB,
            "existential-lifetime",
            Info,
            "impl Trait return types have anonymous existential lifetime",
            "Compiler picks the appropriate lifetime",
            None,
            0.0,
            &["lifetime", "existential", "impl-trait"],
        ),
        rule(
            "LIF-0035",
            DB,
            "send-lifetime",
            Warning,
            "Values sent across threads must satisfy 'static or explicit lifetime constraints",
            "Thread may outlive the sender's scope",
            Some("Use owned values or Arc for shared data"),
            0.75,
            &["lifetime", "send", "thread"],
        ),
    ]
}

fn type_safety_rules() -> Vec<Rule> {
    use RuleDatabase::TypeSafety as DB;
    use RuleSeverity::*;
    vec![
        rule(
            "TYP-0001",
            DB,
            "type-mismatch",
            Error,
            "Expected type A, found type B",
            "Type system enforces correct usage",
            Some("Cast, convert, or fix the producing expression"),
            0.80,
            &["type", "mismatch"],
        ),
        rule(
            "TYP-0002",
            DB,
            "narrowing-cast",
            Warning,
            "Narrowing cast may lose data (e.g., i64 → i32)",
            "Target type cannot represent all source values",
            Some("Use try_into() to handle overflow"),
            0.85,
            &["type", "cast", "narrowing", "overflow"],
        ),
        rule(
            "TYP-0003",
            DB,
            "unsound-transmute",
            Error,
            "mem::transmute between incompatible types",
            "Types must have same size and compatible layout",
            None,
            0.0,
            &["type", "transmute", "unsound", "unsafe"],
        ),
        rule(
            "TYP-0004",
            DB,
            "uninit-read",
            Error,
            "Reading from uninitialized memory",
            "Uninitialized values are undefined behavior",
            Some("Initialize the variable before use"),
            0.90,
            &["type", "uninit", "ub"],
        ),
        rule(
            "TYP-0005",
            DB,
            "missing-trait-impl",
            Error,
            "Required trait not implemented for type",
            "Operation requires trait bound that type doesn't satisfy",
            Some("Implement the trait or change the type"),
            0.70,
            &["type", "trait", "bound", "missing"],
        ),
        rule(
            "TYP-0006",
            DB,
            "exhaustive-match",
            Error,
            "Match expression not exhaustive",
            "All possible values must be handled",
            Some("Add missing arms or use _ wildcard"),
            0.90,
            &["type", "match", "exhaustive"],
        ),
        rule(
            "TYP-0007",
            DB,
            "unreachable-pattern",
            Warning,
            "Pattern is unreachable — covered by earlier arm",
            "Redundant arm; may indicate logic error",
            Some("Remove unreachable arm"),
            0.90,
            &["type", "match", "unreachable", "lint"],
        ),
        rule(
            "TYP-0008",
            DB,
            "never-type",
            Info,
            "! type indicates computation never completes normally",
            "Used for diverging functions (panic, loop forever)",
            None,
            0.0,
            &["type", "never", "diverge"],
        ),
        rule(
            "TYP-0009",
            DB,
            "integer-overflow",
            Warning,
            "Integer overflow in debug mode panics, wraps in release",
            "Implicit overflow is usually a bug",
            Some("Use wrapping_add() or checked_add()"),
            0.75,
            &["type", "integer", "overflow"],
        ),
        rule(
            "TYP-0010",
            DB,
            "float-equality",
            Warning,
            "Floating-point equality comparison may be imprecise",
            "f64 == f64 can give unexpected results",
            Some("Use (a - b).abs() < epsilon"),
            0.80,
            &["type", "float", "equality", "comparison"],
        ),
        rule(
            "TYP-0011",
            DB,
            "option-unwrap",
            Warning,
            "unwrap() on Option panics if None",
            "Use match, if-let, or unwrap_or_else instead",
            Some("Replace with unwrap_or_default() or ?"),
            0.85,
            &["type", "option", "unwrap", "panic"],
        ),
        rule(
            "TYP-0012",
            DB,
            "result-unwrap",
            Warning,
            "unwrap() on Result panics if Err",
            "Handle errors instead of panicking",
            Some("Replace with ? operator or match"),
            0.85,
            &["type", "result", "unwrap", "panic"],
        ),
        rule(
            "TYP-0013",
            DB,
            "infallible-conversion",
            Info,
            "From/Into conversions that cannot fail",
            "Use .into() for infallible conversions",
            None,
            0.0,
            &["type", "conversion", "infallible"],
        ),
        rule(
            "TYP-0014",
            DB,
            "fallible-conversion",
            Info,
            "TryFrom/TryInto conversions that may fail",
            "Returns Result; must handle errors",
            None,
            0.0,
            &["type", "conversion", "fallible"],
        ),
        rule(
            "TYP-0015",
            DB,
            "coercion-deref",
            Info,
            "&String coerces to &str; &Vec<T> to &[T]",
            "Deref coercion simplifies API boundaries",
            None,
            0.0,
            &["type", "coercion", "deref"],
        ),
        rule(
            "TYP-0016",
            DB,
            "turbofish",
            Hint,
            "Use ::<Type> (turbofish) when type inference is ambiguous",
            "Explicit type annotation resolves ambiguity",
            Some("Add ::<ConcreteType> to the call"),
            0.80,
            &["type", "turbofish", "inference"],
        ),
        rule(
            "TYP-0017",
            DB,
            "phantom-variance",
            Info,
            "PhantomData<T> controls variance without storing T",
            "Used to influence type relationships",
            None,
            0.0,
            &["type", "phantomdata", "variance"],
        ),
        rule(
            "TYP-0018",
            DB,
            "unsized-type",
            Info,
            "dyn Trait, str, [T] are unsized; need indirection",
            "Must be behind pointer: &dyn Trait, &str, &[T], Box<dyn Trait>",
            None,
            0.0,
            &["type", "unsized", "dyn", "slice"],
        ),
        rule(
            "TYP-0019",
            DB,
            "orphan-rule",
            Error,
            "Cannot implement foreign trait on foreign type",
            "Orphan rule prevents conflicting impls across crates",
            Some("Create a newtype wrapper"),
            0.70,
            &["type", "orphan", "trait", "impl"],
        ),
        rule(
            "TYP-0020",
            DB,
            "blanket-impl",
            Info,
            "impl<T: Trait> OtherTrait for T — applies to all T satisfying Trait",
            "Blanket implementations provide automatic impls",
            None,
            0.0,
            &["type", "blanket", "impl", "generic"],
        ),
        rule(
            "TYP-0021",
            DB,
            "coherence",
            Error,
            "Overlapping trait implementations detected",
            "Only one impl can apply for any given type",
            Some("Specialize or use different types"),
            0.60,
            &["type", "coherence", "overlap"],
        ),
        rule(
            "TYP-0022",
            DB,
            "type-alias-opaque",
            Info,
            "type Foo = Bar creates a transparent alias; impl Trait is opaque",
            "Aliases are structurally identical; opaque types are abstract",
            None,
            0.0,
            &["type", "alias", "opaque"],
        ),
        rule(
            "TYP-0023",
            DB,
            "zero-sized",
            Info,
            "Zero-sized types (ZSTs) occupy no memory",
            "() and PhantomData are ZSTs; free at runtime",
            None,
            0.0,
            &["type", "zst", "zero-sized"],
        ),
        rule(
            "TYP-0024",
            DB,
            "enum-layout",
            Info,
            "Enums store the largest variant + tag",
            "Size equals max(variant sizes) + discriminant",
            None,
            0.0,
            &["type", "enum", "layout", "size"],
        ),
        rule(
            "TYP-0025",
            DB,
            "niche-optimization",
            Info,
            "Option<&T> has same size as &T — null niche fills None",
            "Compiler exploits impossible values for tag-free enums",
            None,
            0.0,
            &["type", "option", "niche", "optimization"],
        ),
        rule(
            "TYP-0026",
            DB,
            "repr-c",
            Info,
            "#[repr(C)] ensures C-compatible struct layout",
            "Required for FFI structs",
            None,
            0.0,
            &["type", "repr", "c-layout", "ffi"],
        ),
        rule(
            "TYP-0027",
            DB,
            "repr-transparent",
            Info,
            "#[repr(transparent)] makes newtype same as inner type",
            "Single-field struct has identical ABI to field",
            None,
            0.0,
            &["type", "repr", "transparent", "newtype"],
        ),
        rule(
            "TYP-0028",
            DB,
            "recursive-type",
            Error,
            "Recursive type has infinite size",
            "Must use indirection: Box, Rc, or reference",
            Some("Wrap recursive field in Box<T>"),
            0.90,
            &["type", "recursive", "infinite", "box"],
        ),
        rule(
            "TYP-0029",
            DB,
            "downcasting",
            Warning,
            "Downcast trait object to concrete type may fail",
            "Use Any::downcast_ref for runtime type checking",
            Some("Use downcast_ref::<T>()"),
            0.70,
            &["type", "downcast", "trait-object", "any"],
        ),
        rule(
            "TYP-0030",
            DB,
            "clone-vs-copy",
            Info,
            "Copy is bitwise clone; Clone may be expensive",
            "Copy is implicit; Clone requires explicit .clone()",
            None,
            0.0,
            &["type", "copy", "clone", "semantics"],
        ),
        rule(
            "TYP-0031",
            DB,
            "sized-bound",
            Info,
            "T: Sized is the default bound; opt out with T: ?Sized",
            "Most generics require known size at compile time",
            None,
            0.0,
            &["type", "sized", "bound", "generic"],
        ),
        rule(
            "TYP-0032",
            DB,
            "associated-type",
            Info,
            "Associated types are fixed per impl; generic params vary per call",
            "Use associated types when there's one natural choice",
            None,
            0.0,
            &["type", "associated", "generic", "trait"],
        ),
        rule(
            "TYP-0033",
            DB,
            "newtype-pattern",
            Hint,
            "Newtype pattern wraps a type for type safety or trait impl",
            "struct Meters(f64) prevents mixing with Seconds(f64)",
            Some("Create newtype wrapper struct"),
            0.80,
            &["type", "newtype", "pattern", "safety"],
        ),
        rule(
            "TYP-0034",
            DB,
            "builder-pattern",
            Hint,
            "Builder pattern for complex struct construction",
            "Fluent API: builder.field(v).field2(v2).build()",
            None,
            0.0,
            &["type", "builder", "pattern", "construction"],
        ),
        rule(
            "TYP-0035",
            DB,
            "from-error",
            Hint,
            "Implement From<SubError> for Error to enable ? operator",
            "Automatic error conversion via From trait",
            Some("impl From<SubError> for Error {...}"),
            0.85,
            &["type", "error", "from", "conversion"],
        ),
        rule(
            "TYP-0036",
            DB,
            "never-constructed",
            Warning,
            "Enum variant never constructed",
            "Dead code; may indicate incomplete implementation",
            Some("Remove unused variant or implement construction"),
            0.70,
            &["type", "enum", "dead-code"],
        ),
        rule(
            "TYP-0037",
            DB,
            "must-use",
            Warning,
            "#[must_use] result ignored",
            "Ignoring Result/Option silently discards errors",
            Some("Handle the return value or explicitly ignore with let _ ="),
            0.90,
            &["type", "must-use", "result", "lint"],
        ),
        rule(
            "TYP-0038",
            DB,
            "display-debug",
            Hint,
            "Implement Display for user-facing output; Debug for developer",
            "Display for {}, Debug for {:?}",
            Some("impl fmt::Display for Type {...}"),
            0.75,
            &["type", "display", "debug", "fmt"],
        ),
        rule(
            "TYP-0039",
            DB,
            "as-cast-truncation",
            Warning,
            "as cast truncates silently — use try_into for safe conversion",
            "i64 as i32 wraps without warning",
            Some("Replace with x.try_into().unwrap_or(default)"),
            0.80,
            &["type", "cast", "truncation", "as"],
        ),
        rule(
            "TYP-0040",
            DB,
            "const-generics",
            Info,
            "Const generics parameterize types by compile-time values",
            "Array<T, N> where const N: usize",
            None,
            0.0,
            &["type", "const-generics", "parameter"],
        ),
    ]
}

fn concurrency_rules() -> Vec<Rule> {
    use RuleDatabase::Concurrency as DB;
    use RuleSeverity::*;
    vec![
        rule(
            "CON-0001",
            DB,
            "data-race",
            Error,
            "Potential data race: shared mutable state across threads",
            "Undefined behavior in Rust/MechGen",
            Some("Wrap in Mutex, RwLock, or use Atomic types"),
            0.80,
            &["concurrency", "data-race", "thread"],
        ),
        rule(
            "CON-0002",
            DB,
            "deadlock",
            Warning,
            "Potential deadlock: lock ordering violation",
            "Two locks acquired in different orders across threads",
            Some("Establish consistent lock ordering"),
            0.55,
            &["concurrency", "deadlock", "lock", "ordering"],
        ),
        rule(
            "CON-0003",
            DB,
            "send-violation",
            Error,
            "Type not Send — cannot be transferred between threads",
            "Type contains non-thread-safe internals (Rc, raw pointers)",
            Some("Use Arc instead of Rc; ensure all fields are Send"),
            0.80,
            &["concurrency", "send", "thread", "transfer"],
        ),
        rule(
            "CON-0004",
            DB,
            "sync-violation",
            Error,
            "Type not Sync — cannot be shared between threads via &",
            "Type allows mutation through shared reference without synchronization",
            Some("Wrap in Mutex for synchronized access"),
            0.80,
            &["concurrency", "sync", "shared", "thread"],
        ),
        rule(
            "CON-0005",
            DB,
            "mutex-poison",
            Warning,
            "Mutex may be poisoned after panic in another thread",
            "lock() returns Err if holder panicked",
            Some("Use lock().unwrap_or_else(|e| e.into_inner())"),
            0.75,
            &["concurrency", "mutex", "poison", "panic"],
        ),
        rule(
            "CON-0006",
            DB,
            "rwlock-writer-starvation",
            Info,
            "RwLock may cause writer starvation under heavy read load",
            "Many readers can block writers indefinitely",
            None,
            0.0,
            &["concurrency", "rwlock", "starvation"],
        ),
        rule(
            "CON-0007",
            DB,
            "channel-disconnect",
            Warning,
            "Channel recv() may fail if all senders dropped",
            "Disconnected channel returns RecvError",
            Some("Handle Err case from recv()"),
            0.80,
            &["concurrency", "channel", "disconnect"],
        ),
        rule(
            "CON-0008",
            DB,
            "arc-mutex-pattern",
            Info,
            "Arc<Mutex<T>> is the standard shared mutable state pattern",
            "Arc for shared ownership, Mutex for interior mutability",
            None,
            0.0,
            &["concurrency", "arc", "mutex", "pattern"],
        ),
        rule(
            "CON-0009",
            DB,
            "atomic-ordering",
            Warning,
            "Incorrect atomic memory ordering may cause subtle bugs",
            "Use SeqCst when unsure; Relaxed only for counters",
            Some("Use Ordering::SeqCst for correctness"),
            0.60,
            &["concurrency", "atomic", "ordering", "memory"],
        ),
        rule(
            "CON-0010",
            DB,
            "thread-spawn-lifetime",
            Warning,
            "Spawned thread requires 'static or scoped thread",
            "std::thread::spawn requires 'static closure",
            Some("Use std::thread::scope for non-'static references"),
            0.80,
            &["concurrency", "thread", "spawn", "lifetime"],
        ),
        rule(
            "CON-0011",
            DB,
            "tokio-runtime-block",
            Error,
            "Blocking in async runtime blocks the executor thread",
            "Use spawn_blocking() for blocking operations",
            Some("Move blocking code to spawn_blocking()"),
            0.85,
            &["concurrency", "async", "blocking", "tokio"],
        ),
        rule(
            "CON-0012",
            DB,
            "async-mutex",
            Warning,
            "Using std::sync::Mutex in async; prefer tokio::sync::Mutex",
            "std Mutex blocks the thread; tokio Mutex yields",
            Some("Replace with tokio::sync::Mutex"),
            0.80,
            &["concurrency", "async", "mutex", "tokio"],
        ),
        rule(
            "CON-0013",
            DB,
            "select-fairness",
            Info,
            "tokio::select! is not fair — first ready branch wins",
            "Under contention, some branches may starve",
            None,
            0.0,
            &["concurrency", "async", "select", "fairness"],
        ),
        rule(
            "CON-0014",
            DB,
            "cancellation-safety",
            Warning,
            "Async operation not cancellation-safe in select!",
            "Partial progress lost when branch is cancelled",
            Some("Complete the future before selecting"),
            0.60,
            &["concurrency", "async", "cancellation", "select"],
        ),
        rule(
            "CON-0015",
            DB,
            "rayon-par-iter",
            Info,
            "Rayon parallel iterators require Send + Sync bounds",
            "Data must be safely shareable for parallelism",
            None,
            0.0,
            &["concurrency", "rayon", "parallel", "iterator"],
        ),
        rule(
            "CON-0016",
            DB,
            "global-state",
            Warning,
            "Global mutable state requires synchronization",
            "static mut is unsafe; use Mutex<T> or atomics",
            Some("Use std::sync::OnceLock or Lazy for initialization"),
            0.80,
            &["concurrency", "global", "static", "mutex"],
        ),
        rule(
            "CON-0017",
            DB,
            "crossbeam-scope",
            Hint,
            "crossbeam::scope allows non-'static references in threads",
            "Scoped threads join before scope ends, ensuring lifetimes",
            Some("Use crossbeam::scope for scoped parallelism"),
            0.70,
            &["concurrency", "crossbeam", "scope", "thread"],
        ),
        rule(
            "CON-0018",
            DB,
            "once-cell",
            Hint,
            "Use OnceLock/LazyLock for thread-safe lazy initialization",
            "Initialized at most once; safe to share across threads",
            Some("Replace manual init with OnceLock::new()"),
            0.85,
            &["concurrency", "once-cell", "lazy", "init"],
        ),
        rule(
            "CON-0019",
            DB,
            "barrier",
            Info,
            "Barrier synchronizes N threads at a rendezvous point",
            "All threads wait until the last one arrives",
            None,
            0.0,
            &["concurrency", "barrier", "synchronization"],
        ),
        rule(
            "CON-0020",
            DB,
            "condvar",
            Info,
            "Condvar allows waiting for a condition with a Mutex",
            "Use with Mutex for efficient event waiting",
            None,
            0.0,
            &["concurrency", "condvar", "wait", "mutex"],
        ),
        rule(
            "CON-0021",
            DB,
            "parking-lot",
            Hint,
            "Consider parking_lot::Mutex for better performance",
            "Faster than std Mutex in many workloads",
            Some("Replace std::sync::Mutex with parking_lot::Mutex"),
            0.65,
            &["concurrency", "parking-lot", "mutex", "performance"],
        ),
        rule(
            "CON-0022",
            DB,
            "work-stealing",
            Info,
            "Work-stealing scheduler distributes tasks across threads",
            "Rayon and Tokio use work-stealing for load balancing",
            None,
            0.0,
            &["concurrency", "work-stealing", "scheduler"],
        ),
        rule(
            "CON-0023",
            DB,
            "pin-future",
            Info,
            "Futures must be Pinned before polling",
            "Async state machines contain self-references",
            None,
            0.0,
            &["concurrency", "async", "pin", "future"],
        ),
        rule(
            "CON-0024",
            DB,
            "send-future",
            Warning,
            "Future not Send — cannot be spawned on multi-threaded runtime",
            "Future holds non-Send types across await",
            Some("Clone data before await or use single-threaded runtime"),
            0.70,
            &["concurrency", "async", "future", "send"],
        ),
        rule(
            "CON-0025",
            DB,
            "unbounded-channel",
            Warning,
            "Unbounded channel can grow without limit",
            "Memory usage grows if consumer is slower than producer",
            Some("Use bounded channel with backpressure"),
            0.65,
            &["concurrency", "channel", "unbounded", "memory"],
        ),
        rule(
            "CON-0026",
            DB,
            "actor-model",
            Info,
            "Actor model: each actor has private state, communicates via messages",
            "Avoids shared state; good for concurrency",
            None,
            0.0,
            &["concurrency", "actor", "model", "pattern"],
        ),
        rule(
            "CON-0027",
            DB,
            "semaphore",
            Info,
            "Semaphore limits concurrent access to a resource",
            "Allows N concurrent holders",
            None,
            0.0,
            &["concurrency", "semaphore", "limit"],
        ),
        rule(
            "CON-0028",
            DB,
            "lock-free",
            Info,
            "Lock-free data structures use atomics instead of locks",
            "Avoids blocking but complex to implement correctly",
            None,
            0.0,
            &["concurrency", "lock-free", "atomic"],
        ),
        rule(
            "CON-0029",
            DB,
            "rwlock-deadlock-recursive",
            Warning,
            "RwLock read lock inside write lock causes deadlock on some platforms",
            "Re-entrant read inside write blocks forever",
            Some("Avoid acquiring read lock while holding write lock"),
            0.70,
            &["concurrency", "rwlock", "deadlock", "recursive"],
        ),
        rule(
            "CON-0030",
            DB,
            "thread-local",
            Info,
            "thread_local! provides per-thread storage without synchronization",
            "Each thread has its own copy",
            None,
            0.0,
            &["concurrency", "thread-local", "storage"],
        ),
        rule(
            "CON-0031",
            DB,
            "volatile",
            Info,
            "Volatile access for memory-mapped I/O: std::ptr::read_volatile",
            "Prevents compiler from optimizing away reads/writes",
            None,
            0.0,
            &["concurrency", "volatile", "mmio", "unsafe"],
        ),
        rule(
            "CON-0032",
            DB,
            "spinlock",
            Warning,
            "Spinlock wastes CPU cycles; prefer Mutex for general use",
            "Spinning is only appropriate for very short critical sections",
            Some("Replace with std::sync::Mutex"),
            0.70,
            &["concurrency", "spinlock", "performance"],
        ),
        rule(
            "CON-0033",
            DB,
            "task-spawn-cost",
            Info,
            "Async task spawn is cheaper than OS thread spawn",
            "Use tasks for fine-grained concurrency",
            None,
            0.0,
            &["concurrency", "async", "task", "spawn", "cost"],
        ),
        rule(
            "CON-0034",
            DB,
            "structured-concurrency",
            Hint,
            "Prefer structured concurrency: spawn tasks in a scope",
            "JoinSet/TaskGroup ensure all tasks complete before proceeding",
            Some("Use JoinSet to manage spawned tasks"),
            0.70,
            &["concurrency", "structured", "joinset", "scope"],
        ),
        rule(
            "CON-0035",
            DB,
            "priority-inversion",
            Info,
            "Priority inversion: high-priority thread waits on low-priority holding lock",
            "OS scheduling anomaly with locks",
            None,
            0.0,
            &["concurrency", "priority", "inversion", "scheduling"],
        ),
    ]
}

fn ffi_rules() -> Vec<Rule> {
    use RuleDatabase::FFI as DB;
    use RuleSeverity::*;
    vec![
        rule(
            "FFI-0001",
            DB,
            "null-pointer-deref",
            Error,
            "Potential null pointer dereference from foreign function",
            "C functions may return null; must check before deref",
            Some("Check for null before dereferencing"),
            0.90,
            &["ffi", "null", "pointer", "deref"],
        ),
        rule(
            "FFI-0002",
            DB,
            "layout-mismatch",
            Error,
            "Struct layout mismatch between MechGen and foreign type",
            "ABI incompatibility; use #[repr(C)]",
            Some("Add #[repr(C)] to struct definition"),
            0.85,
            &["ffi", "layout", "repr-c", "abi"],
        ),
        rule(
            "FFI-0003",
            DB,
            "missing-free",
            Error,
            "Foreign-allocated memory not freed",
            "C allocations need explicit free(); Rust Drop won't fire",
            Some("Call the appropriate free function"),
            0.80,
            &["ffi", "free", "leak", "memory"],
        ),
        rule(
            "FFI-0004",
            DB,
            "string-nul",
            Error,
            "C string missing NUL terminator",
            "C expects NUL-terminated strings",
            Some("Use CString or add \\0"),
            0.90,
            &["ffi", "string", "nul", "cstring"],
        ),
        rule(
            "FFI-0005",
            DB,
            "string-utf8",
            Warning,
            "Foreign string may not be valid UTF-8",
            "Rust strings require valid UTF-8",
            Some("Use CStr::to_string_lossy() or validate"),
            0.80,
            &["ffi", "string", "utf8", "encoding"],
        ),
        rule(
            "FFI-0006",
            DB,
            "extern-abi",
            Warning,
            "Function missing extern \"C\" ABI declaration",
            "Default Rust ABI is incompatible with C",
            Some("Add extern \"C\" to function signature"),
            0.90,
            &["ffi", "extern", "abi", "c"],
        ),
        rule(
            "FFI-0007",
            DB,
            "opaque-pointer",
            Info,
            "Opaque foreign types should be modeled as extern type or non-constructable struct",
            "Prevents accidental construction or size assumptions",
            None,
            0.0,
            &["ffi", "opaque", "extern-type"],
        ),
        rule(
            "FFI-0008",
            DB,
            "callback-panic",
            Error,
            "Panic across FFI boundary is undefined behavior",
            "Panics must be caught before returning to C",
            Some("Use catch_unwind at FFI boundary"),
            0.85,
            &["ffi", "callback", "panic", "unwind"],
        ),
        rule(
            "FFI-0009",
            DB,
            "alignment",
            Warning,
            "Type alignment may differ between Rust and C",
            "Misaligned access is UB on some platforms",
            Some("Verify alignment with #[repr(C, align(N))]"),
            0.70,
            &["ffi", "alignment", "repr", "ub"],
        ),
        rule(
            "FFI-0010",
            DB,
            "borrow-across-ffi",
            Warning,
            "Reference passed to C code — aliasing rules cannot be enforced",
            "C code may alias or store the pointer beyond borrow scope",
            Some("Pass raw pointer instead if C stores it"),
            0.60,
            &["ffi", "borrow", "pointer", "aliasing"],
        ),
        rule(
            "FFI-0011",
            DB,
            "enum-repr",
            Error,
            "Enum without defined repr has unstable discriminant values",
            "FFI enums need #[repr(C)] or #[repr(i32)]",
            Some("Add #[repr(C)] to the enum"),
            0.90,
            &["ffi", "enum", "repr", "discriminant"],
        ),
        rule(
            "FFI-0012",
            DB,
            "bool-repr",
            Warning,
            "Rust bool is 1 byte; C _Bool may differ",
            "Use c_int or c_char for FFI booleans",
            Some("Replace bool with c_int in FFI signatures"),
            0.75,
            &["ffi", "bool", "size", "c"],
        ),
        rule(
            "FFI-0013",
            DB,
            "size-t-usize",
            Info,
            "C size_t maps to Rust usize; ensure target compatibility",
            "Both are pointer-sized on the target platform",
            None,
            0.0,
            &["ffi", "size_t", "usize", "portability"],
        ),
        rule(
            "FFI-0014",
            DB,
            "variadic-unsafe",
            Warning,
            "Variadic C functions are inherently unsafe",
            "No type checking on variadic arguments",
            None,
            0.0,
            &["ffi", "variadic", "unsafe", "printf"],
        ),
        rule(
            "FFI-0015",
            DB,
            "global-c-state",
            Warning,
            "C library may have global mutable state",
            "Thread-safety depends on the C library's guarantees",
            Some("Synchronize access or verify C library thread-safety"),
            0.50,
            &["ffi", "global", "state", "thread-safety"],
        ),
        rule(
            "FFI-0016",
            DB,
            "wasm-import",
            Info,
            "WASM imports use extern \"C\" with #[link(wasm_import_module)]",
            "WebAssembly host functions require specific linking",
            None,
            0.0,
            &["ffi", "wasm", "import", "link"],
        ),
        rule(
            "FFI-0017",
            DB,
            "bindgen",
            Hint,
            "Use bindgen to auto-generate FFI bindings from C headers",
            "Reduces manual binding errors",
            Some("Run `bindgen header.h -o bindings.rs`"),
            0.85,
            &["ffi", "bindgen", "header", "autogenerate"],
        ),
        rule(
            "FFI-0018",
            DB,
            "cbindgen",
            Hint,
            "Use cbindgen to generate C headers from Rust code",
            "Ensures C consumers have correct declarations",
            Some("Run `cbindgen --config cbindgen.toml --output header.h`"),
            0.85,
            &["ffi", "cbindgen", "header", "export"],
        ),
        rule(
            "FFI-0019",
            DB,
            "pin-ffi",
            Warning,
            "Pinned types should not be passed across FFI — C doesn't understand Pin",
            "C code may move the value",
            Some("Unpin before passing to C, or use raw pointer"),
            0.60,
            &["ffi", "pin", "move", "safety"],
        ),
        rule(
            "FFI-0020",
            DB,
            "error-code",
            Hint,
            "Use explicit error codes instead of Result across FFI",
            "C doesn't have Result; use int return + out-parameter",
            Some("Return c_int error code and pass result via out-pointer"),
            0.80,
            &["ffi", "error", "return-code", "c"],
        ),
    ]
}

// ── Agent Elision Rules (AEL-0001 .. AEL-0030) ─────────────────────
// These rules describe safety constructs the compiler handles automatically
// in agent mode, allowing their syntax to be elided from the language.

fn agent_elision_rules() -> Vec<Rule> {
    use RuleDatabase::AgentElision as DB;
    use RuleSeverity::*;
    vec![
        rule(
            "AEL-0001",
            DB,
            "unsafe-block-elision",
            Info,
            "Compiler infers unsafe boundaries from SKB rules in agent mode",
            "Agent mode elides `unsafe` blocks; the compiler uses OWN/BOR/FFI rules to verify safety",
            None,
            1.0,
            &["agent-elision", "unsafe", "safety"],
        ),
        rule(
            "AEL-0002",
            DB,
            "unsafe-fn-elision",
            Info,
            "Compiler marks functions as unsafe internally based on body analysis",
            "Agent mode does not require `unsafe fn` — the compiler detects FFI calls, raw pointer ops, and union access",
            None,
            1.0,
            &["agent-elision", "unsafe", "function"],
        ),
        rule(
            "AEL-0003",
            DB,
            "lifetime-annotation-elision",
            Info,
            "Compiler infers all lifetime annotations in agent mode",
            "Explicit lifetime annotations are elided; the compiler's LIF rules resolve all borrows",
            None,
            1.0,
            &["agent-elision", "lifetime", "annotation"],
        ),
        rule(
            "AEL-0004",
            DB,
            "mutability-inference",
            Info,
            "Compiler infers &mut from usage context in agent mode",
            "&T vs &mut T is determined by the compiler from write operations on the reference",
            None,
            1.0,
            &["agent-elision", "mutability", "reference"],
        ),
        rule(
            "AEL-0005",
            DB,
            "send-sync-inference",
            Info,
            "Compiler derives Send/Sync bounds automatically in agent mode",
            "Send and Sync bounds are inferred from type structure; no explicit annotations needed",
            None,
            0.95,
            &["agent-elision", "send", "sync", "concurrency"],
        ),
        rule(
            "AEL-0006",
            DB,
            "closure-capture-inference",
            Info,
            "Compiler infers move vs borrow capture for closures in agent mode",
            "The `move` keyword is unnecessary; compiler determines capture mode from usage",
            None,
            1.0,
            &["agent-elision", "closure", "move", "capture"],
        ),
        rule(
            "AEL-0007",
            DB,
            "pin-inference",
            Info,
            "Compiler handles Pin<T> wrapping for self-referential types",
            "Pin requirements are detected and enforced without explicit syntax",
            None,
            0.90,
            &["agent-elision", "pin", "self-referential"],
        ),
        rule(
            "AEL-0008",
            DB,
            "dyn-impl-inference",
            Info,
            "Compiler chooses between static and dynamic dispatch automatically",
            "No `dyn`/`impl` distinction needed — compiler selects based on call site analysis",
            None,
            0.90,
            &["agent-elision", "dispatch", "trait-object"],
        ),
        rule(
            "AEL-0009",
            DB,
            "borrow-check-relaxation",
            Info,
            "Borrow checker runs post-hoc in agent mode, not as syntax constraint",
            "Agent code is accepted without explicit borrow annotations; violations detected as compiler diagnostics",
            None,
            1.0,
            &["agent-elision", "borrow", "check"],
        ),
        rule(
            "AEL-0010",
            DB,
            "bounds-check-elision",
            Info,
            "Runtime bounds checks may be elided when compiler proves safety",
            "Index operations skip bounds checks when the compiler can prove the index is in range",
            Some("Compiler inserts `unreachable_unchecked()` for proven bounds"),
            0.85,
            &["agent-elision", "bounds", "performance"],
        ),
        rule(
            "AEL-0011",
            DB,
            "overflow-check-elision",
            Info,
            "Arithmetic overflow checks elided when compiler proves no overflow",
            "Agent mode skips overflow checks for operations proved to be in range",
            None,
            0.80,
            &["agent-elision", "overflow", "arithmetic"],
        ),
        rule(
            "AEL-0012",
            DB,
            "phantom-data-elision",
            Info,
            "PhantomData<T> fields inserted automatically by compiler",
            "Variance and drop-check markers handled internally without explicit PhantomData",
            None,
            0.95,
            &["agent-elision", "phantom", "variance"],
        ),
        rule(
            "AEL-0013",
            DB,
            "async-await-compression",
            Info,
            "`.await` compressed to `.w` in agent mode syntax",
            "Agent mode minimizes token count; `.w` is a 4-character saving per await point",
            None,
            1.0,
            &["agent-elision", "async", "await", "compression"],
        ),
        rule(
            "AEL-0014",
            DB,
            "return-compression",
            Info,
            "`return` compressed to `ret` in agent mode syntax",
            "Agent mode uses `ret` for 3-character saving per return statement",
            None,
            1.0,
            &["agent-elision", "return", "compression"],
        ),
        rule(
            "AEL-0015",
            DB,
            "effect-compression",
            Info,
            "`effect` compressed to `fx` in agent mode syntax",
            "Agent mode uses `fx` for 4-character saving per effect declaration",
            None,
            1.0,
            &["agent-elision", "effect", "compression"],
        ),
        rule(
            "AEL-0016",
            DB,
            "handle-compression",
            Info,
            "`handle` compressed to `hx` in agent mode syntax",
            "Agent mode uses `hx` for 4-character saving per handler",
            None,
            1.0,
            &["agent-elision", "handle", "compression"],
        ),
        rule(
            "AEL-0017",
            DB,
            "spec-compression",
            Info,
            "`spec` compressed to `sp` in agent mode syntax",
            "Agent mode uses `sp` for 2-character saving per spec block",
            None,
            1.0,
            &["agent-elision", "spec", "compression"],
        ),
        rule(
            "AEL-0018",
            DB,
            "extern-compression",
            Info,
            "`extern` compressed to `xn` in agent mode syntax",
            "Agent mode uses `xn` for 4-character saving per extern block",
            None,
            1.0,
            &["agent-elision", "extern", "ffi", "compression"],
        ),
        rule(
            "AEL-0019",
            DB,
            "yield-compression",
            Info,
            "`yield` compressed to `yl` in agent mode syntax",
            "Agent mode uses `yl` for 3-character saving per yield expression",
            None,
            1.0,
            &["agent-elision", "yield", "compression"],
        ),
        rule(
            "AEL-0020",
            DB,
            "unsafe-skb-coverage",
            Warning,
            "Unsafe op detected — SKB rules OWN/BOR/FFI provide compile-time verification",
            "The compiler cross-references unsafe operations against SKB rules to verify correctness without language-level `unsafe` syntax",
            Some("Verify operation is covered by existing SKB rule; if not, add a new AEL rule"),
            0.90,
            &["agent-elision", "unsafe", "verification"],
        ),
        rule(
            "AEL-0021",
            DB,
            "raw-pointer-inference",
            Info,
            "Raw pointer operations (*const T, *mut T) verified via SKB without `unsafe` block",
            "The compiler detects raw pointer dereferences and verifies alignment, validity, and aliasing via FFI and OWN rules",
            None,
            0.85,
            &["agent-elision", "raw-pointer", "unsafe"],
        ),
        rule(
            "AEL-0022",
            DB,
            "union-access-inference",
            Info,
            "Union field access verified via type rules without `unsafe` block",
            "The compiler ensures discriminant is checked before union field access",
            None,
            0.85,
            &["agent-elision", "union", "unsafe"],
        ),
        rule(
            "AEL-0023",
            DB,
            "global-mut-inference",
            Info,
            "Mutable static access verified via concurrency rules without `unsafe` block",
            "The compiler ensures mutable statics are only accessed under proper synchronization",
            None,
            0.80,
            &["agent-elision", "static-mut", "concurrency"],
        ),
        rule(
            "AEL-0024",
            DB,
            "extern-fn-inference",
            Info,
            "Extern function calls verified via FFI rules without `unsafe` block",
            "The compiler validates argument types, nullable pointers, and calling conventions for extern calls",
            None,
            0.90,
            &["agent-elision", "extern", "ffi", "unsafe"],
        ),
        rule(
            "AEL-0025",
            DB,
            "transmute-inference",
            Info,
            "Transmute operations verified via type layout rules without `unsafe` block",
            "The compiler ensures size, alignment, and validity invariants hold for transmutations",
            Some("Verify source and target types have compatible layouts"),
            0.75,
            &["agent-elision", "transmute", "unsafe", "layout"],
        ),
        rule(
            "AEL-0026",
            DB,
            "inline-asm-inference",
            Info,
            "Inline assembly verified via platform rules without `unsafe` block",
            "The compiler validates register constraints and side effects for inline asm",
            None,
            0.70,
            &["agent-elision", "asm", "unsafe", "platform"],
        ),
        rule(
            "AEL-0027",
            DB,
            "ai-construct-compression",
            Info,
            "AI construct keywords compressed to Greek symbols in agent mode",
            "net→Ψ, layer→λ, tensor→Φ, train→Θ, agent→α, kb→κ, evolve→Ω enable 3-5× density",
            None,
            1.0,
            &["agent-elision", "ai", "compression", "greek"],
        ),
        rule(
            "AEL-0028",
            DB,
            "type-sigil-compression",
            Info,
            "Type constructors compressed to sigils in agent mode",
            "Box→^, Rc→$, Arc→@, Mutex→#, Vec→[]~, Option→?, Result→R[] enable compact type expressions",
            None,
            1.0,
            &["agent-elision", "type", "compression", "sigil"],
        ),
        rule(
            "AEL-0029",
            DB,
            "control-flow-compression",
            Info,
            "Control flow keywords compressed to operators in agent mode",
            "if→?, else→:, for→@, loop→@@, break→!, continue→>>, match→?= minimize control flow tokens",
            None,
            1.0,
            &["agent-elision", "control-flow", "compression"],
        ),
        rule(
            "AEL-0030",
            DB,
            "declaration-sigil-compression",
            Info,
            "Declaration keywords compressed to single characters in agent mode",
            "fn→f, let→v, let mut→m, struct→S, enum→E, trait→T, impl→I, mod→M, use→u, pub→+",
            None,
            1.0,
            &["agent-elision", "declaration", "compression"],
        ),
    ]
}

// ── Swarm Safety Rules (SWM-0001 .. SWM-0015) ──────────────────────

fn swarm_safety_rules() -> Vec<Rule> {
    use RuleDatabase::SwarmSafety as DB;
    use RuleSeverity::*;
    vec![
        rule(
            "SWM-0001",
            DB,
            "swarm-deadlock-prevention",
            Error,
            "Detect potential deadlocks in swarm message passing",
            "Agents in a swarm must not form circular blocking dependencies; the compiler verifies message DAGs are acyclic",
            None,
            0.9,
            &["swarm", "deadlock", "message-passing"],
        ),
        rule(
            "SWM-0002",
            DB,
            "consensus-quorum-validation",
            Error,
            "Consensus strategy must be achievable given swarm size",
            "A `unanimous` consensus with size > 1 requires all agents respond; `majority` requires size >= 3 for meaningful voting",
            None,
            1.0,
            &["swarm", "consensus", "quorum"],
        ),
        rule(
            "SWM-0003",
            DB,
            "topology-connectivity",
            Warning,
            "Swarm topology must be connected — no isolated agents",
            "For `ring`, `mesh`, and `tree` topologies, the compiler verifies every agent is reachable from every other agent",
            None,
            1.0,
            &["swarm", "topology", "connectivity"],
        ),
        rule(
            "SWM-0004",
            DB,
            "agent-capability-propagation",
            Error,
            "Swarm inherits the union of its agents' required capabilities",
            "A swarm dispatching to agents that require `llm` or `io` capabilities must itself hold those capabilities",
            None,
            1.0,
            &["swarm", "capability", "propagation"],
        ),
        rule(
            "SWM-0005",
            DB,
            "dispatch-type-safety",
            Error,
            "Dispatch block input/output types must match agent handler signatures",
            "The data scattered to agents must be compatible with the agent's handle() method signature",
            None,
            1.0,
            &["swarm", "dispatch", "type-safety"],
        ),
        rule(
            "SWM-0006",
            DB,
            "aggregate-completeness",
            Warning,
            "Aggregate block should handle partial results for fault tolerance",
            "When on_failure is not defined, the aggregate block must handle the case where fewer results return than dispatched",
            None,
            0.8,
            &["swarm", "aggregate", "fault-tolerance"],
        ),
        rule(
            "SWM-0007",
            DB,
            "swarm-effect-composition",
            Error,
            "Swarm operations compose effects from all constituent agents",
            "A swarm combining agents with / llm and / io effects must declare both in its own effect signature",
            None,
            1.0,
            &["swarm", "effects", "composition"],
        ),
        rule(
            "SWM-0008",
            DB,
            "message-ordering-guarantee",
            Info,
            "Message delivery order depends on topology",
            "Star topology: hub sees all messages; ring: sequential ordering; mesh: no ordering guarantee; broadcast: simultaneous",
            None,
            1.0,
            &["swarm", "message", "ordering"],
        ),
        rule(
            "SWM-0009",
            DB,
            "swarm-size-bounds",
            Warning,
            "Swarm size should be bounded to prevent resource exhaustion",
            "Swarms without an explicit size or with size > 1024 generate a warning; use @perf contracts for large swarms",
            None,
            0.7,
            &["swarm", "size", "resource"],
        ),
        rule(
            "SWM-0010",
            DB,
            "failure-cascade-prevention",
            Error,
            "Swarm must not cascade failures across agents",
            "An agent failure in a swarm must be isolated; the on_failure handler determines recovery (retry, skip, abort)",
            None,
            0.9,
            &["swarm", "failure", "cascade"],
        ),
        rule(
            "SWM-0011",
            DB,
            "broadcast-storm-prevention",
            Warning,
            "Broadcast topology with large swarms risks message storms",
            "Broadcast to > 100 agents without rate limiting generates a warning; prefer scatter/gather patterns",
            None,
            0.8,
            &["swarm", "broadcast", "storm"],
        ),
        rule(
            "SWM-0012",
            DB,
            "consensus-timeout-required",
            Warning,
            "Consensus operations should have explicit timeouts",
            "A swarm with consensus but no timeout may block indefinitely; the compiler inserts a default 30s timeout if omitted",
            None,
            0.9,
            &["swarm", "consensus", "timeout"],
        ),
        rule(
            "SWM-0013",
            DB,
            "swarm-send-sync-agents",
            Error,
            "Agents in a swarm must be Send + Sync for safe concurrent dispatch",
            "The compiler verifies agent types satisfy Send + Sync bounds before allowing swarm membership",
            None,
            1.0,
            &["swarm", "concurrency", "send-sync"],
        ),
        rule(
            "SWM-0014",
            DB,
            "swarm-keyword-compression",
            Info,
            "Agent mode compression for swarm constructs",
            "swarm→Σ/sw, topology→topo, consensus→cons in agent mode; human mode uses full keywords",
            None,
            1.0,
            &["swarm", "agent-elision", "compression"],
        ),
        rule(
            "SWM-0015",
            DB,
            "swarm-determinism-annotation",
            Info,
            "Non-deterministic swarm operations should be annotated",
            "Swarms with mesh topology or weighted consensus produce non-deterministic output; @inv contracts can constrain this",
            None,
            0.7,
            &["swarm", "determinism", "annotation"],
        ),
    ]
}

// ══════════════════════════════════════════════════════════════════════
//  Rule Query API
// ══════════════════════════════════════════════════════════════════════

/// Result of a rule query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleQueryResult {
    pub matches: Vec<Rule>,
    pub query_text: String,
}

/// Query rules by database.
pub fn query_rules_by_db(db: RuleDatabase) -> RuleQueryResult {
    let matches: Vec<_> = builtin_rules().into_iter().filter(|r| r.database == db).collect();
    RuleQueryResult { query_text: format!("database = {:?}", db), matches }
}

/// Query rules by category (exact match).
pub fn query_rules_by_category(category: &str) -> RuleQueryResult {
    let matches: Vec<_> = builtin_rules().into_iter().filter(|r| r.category == category).collect();
    RuleQueryResult { query_text: format!("category = {category}"), matches }
}

/// Query rules by tag.
pub fn query_rules_by_tag(tag: &str) -> RuleQueryResult {
    let matches: Vec<_> =
        builtin_rules().into_iter().filter(|r| r.tags.iter().any(|t| t == tag)).collect();
    RuleQueryResult { query_text: format!("rule tag = {tag}"), matches }
}

/// Query a rule by its ID (e.g. "OWN-0001").
pub fn query_rule_by_id(id: &str) -> Option<Rule> {
    builtin_rules().into_iter().find(|r| r.id == id)
}

/// Query rules by severity.
pub fn query_rules_by_severity(sev: RuleSeverity) -> RuleQueryResult {
    let matches: Vec<_> = builtin_rules().into_iter().filter(|r| r.severity == sev).collect();
    RuleQueryResult { query_text: format!("severity = {:?}", sev), matches }
}

/// Count total rules in the SKB.
pub fn rule_count() -> usize {
    builtin_rules().len()
}

/// Count rules per database.
pub fn rule_counts_by_db() -> Vec<(RuleDatabase, usize)> {
    use RuleDatabase::*;
    [Ownership, Borrow, Lifetime, TypeSafety, Concurrency, FFI, AgentElision, SwarmSafety]
        .into_iter()
        .map(|db| {
            let count = builtin_rules().iter().filter(|r| r.database == db).count();
            (db, count)
        })
        .collect()
}

// ══════════════════════════════════════════════════════════════════════
//  Symbol Metadata (original SKB entries for the query API)
// ══════════════════════════════════════════════════════════════════════

/// A single SKB entry for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkbEntry {
    /// Fully qualified name: e.g. "std.io.read_file".
    pub fqn: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// Effects declared by this symbol.
    pub effects: Vec<String>,
    /// Required capabilities to invoke.
    pub capabilities: Vec<String>,
    /// Spec block (pre/post conditions), if any.
    pub spec: Option<SpecBlock>,
    /// Whether the symbol is deprecated.
    pub deprecated: Option<String>,
    /// Aliases from the Rust ecosystem.
    pub rust_aliases: Vec<String>,
    /// Tags for semantic search.
    pub tags: Vec<String>,
}

/// Spec block attached to a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecBlock {
    pub requires: Vec<String>,
    pub ensures: Vec<String>,
}

/// Symbol kind in the knowledge base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Constant,
    TypeAlias,
}

// ── Built-in SKB ─────────────────────────────────────────────────────

fn builtin_skb() -> Vec<SkbEntry> {
    vec![
        // I/O
        entry(
            "std.io.read_file",
            SymbolKind::Function,
            &["io", "fs"],
            &["fs.read"],
            Some(spec(&["path.exists()"], &["ret.is_ok() => ret.unwrap().len() >= 0"])),
            &["fs::read_to_string"],
            &["io", "file", "read"],
        ),
        entry(
            "std.io.write_file",
            SymbolKind::Function,
            &["io", "fs"],
            &["fs.write"],
            Some(spec(&["path.parent().exists()"], &["path.exists()"])),
            &["fs::write"],
            &["io", "file", "write"],
        ),
        entry(
            "std.io.stdin",
            SymbolKind::Function,
            &["io"],
            &[],
            None,
            &["io::stdin"],
            &["io", "input"],
        ),
        entry(
            "std.io.stdout",
            SymbolKind::Function,
            &["io"],
            &[],
            None,
            &["io::stdout"],
            &["io", "output"],
        ),
        // Net
        entry(
            "std.net.TcpStream",
            SymbolKind::Struct,
            &["io", "net"],
            &["network"],
            None,
            &["net::TcpStream"],
            &["network", "tcp", "stream"],
        ),
        entry(
            "std.net.listen",
            SymbolKind::Function,
            &["io", "net"],
            &["network"],
            Some(spec(&["port > 0", "port < 65536"], &["ret.is_ok() => listener.is_bound()"])),
            &["TcpListener::bind"],
            &["network", "tcp", "listen", "server"],
        ),
        // Collections
        entry(
            "std.collections.Vec",
            SymbolKind::Struct,
            &[],
            &[],
            None,
            &["Vec"],
            &["collection", "array", "vector"],
        ),
        entry(
            "std.collections.HashMap",
            SymbolKind::Struct,
            &[],
            &[],
            None,
            &["HashMap"],
            &["collection", "map", "hash"],
        ),
        // Agent primitives
        entry(
            "std.agent.Agent",
            SymbolKind::Trait,
            &[],
            &[],
            None,
            &[],
            &["agent", "trait", "handle"],
        ),
        entry(
            "std.agent.Swarm",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &[],
            &["agent", "swarm", "multi-agent"],
        ),
        entry(
            "std.agent.Bus",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &[],
            &["agent", "bus", "pubsub", "messaging"],
        ),
        entry(
            "std.agent.Memory",
            SymbolKind::Struct,
            &["io"],
            &[],
            None,
            &[],
            &["agent", "memory", "persist", "state"],
        ),
        entry(
            "std.agent.Lease",
            SymbolKind::Struct,
            &[],
            &[],
            None,
            &[],
            &["agent", "capability", "lease", "rbac"],
        ),
        // Sync
        entry(
            "std.sync.Mutex",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &["Mutex"],
            &["sync", "mutex", "lock"],
        ),
        entry(
            "std.sync.RwLock",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &["RwLock"],
            &["sync", "rwlock", "read-write"],
        ),
        entry(
            "std.sync.Channel",
            SymbolKind::Struct,
            &["concurrency"],
            &[],
            None,
            &["mpsc::channel"],
            &["sync", "channel", "mpsc"],
        ),
    ]
}

fn entry(
    fqn: &str,
    kind: SymbolKind,
    effects: &[&str],
    caps: &[&str],
    spec: Option<SpecBlock>,
    aliases: &[&str],
    tags: &[&str],
) -> SkbEntry {
    SkbEntry {
        fqn: fqn.into(),
        kind,
        effects: effects.iter().map(|s| s.to_string()).collect(),
        capabilities: caps.iter().map(|s| s.to_string()).collect(),
        spec,
        deprecated: None,
        rust_aliases: aliases.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

fn spec(requires: &[&str], ensures: &[&str]) -> SpecBlock {
    SpecBlock {
        requires: requires.iter().map(|s| s.to_string()).collect(),
        ensures: ensures.iter().map(|s| s.to_string()).collect(),
    }
}

// ── Query Interface ──────────────────────────────────────────────────

/// Result of an SKB query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub matches: Vec<SkbEntry>,
    pub query_text: String,
}

/// Query the SKB by fully qualified name (exact or prefix match).
pub fn query_by_fqn(fqn: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.fqn == fqn || e.fqn.starts_with(&format!("{fqn}.")))
        .collect();
    QueryResult { matches, query_text: format!("fqn = {fqn}") }
}

/// Query the SKB by effect name: find all symbols that declare a given effect.
pub fn query_by_effect(effect: &str) -> QueryResult {
    let matches: Vec<_> =
        builtin_skb().into_iter().filter(|e| e.effects.iter().any(|eff| eff == effect)).collect();
    QueryResult { matches, query_text: format!("effect = {effect}") }
}

/// Query by required capability: find symbols that require a given capability.
pub fn query_by_capability(cap: &str) -> QueryResult {
    let matches: Vec<_> =
        builtin_skb().into_iter().filter(|e| e.capabilities.iter().any(|c| c == cap)).collect();
    QueryResult { matches, query_text: format!("capability = {cap}") }
}

/// Query by tag (semantic search).
pub fn query_by_tag(tag: &str) -> QueryResult {
    let matches: Vec<_> =
        builtin_skb().into_iter().filter(|e| e.tags.iter().any(|t| t == tag)).collect();
    QueryResult { matches, query_text: format!("tag = {tag}") }
}

/// Query by Rust alias: find the MechGen equivalent of a Rust symbol.
pub fn query_by_rust_alias(alias: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.rust_aliases.iter().any(|a| a.contains(alias)))
        .collect();
    QueryResult { matches, query_text: format!("rust_alias contains {alias}") }
}

/// Lookup the spec block for a symbol.
pub fn query_spec(fqn: &str) -> Option<SpecBlock> {
    builtin_skb().into_iter().find(|e| e.fqn == fqn).and_then(|e| e.spec)
}

/// List all symbols in a module (prefix match).
pub fn query_module(module_prefix: &str) -> QueryResult {
    let matches: Vec<_> =
        builtin_skb().into_iter().filter(|e| e.fqn.starts_with(module_prefix)).collect();
    QueryResult { matches, query_text: format!("module = {module_prefix}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_exact_fqn() {
        let result = query_by_fqn("std.io.read_file");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].fqn, "std.io.read_file");
    }

    #[test]
    fn query_module_prefix() {
        let result = query_module("std.io");
        assert!(result.matches.len() >= 2);
    }

    #[test]
    fn query_io_effect() {
        let result = query_by_effect("io");
        assert!(result.matches.len() >= 3);
    }

    #[test]
    fn query_network_capability() {
        let result = query_by_capability("network");
        assert!(result.matches.len() >= 1);
    }

    #[test]
    fn query_rust_alias() {
        let result = query_by_rust_alias("HashMap");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].fqn, "std.collections.HashMap");
    }

    #[test]
    fn query_spec_read_file() {
        let spec = query_spec("std.io.read_file").unwrap();
        assert!(!spec.requires.is_empty());
        assert!(!spec.ensures.is_empty());
    }

    #[test]
    fn query_agent_primitives() {
        let result = query_by_tag("agent");
        assert!(result.matches.len() >= 4);
    }

    #[test]
    fn query_nonexistent() {
        let result = query_by_fqn("nonexistent.module");
        assert!(result.matches.is_empty());
    }

    // ── Rule database tests ──────────────────────────────────────────

    #[test]
    fn rule_total_count_210() {
        assert_eq!(rule_count(), 255);
    }

    #[test]
    fn rule_counts_per_database() {
        let counts = rule_counts_by_db();
        let map: std::collections::HashMap<_, _> =
            counts.into_iter().map(|(db, n)| (format!("{:?}", db), n)).collect();
        assert_eq!(map["Ownership"], 40);
        assert_eq!(map["Borrow"], 40);
        assert_eq!(map["Lifetime"], 35);
        assert_eq!(map["TypeSafety"], 40);
        assert_eq!(map["Concurrency"], 35);
        assert_eq!(map["FFI"], 20);
        assert_eq!(map["AgentElision"], 30);
        assert_eq!(map["SwarmSafety"], 15);
    }

    #[test]
    fn query_rule_by_id_own0001() {
        let rule = query_rule_by_id("OWN-0001").expect("OWN-0001 should exist");
        assert_eq!(rule.database, RuleDatabase::Ownership);
        assert_eq!(rule.category, "use-after-move");
        assert_eq!(rule.severity, RuleSeverity::Error);
        assert!(rule.fix_template.is_some());
    }

    #[test]
    fn query_rule_by_id_nonexistent() {
        assert!(query_rule_by_id("XXX-9999").is_none());
    }

    #[test]
    fn query_rules_by_db_ownership() {
        let result = query_rules_by_db(RuleDatabase::Ownership);
        assert_eq!(result.matches.len(), 40);
        assert!(result.matches.iter().all(|r| r.database == RuleDatabase::Ownership));
    }

    #[test]
    fn query_rules_by_db_ffi() {
        let result = query_rules_by_db(RuleDatabase::FFI);
        assert_eq!(result.matches.len(), 20);
        assert!(result.matches[0].id.starts_with("FFI-"));
    }

    #[test]
    fn query_rules_by_category_data_race() {
        let result = query_rules_by_category("data-race");
        assert!(result.matches.len() >= 1);
        assert!(result.matches.iter().all(|r| r.category == "data-race"));
    }

    #[test]
    fn query_rules_by_tag_concurrency() {
        let result = query_rules_by_tag("concurrency");
        assert!(result.matches.len() >= 5);
    }

    #[test]
    fn query_rules_by_severity_error() {
        let result = query_rules_by_severity(RuleSeverity::Error);
        assert!(result.matches.len() >= 20);
        assert!(result.matches.iter().all(|r| r.severity == RuleSeverity::Error));
    }

    #[test]
    fn all_rule_ids_unique() {
        let rules = builtin_rules();
        let mut ids: Vec<&str> = rules.iter().map(|r| r.id.as_str()).collect();
        ids.sort();
        let len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), len, "duplicate rule IDs detected");
    }

    #[test]
    fn rules_serializable_to_json() {
        let rule = query_rule_by_id("BOR-0001").unwrap();
        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("BOR-0001"));
        assert!(json.contains("double-mutable-borrow"));
    }
}
