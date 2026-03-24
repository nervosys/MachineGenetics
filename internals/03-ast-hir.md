# Chapter 3: AST & HIR

The compiler uses two tree representations: the **AST** (Abstract Syntax
Tree), produced by the parser, and the **HIR** (High-level Intermediate
Representation), produced by lowering the AST after name resolution.

---

## 3.1 AST Design

The AST is in `rdx_ast` (prototype: `prototype/src/ast.rs`).

### Top-Level Structure

```rust
pub struct Module {
    pub items: Vec<Item>,
}

pub struct Item {
    pub visibility: Visibility,  // Private | Public
    pub attributes: Vec<Attribute>,
    pub kind: ItemKind,
}

pub enum ItemKind {
    Function(FunctionDef),
    Struct(StructDef),
    Enum(EnumDef),
    Trait(TraitDef),
    Impl(ImplBlock),
    Module(ModuleDef),
    Use(UseDef),
    TypeAlias(TypeAlias),
    Const(ConstDef),
    Effect(EffectDef),
    Spec(SpecDef),
}
```

Every AST node is `Serialize`/`Deserialize` — the entire AST can be emitted
as JSON for agent consumption via `mg parse --emit ast`.

### Type Representation

The AST `Type` enum represents source-level type syntax, including all MechGen
sugar:

```rust
pub enum Type {
    Path { segments: Vec<String>, type_args: Vec<Type> },
    Reference { mutable: bool, inner: Box<Type> },
    OwnedPtr { inner: Box<Type> },         // ^T  (Box)
    Rc { inner: Box<Type> },               // $T  (Rc)
    Arc { inner: Box<Type> },              // @T  (Arc)
    Slice { inner: Box<Type> },            // [T]
    Array { inner: Box<Type>, size: Box<Expr> },  // [T; N]
    Vec { inner: Box<Type> },              // [T]~
    Tuple { elements: Vec<Type> },
    Option { inner: Box<Type> },           // ?T
    Result { ok: Box<Type>, err: Box<Type> },  // R[T, E]
    Map { key: Box<Type>, value: Box<Type> },  // {K: V}
    Fn { params: Vec<Type>, ret: Option<Box<Type>> },
    Never,                                 // !
    Inferred,                              // _
    SelfType,                              // _T
    StringType,                            // s
}
```

Each variant directly maps to MechGen syntax — no desugaring at this stage.
For example, `?T` in source becomes `Type::Option { inner: T }` in the AST.

### Expression Nodes

```rust
pub enum Expr {
    Literal { value: String, kind: LiteralKind },
    Ident { name: String },
    Binary { op: String, left: Box<Expr>, right: Box<Expr> },
    Unary { op: String, operand: Box<Expr> },
    Call { func: Box<Expr>, args: Vec<Expr> },
    MethodCall { receiver: Box<Expr>, method: String, args: Vec<Expr> },
    FieldAccess { object: Box<Expr>, field: String },
    Index { object: Box<Expr>, index: Box<Expr> },
    StructLit { path: Vec<String>, fields: Vec<FieldInit> },
    If { condition: Box<Expr>, then_block: Block, else_block: Option<Block> },
    Match { scrutinee: Box<Expr>, arms: Vec<MatchArm> },
    ForLoop { pattern: Pattern, iter: Box<Expr>, body: Block },
    Loop { body: Block },
    Block(Block),
    Return { value: Option<Box<Expr>> },
    Closure { params: Vec<Param>, body: Box<Expr> },
    Await { inner: Box<Expr> },
    Try { inner: Box<Expr> },     // ?  postfix
    Range { start: Option<Box<Expr>>, end: Option<Box<Expr>>, inclusive: bool },
    FormatString { parts: Vec<FormatPart> },
    PrintString { parts: Vec<FormatPart> },
    Handle { effects: Vec<String>, body: Block, handlers: Vec<Handler> },
    // ...
}
```

### Blocks and Statements

```rust
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail_expr: Option<Box<Expr>>,  // implicit return
}

pub enum Stmt {
    Let { mutable: bool, pattern: Pattern, ty: Option<Type>, value: Expr },
    Expr { expr: Expr },
    Item { item: Box<Item> },
}
```

The `tail_expr` represents MechGen's expression-oriented blocks: the last
expression without a semicolon is the block's value.

---

## 3.2 Name Resolution

Name resolution is in `rdx_resolve` (prototype: `prototype/src/resolve.rs`).

### What It Does

1. Walk the AST and build a scope tree
2. Assign a `SymbolId` to every definition
3. Link every identifier use to its definition's `SymbolId`
4. Report undefined name errors
5. Build the module tree (visibility, imports)

### Scope Model

```rust
pub struct Scope {
    pub parent: Option<ScopeId>,
    pub kind: ScopeKind,        // Module, Function, Block, Impl
    pub definitions: HashMap<String, SymbolId>,
}

pub enum ScopeKind {
    Root,
    Module,
    Function,
    Block,
    Impl,
    Trait,
}
```

Scopes nest: a block scope can see its parent function scope, which can see
its parent module scope. Name lookup walks up the scope chain.

### Use Resolution

```MechGen
u std.collections.{HashMap, HashSet}
```

The resolver:
1. Parses the use path (`std` → `collections` → `{HashMap, HashSet}`)
2. Walks the module tree to find each target
3. Inserts the imported names into the current scope
4. Records the `SymbolId` mapping

### Import Styles

| Syntax                                 | Resolution                     |
| -------------------------------------- | ------------------------------ |
| `u std.fs`                             | Import module `fs`             |
| `u std.fs.read_to_string`              | Import single function         |
| `u std.collections.{HashMap, HashSet}` | Import multiple items          |
| `u std.collections.*`                  | Glob import (all public items) |
| `u crate.utils.helper`                 | Crate-relative import          |
| `u super.sibling`                      | Parent-relative import         |

---

## 3.3 HIR Design

The HIR is in `rdx_hir` (prototype: `prototype/src/hir.rs`).

### Purpose

The HIR differs from the AST in several ways:

| Aspect        | AST                | HIR                     |
| ------------- | ------------------ | ----------------------- |
| Names         | Strings            | `SymbolId` (resolved)   |
| Types         | Source syntax      | `Ty` (canonical)        |
| Effects       | String annotations | `EffectSet` (validated) |
| Sugar         | Preserved          | Desugared               |
| Serialization | JSON for tools     | Internal only           |

### Ty — Canonical Type

```rust
pub enum Ty {
    Int(IntTy),          // i8, i16, i32, i64, i128, isize
    Uint(UintTy),        // u8, u16, u32, u64, u128, usize
    Float(FloatTy),      // f32, f64
    Bool,
    Str,                 // &str
    Char,
    Unit,                // ()
    Never,               // !
    Named(SymbolId, Vec<Ty>),  // user-defined type + type args
    Ref(bool, Box<Ty>),  // &T or &!T
    OwnedPtr(Box<Ty>),   // ^T
    Rc(Box<Ty>),         // $T
    Arc(Box<Ty>),        // @T
    Slice(Box<Ty>),      // [T]
    Array(Box<Ty>, u64), // [T; N]
    Vec(Box<Ty>),        // [T]~
    Tuple(Vec<Ty>),
    Option(Box<Ty>),     // ?T
    Result(Box<Ty>, Box<Ty>),  // R[T, E]
    Map(Box<Ty>, Box<Ty>),     // {K: V}
    Set(Box<Ty>),              // {T}
    Fn(Vec<Ty>, Box<Ty>),      // fn type
    TypeVar(u32),              // inference variable
    Error,                     // sentinel for error recovery
}
```

### EffectSet

```rust
pub type EffectSet = BTreeSet<Effect>;

pub enum Effect {
    Io,        // file system, stdio
    Net,       // network access
    Async,     // async operations
    Unsafe,    // unsafe code
    Db,        // database access
    Agent,     // agent spawning
    Log,       // logging / tracing
    Env,       // environment variables
    Custom(String),  // user-defined effects
}
```

### HIR Expression

```rust
pub struct HirExpr {
    pub id: HirExprId,
    pub kind: HirExprKind,
    pub ty: Ty,            // inferred or checked type
    pub effects: EffectSet, // effects this expression produces
    pub span: Span,
}
```

Every HIR expression carries its type and effect set. This enables
downstream passes (MLIR codegen, capability checking) to query any
expression's type and effects without recomputation.

### Lowering: AST → HIR

The lowering pass performs these transformations:

| AST               | HIR                                        |
| ----------------- | ------------------------------------------ |
| `?T` (type)       | `Ty::Option(T)`                            |
| `R[T, E]` (type)  | `Ty::Result(T, E)`                         |
| `[T]~` (type)     | `Ty::Vec(T)`                               |
| `{K: V}` (type)   | `Ty::Map(K, V)`                            |
| `^T` (type)       | `Ty::OwnedPtr(T)`                          |
| `$T` (type)       | `Ty::Rc(T)`                                |
| `@T` (type)       | `Ty::Arc(T)`                               |
| `s` (type)        | `Ty::Str`                                  |
| `1b` / `0b`       | `HirExpr::Literal(Bool, true/false)`       |
| `p"hello {x}"`    | `HirExpr::Call(println, format_args)`      |
| `f"hello {x}"`    | `HirExpr::Call(format, format_args)`       |
| String names      | `SymbolId` references                      |
| `+` prefix        | `Visibility::Public` (already done in AST) |
| `? expr { arms }` | `HirExpr::Match(expr, arms)`               |
| `@ x ~ iter { }`  | `HirExpr::ForLoop(x, iter, body)`          |

---

## 3.4 AST Visitors

Both AST and HIR provide visitor traits for implementing passes:

```rust
pub trait AstVisitor {
    fn visit_item(&mut self, item: &Item) { walk_item(self, item) }
    fn visit_fn(&mut self, f: &FunctionDef) { walk_fn(self, f) }
    fn visit_expr(&mut self, e: &Expr) { walk_expr(self, e) }
    fn visit_stmt(&mut self, s: &Stmt) { walk_stmt(self, s) }
    fn visit_type(&mut self, t: &Type) { walk_type(self, t) }
    fn visit_pattern(&mut self, p: &Pattern) { walk_pattern(self, p) }
}
```

The `walk_*` functions provide the default traversal. Override a `visit_*`
method to run custom logic at that node type, call the corresponding `walk_*`
to continue recursion.

### Example: Count All Function Definitions

```rust
struct FnCounter { count: usize }

impl AstVisitor for FnCounter {
    fn visit_fn(&mut self, _f: &FunctionDef) {
        self.count += 1;
        // don't recurse into nested fns — or call walk_fn(self, f) to do so
    }
}
```
