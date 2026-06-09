/// MechGen AST — Abstract Syntax Tree for the MechGen canonical syntax.
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
    Agent(AgentDef),
    Net(NetDef),
    Kb(KbDef),
    Evolve(EvolveDef),
    Train(TrainDef),
    Swarm(SwarmDef),
    Data(DataDef),
    Extend(ExtendBlock),
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
    pub contracts: Vec<ContractClause>,
    pub body: Block,
    pub body_expr: Option<Box<Expr>>,
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
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Type {
    Path {
        segments: Vec<String>,
        type_args: Vec<Type>,
    },
    Reference {
        mutable: bool,
        inner: Box<Type>,
    },
    OwnedPtr {
        inner: Box<Type>,
    }, // ^T
    Rc {
        inner: Box<Type>,
    }, // $T
    Arc {
        inner: Box<Type>,
    }, // @T
    Cow {
        inner: Box<Type>,
    }, // &~T
    Cell {
        inner: Box<Type>,
    }, // %T
    RefCell {
        inner: Box<Type>,
    }, // %!T
    Mutex {
        inner: Box<Type>,
    }, // #T
    RwLock {
        inner: Box<Type>,
    }, // #~T
    Slice {
        inner: Box<Type>,
    }, // [T]
    Array {
        inner: Box<Type>,
        size: Box<Expr>,
    }, // [T; N]
    Vec {
        inner: Box<Type>,
    }, // [T]~
    Set {
        inner: Box<Type>,
    }, // {T}
    Tuple {
        elements: Vec<Type>,
    },
    Option {
        inner: Box<Type>,
    }, // ?T
    Result {
        ok: Box<Type>,
        err: Box<Type>,
    }, // R[T, E]
    Map {
        key: Box<Type>,
        value: Box<Type>,
    }, // {K: V}
    Ptr {
        inner: Box<Type>,
    }, // Ptr[T]
    Simd {
        inner: Box<Type>,
        width: u64,
    }, // Simd[T, N]
    Tensor {
        inner: Box<Type>,
        shape: Vec<TensorDim>,
    }, // Tensor[T, Shape]
    ParamTy {
        inner: Box<Type>,
        shape: Vec<TensorDim>,
    }, // Param[T, Shape]
    Genome {
        inner: Box<Type>,
    }, // Genome[T]
    Policy {
        state: Box<Type>,
        action: Box<Type>,
    }, // Policy[S, A]
    KnowledgeBase, // KnowledgeBase
    LlmType,       // LLM
    Fn {
        params: Vec<Type>,
        ret: Option<Box<Type>>,
    },
    Never,      // !
    Inferred,   // _
    SelfType,   // _T
    StringType, // s
    /// A refinement type: base type with a value-level predicate.
    Refined {
        base: Box<Type>,
        predicate: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail_expr: Option<Box<Expr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Stmt {
    Let {
        mutable: bool,
        pattern: Pattern,
        ty: Option<Type>,
        value: Expr,
    },
    Expr {
        expr: Expr,
    },
    Item {
        item: Box<Item>,
    },
    Guard {
        cond: Expr,
        else_block: Block,
    },
    Defer {
        expr: Expr,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Expr {
    Literal {
        value: String,
        kind: LiteralKind,
    },
    Ident {
        name: String,
    },
    Binary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: String,
        operand: Box<Expr>,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        type_args: Vec<Type>,
        args: Vec<Expr>,
    },
    FieldAccess {
        object: Box<Expr>,
        field: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    StructLit {
        path: Vec<String>,
        fields: Vec<FieldInit>,
    },
    TupleLit {
        elements: Vec<Expr>,
    },
    ArrayLit {
        elements: Vec<Expr>,
    },
    ArrayRepeat {
        value: Box<Expr>,
        count: Box<Expr>,
    },
    /// Map literal: `{}` or `{k: v, k: v, ...}`.
    /// Pairs are positional; type inference figures out K and V.
    MapLit {
        entries: Vec<(Expr, Expr)>,
    },
    Closure {
        params: Vec<Param>,
        body: Box<Expr>,
    },
    If {
        cond: Box<Expr>,
        then_block: Block,
        else_block: Option<Block>,
    },
    Match {
        scrutinee: Option<Box<Expr>>,
        arms: Vec<MatchArm>,
    },
    Loop {
        body: Block,
    },
    While {
        cond: Box<Expr>,
        body: Block,
    },
    For {
        pattern: Pattern,
        iter: Box<Expr>,
        body: Block,
    },
    Block {
        block: Block,
    },
    Return {
        value: Option<Box<Expr>>,
    },
    Break {
        value: Option<Box<Expr>>,
    },
    Continue,
    Try {
        expr: Box<Expr>,
    },
    Await {
        expr: Box<Expr>,
    },
    Cast {
        expr: Box<Expr>,
        ty: Type,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
    Pipeline {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Is {
        expr: Box<Expr>,
        pattern: Pattern,
    },
    Todo,
    Unimplemented,
    UnsafeBlock {
        block: Block,
    },
    Error {
        message: String,
    },
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
    Ident {
        name: String,
    },
    Literal {
        value: String,
    },
    Wildcard,
    Tuple {
        elements: Vec<Pattern>,
    },
    Struct {
        path: Vec<String>,
        fields: Vec<FieldPattern>,
    },
    Enum {
        path: Vec<String>,
        elements: Vec<Pattern>,
    },
    Slice {
        elements: Vec<Pattern>,
        rest: bool,
    },
    Or {
        patterns: Vec<Pattern>,
    },
    Ref {
        pattern: Box<Pattern>,
    },
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
    pub contracts: Vec<ContractClause>,
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
    /// Optional refinement predicate (`~> condition`).
    pub refinement: Option<String>,
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
    /// Optional function-style parameters (for synthesis specs).
    pub params: Vec<Param>,
    /// Optional return type (for synthesis specs).
    pub return_type: Option<Type>,
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

/// An agent capability definition: `agent Name { capabilities: [...], ... }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    /// Capabilities this agent has (e.g. "read_source", "query_types").
    pub capabilities: Vec<String>,
    /// Operations requiring explicit approval.
    pub requires_approval: Vec<String>,
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

/// A contract clause attached to a function or type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractClause {
    pub kind: ContractClauseKind,
    /// The condition expression text (e.g. "n > 0").
    pub condition: String,
    /// Optional human-readable message.
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractClauseKind {
    /// @req — precondition.
    Requires,
    /// @ens — postcondition.
    Ensures,
    /// @inv — invariant.
    Invariant,
}

// ── AI subsystem definitions ─────────────────────────────────

/// A dimension in a tensor shape: either a named variable or a literal integer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TensorDim {
    Lit(u64),
    Var(String),
}

/// Neural network definition: `net Name { layer ...; layer ...; forward { ... } }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub layers: Vec<LayerDef>,
    pub forward: Block,
}

/// A single layer inside a `net` definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDef {
    pub name: String,
    pub layer_type: Type,
    pub args: Vec<Expr>,
}

/// Knowledge base definition: `kb Name { fact ...; rule ...; }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbDef {
    pub name: String,
    pub facts: Vec<FactDef>,
    pub rules: Vec<RuleDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactDef {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDef {
    pub name: String,
    pub params: Vec<Param>,
    pub conditions: Vec<Expr>,
    pub body: Block,
}

/// Evolutionary computation definition: `evolve Name { genome ...; fitness { ... } }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolveDef {
    pub name: String,
    pub genome_type: Type,
    pub population_size: Option<Expr>,
    pub generations: Option<Expr>,
    pub fitness: Block,
    pub mutate_fn: Option<Block>,
    pub crossover_fn: Option<Block>,
    pub select_fn: Option<Block>,
}

/// Training loop definition: `train Name { ... }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainDef {
    pub name: String,
    pub net: String,
    pub optimizer: Option<Expr>,
    pub loss: Option<Expr>,
    pub epochs: Option<Expr>,
    pub body: Block,
    /// Optional inline training inputs as an array-of-arrays literal:
    /// `inputs: [[0.5, 0.5], [1.0, 0.0]]`. When present alongside
    /// `targets`, replaces the synthetic dataset in `--target=ml-train`.
    pub inputs: Option<Expr>,
    /// Optional inline training targets, paired one-to-one with `inputs`.
    pub targets: Option<Expr>,
    /// Optional path to a CSV file containing the training set. First
    /// `in_dim` columns are inputs, last `out_dim` columns are targets.
    /// Overridden by `inputs`/`targets` when those are present.
    pub dataset: Option<Expr>,
    /// Optional validation split as a fraction in `[0.0, 1.0)` — the last
    /// fraction of the dataset is held out for per-epoch val loss.
    pub val_split: Option<Expr>,
    /// Optional path for saving/loading trained weights. If the file
    /// exists at train start, weights are loaded; after training they
    /// are written back, allowing iterative refinement across runs.
    pub checkpoint: Option<Expr>,
    /// Optional mini-batch size. When set, each epoch shuffles the train
    /// set and iterates through `ceil(n_train / batch_size)` mini-batches
    /// per epoch. Defaults to full-batch (no shuffle).
    pub batch_size: Option<Expr>,
    /// Optional early-stopping patience: halt if validation loss hasn't
    /// improved for this many consecutive epochs. Requires `val_split > 0`.
    pub patience: Option<Expr>,
    /// Optional generation prompt: array of integer token IDs.
    /// Used by `--target=ml-generate` as the seed sequence.
    pub prompt: Option<Expr>,
    /// Optional max tokens to generate (default 16) when
    /// `--target=ml-generate` is invoked.
    pub max_tokens: Option<Expr>,
    /// Generation sampling temperature. `0` or absent means greedy argmax;
    /// `1.0` is raw softmax sampling; `<1.0` sharpens, `>1.0` flattens.
    pub temperature: Option<Expr>,
    /// Restrict generation sampling to the top-k logits. `0` or absent
    /// means full vocab sampling. Combines with `temperature`.
    pub top_k: Option<Expr>,
    /// Restrict generation sampling to the smallest set of tokens whose
    /// cumulative softmax probability ≥ `top_p`. `0` or absent disables.
    pub top_p: Option<Expr>,
    /// LCG seed for deterministic generation sampling (and shuffling).
    pub seed: Option<Expr>,
    /// Per-tensor gradient clipping by L2 norm. If `||g||₂ > clip_grad`,
    /// scale `g` by `clip_grad / ||g||₂` before the optimiser step.
    pub clip_grad: Option<Expr>,
    /// Linear LR warmup over this many initial steps (lr ramps 0 → base).
    pub warmup_steps: Option<Expr>,
    /// LR schedule after warmup: `none` (default) or `cosine` (half-cycle
    /// cosine decay to 0 over `epochs · batches_per_epoch − warmup_steps`).
    pub lr_schedule: Option<Expr>,
    /// Decoupled weight decay coefficient (AdamW-style). `0` or absent
    /// disables. Applied as `w ← w − lr · wd · w` after the optimiser step.
    pub weight_decay: Option<Expr>,
    /// When `true`, share the Embedding table with the final Linear head's
    /// weight (transposed). Requires `Embedding(V, E)` and a trailing
    /// `Linear(E, V, ...)` with matching V and E.
    pub tied_embeddings: Option<Expr>,
    /// For `lr_schedule: plateau`: epochs without ≥1e-6 val-loss
    /// improvement before multiplying the LR by `lr_factor`.
    pub plateau_patience: Option<Expr>,
    /// Multiplier applied to the LR when the plateau guard triggers
    /// (e.g. `0.5` halves the LR). Defaults to `0.5`.
    pub lr_factor: Option<Expr>,
}

/// Multi-agent swarm definition: `swarm Name { agent: Type; size: N; ... }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmDef {
    pub name: String,
    /// The agent type that populates this swarm.
    pub agent_type: String,
    /// Number of agents in the swarm.
    pub size: Option<Expr>,
    /// Communication topology: "star", "ring", "mesh", "broadcast", "tree".
    pub topology: Option<String>,
    /// Consensus strategy: "majority", "unanimous", "weighted", "quorum".
    pub consensus: Option<String>,
    /// Transport: "local" (MechGen swarm_bus), "rmi-quic", "rmi-tcp" (rmi::distributed::transport).
    pub transport: Option<String>,
    /// Dispatch / scatter logic.
    pub on_dispatch: Option<Block>,
    /// Aggregation / gather logic.
    pub on_aggregate: Option<Block>,
    /// Failure handler.
    pub on_failure: Option<Block>,
}

// ── Data / Extend definitions ────────────────────────────────

/// `data Point(x: f64, y: f64)` or `data Tree[T] = Leaf(T) | Branch(Tree[T], Tree[T])`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub kind: DataKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataKind {
    Record(Vec<DataField>),
    Sum(Vec<DataVariant>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataField {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataVariant {
    pub name: String,
    pub fields: Vec<Type>,
}

/// `extend Type { fn method() { ... } }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendBlock {
    pub target_type: Type,
    pub items: Vec<Item>,
}
