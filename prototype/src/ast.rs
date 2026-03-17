/// Redox AST — Abstract Syntax Tree for the Redox canonical syntax.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub visibility: Visibility,
    pub attributes: Vec<Attribute>,
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Private,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute {
    pub name: String,
    pub args: Vec<String>,
    pub bang: bool, // e.g., @i!
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
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
    Static(StaticDef),
    Effect(EffectDef),
    Spec(SpecDef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub is_async: bool,
    pub is_unsafe: bool,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub where_clause: Vec<WherePredicate>,
    pub effects: Vec<String>,
    pub body: Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<String>,
    pub default: Option<Type>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Type {
    Path { segments: Vec<String>, type_args: Vec<Type> },
    Reference { mutable: bool, inner: Box<Type> },
    OwnedPtr { inner: Box<Type> },        // ^T
    Rc { inner: Box<Type> },              // $T
    Arc { inner: Box<Type> },             // @T
    Cow { inner: Box<Type> },             // &~T
    Cell { inner: Box<Type> },            // %T
    RefCell { inner: Box<Type> },         // %!T
    Mutex { inner: Box<Type> },           // #T
    RwLock { inner: Box<Type> },          // #~T
    Slice { inner: Box<Type> },           // [T]
    Array { inner: Box<Type>, size: Box<Expr> }, // [T; N]
    Vec { inner: Box<Type> },             // [T]~
    Set { inner: Box<Type> },             // {T}
    Tuple { elements: Vec<Type> },
    Option { inner: Box<Type> },          // ?T
    Result { ok: Box<Type>, err: Box<Type> }, // R[T, E]
    Map { key: Box<Type>, value: Box<Type> }, // {K: V}
    Ptr { inner: Box<Type> },             // Ptr[T]
    Simd { inner: Box<Type>, width: u64 }, // Simd[T, N]
    Fn { params: Vec<Type>, ret: Option<Box<Type>> },
    Never,                                // !
    Inferred,                             // _
    SelfType,                             // _T
    StringType,                           // s
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail_expr: Option<Box<Expr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Stmt {
    Let { mutable: bool, pattern: Pattern, ty: Option<Type>, value: Expr },
    Expr { expr: Expr },
    Item { item: Box<Item> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Expr {
    Literal { value: String, kind: LiteralKind },
    Ident { name: String },
    Binary { op: String, left: Box<Expr>, right: Box<Expr> },
    Unary { op: String, operand: Box<Expr> },
    Call { func: Box<Expr>, args: Vec<Expr> },
    MethodCall { receiver: Box<Expr>, method: String, type_args: Vec<Type>, args: Vec<Expr> },
    FieldAccess { object: Box<Expr>, field: String },
    Index { object: Box<Expr>, index: Box<Expr> },
    StructLit { path: Vec<String>, fields: Vec<FieldInit> },
    TupleLit { elements: Vec<Expr> },
    ArrayLit { elements: Vec<Expr> },
    ArrayRepeat { value: Box<Expr>, count: Box<Expr> },
    Closure { params: Vec<Param>, body: Box<Expr> },
    If { cond: Box<Expr>, then_block: Block, else_block: Option<Block> },
    Match { scrutinee: Option<Box<Expr>>, arms: Vec<MatchArm> },
    Loop { body: Block },
    While { cond: Box<Expr>, body: Block },
    For { pattern: Pattern, iter: Box<Expr>, body: Block },
    Block { block: Block },
    Return { value: Option<Box<Expr>> },
    Break { value: Option<Box<Expr>> },
    Continue,
    Try { expr: Box<Expr> },
    Await { expr: Box<Expr> },
    Cast { expr: Box<Expr>, ty: Type },
    Assign { target: Box<Expr>, value: Box<Expr> },
    Range { start: Box<Expr>, end: Box<Expr>, inclusive: bool },
    Todo,
    Unimplemented,
    UnsafeBlock { block: Block },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiteralKind {
    Int,
    Float,
    String,
    FormatString,
    Char,
    Bool,
    Byte,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInit {
    pub name: String,
    pub value: Option<Expr>, // None means shorthand (name = local var)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Pattern {
    Ident { name: String },
    Literal { value: String },
    Wildcard,
    Tuple { elements: Vec<Pattern> },
    Struct { path: Vec<String>, fields: Vec<FieldPattern> },
    Enum { path: Vec<String>, elements: Vec<Pattern> },
    Slice { elements: Vec<Pattern>, rest: bool },
    Or { patterns: Vec<Pattern> },
    Ref { pattern: Box<Pattern> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldPattern {
    pub name: String,
    pub pattern: Option<Pattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructField {
    pub visibility: Visibility,
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub kind: VariantKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariantKind {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<StructField>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub super_traits: Vec<String>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplBlock {
    pub generics: Vec<GenericParam>,
    pub self_type: Type,
    pub trait_path: Option<Vec<String>>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDef {
    pub name: String,
    pub items: Option<Vec<Item>>, // None = external module (M foo;)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseDef {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub glob: bool,
    pub group: Vec<UseDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeAlias {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub ty: Type,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstDef {
    pub name: String,
    pub ty: Type,
    pub value: Expr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDef {
    pub name: String,
    pub operations: Vec<EffectOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectOp {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub items: Vec<SpecItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpecItem {
    Require(String),
    Ensure(String),
    Performance(String, String),
    Effect(Vec<String>),
    Invariant(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticDef {
    pub name: String,
    pub mutable: bool,
    pub ty: Type,
    pub value: Expr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WherePredicate {
    pub type_param: String,
    pub bounds: Vec<String>,
}
