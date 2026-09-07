//! Differentiability inference — which functions have a derivative, and where.
//!
//! MAGE is meant to be differentiable *by design*, and this is the pass that
//! makes the phrase checkable. See `DIFFERENTIABILITY.md` for the argument; the
//! short version is that "fully differentiable" is not a claim any language with
//! `if` can make, so what is claimed instead is:
//!
//! > every function reported as differentiable is differentiable **almost
//! > everywhere** on its domain, and the compiler says which of the four states
//! > it is in rather than assuming the best one.
//!
//! **Modelled on `effects.rs` deliberately.** Differentiability is a
//! propagating, inferable, declarable property with a join over the call graph
//! — structurally the same analysis as effects, down to the cycle handling. It
//! is not a new kind of pass and should not look like one.
//!
//! ## The fourth state is the point
//!
//! `Unknown` is not a grade of differentiability. It is the absence of a
//! verdict, and it exists because the alternative is answering `Smooth` for "I
//! did not look at that construct". That distinction — *the claim is untested,
//! not clean* — is the one `StatodynamicAnalysis` builds its statodynamic
//! lattice around, and the one this repository spent 2026-09-01/02 removing
//! documentation that got wrong.

use crate::ast;
use crate::effects::EffectInfer;
use crate::hir::Effect;
use std::collections::HashMap;

/// How differentiable something is.
///
/// Ordered by `rank`, and joined worst-case, the way an effect set is unioned.
/// A definite negative outranks an unknown: if one branch cannot be
/// differentiated and another was not analysed, the function cannot be
/// differentiated, and saying so is more useful than saying nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diff {
    /// Differentiable everywhere on its domain.
    Smooth,
    /// Differentiable off a set of measure zero — a kink, or a branch boundary.
    AlmostEverywhere,
    /// The pass could not determine it. Carries what it could not analyse.
    Unknown(String),
    /// Not differentiable, and here is why.
    No(String),
}

impl Diff {
    fn rank(&self) -> u8 {
        match self {
            Diff::Smooth => 0,
            Diff::AlmostEverywhere => 1,
            Diff::Unknown(_) => 2,
            Diff::No(_) => 3,
        }
    }

    /// Worst-case join. A composition is as differentiable as its least
    /// differentiable part.
    pub fn join(self, other: Diff) -> Diff {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }

    /// Does a derivative exist at all (a.e. or better)?
    pub fn is_differentiable(&self) -> bool {
        matches!(self, Diff::Smooth | Diff::AlmostEverywhere)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Diff::Smooth => "smooth",
            Diff::AlmostEverywhere => "almost everywhere",
            Diff::Unknown(_) => "unknown",
            Diff::No(_) => "not differentiable",
        }
    }

    /// The stated reason, for the two states that carry one.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Diff::Unknown(r) | Diff::No(r) => Some(r),
            _ => None,
        }
    }
}

/// Effects that mean a function's output is not a function of its inputs.
///
/// A derivative is a statement about how an output moves when an input moves.
/// If the output also depends on a clock, a socket or an entropy source, there
/// is nothing to differentiate — regardless of the arithmetic inside.
///
/// `Gpu` and `Npu` say *where* a function computes, not whether it is a
/// function, and are deliberately absent. So is `Alloc`. `Rng` is present: a
/// stochastic function has no derivative in this sense, and the
/// reparameterisation trick is a change of program, not a change of verdict.
///
/// This list is a decision, written down so it can be argued with rather than
/// discovered by reading the matcher.
pub const NON_FUNCTIONAL: &[Effect] = &[
    Effect::IO,
    Effect::FS,
    Effect::Net,
    Effect::Env,
    Effect::Time,
    Effect::Rng,
    Effect::Llm,
    Effect::Agent,
    Effect::Async,
];

pub struct DiffInfer {
    /// Inferred status per function.
    pub inferred: HashMap<String, Diff>,
    /// Inferred status per `net`, in declaration order.
    pub nets: Vec<NetDiff>,
    /// Inferred status per `train`, in declaration order.
    pub trains: Vec<TrainDiff>,
    /// Call graph: caller → callees, the same shape `effects.rs` builds.
    call_graph: HashMap<String, Vec<String>>,
    /// Cycle detection for mutually recursive functions.
    in_progress: Vec<String>,
    /// Locally-determined status, before callees are folded in.
    local: HashMap<String, Diff>,
}

impl Default for DiffInfer {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffInfer {
    pub fn new() -> Self {
        DiffInfer {
            inferred: HashMap::new(),
            nets: Vec::new(),
            trains: Vec::new(),
            call_graph: HashMap::new(),
            in_progress: Vec::new(),
            local: HashMap::new(),
        }
    }

    /// The verdict for a `net` by name, or `Unknown` if this pass never saw it.
    pub fn net_of(&self, name: &str) -> Diff {
        self.nets
            .iter()
            .find(|n| n.name == name)
            .map(|n| n.verdict.clone())
            .unwrap_or_else(|| Diff::Unknown(format!("no analysis for net `{name}`")))
    }

    /// The verdict for a `train` block by name, or `Unknown` if unseen.
    pub fn train_of(&self, name: &str) -> Diff {
        self.trains
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.verdict.clone())
            .unwrap_or_else(|| Diff::Unknown(format!("no analysis for train `{name}`")))
    }

    /// The status of a function, or `Unknown` if this pass never saw it.
    ///
    /// Not `Smooth`. A name this pass has no record of is exactly the case the
    /// fourth state exists for.
    pub fn diff_of(&self, name: &str) -> Diff {
        self.inferred
            .get(name)
            .cloned()
            .unwrap_or_else(|| Diff::Unknown(format!("no analysis for `{name}`")))
    }

    /// Every function, in a stable order, for reporting.
    pub fn all(&self) -> Vec<(String, Diff)> {
        let mut v: Vec<_> = self
            .inferred
            .iter()
            .map(|(k, d)| (k.clone(), d.clone()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }
}

// ── Types ────────────────────────────────────────────────────────────

/// Is this a type a derivative can be taken with respect to?
///
/// Floats and tensors of floats. Integers, `bool`, `str` and `char` are
/// discrete: there is no limit to take. A reference or a box is transparent.
fn type_is_continuous(ty: &ast::Type) -> bool {
    match ty {
        ast::Type::Path { segments, type_args } => {
            let name = segments.last().map(String::as_str).unwrap_or("");
            match name {
                "f32" | "f64" => true,
                // A tensor is differentiable when its element type is.
                "tensor" | "Tensor" | "Param" => {
                    type_args.first().map(type_is_continuous).unwrap_or(true)
                }
                // A container is differentiable when its contents are.
                "Vec" | "Option" => type_args.first().map(type_is_continuous).unwrap_or(false),
                _ => false,
            }
        }
        // Every wrapper is transparent: a box of floats is still floats.
        ast::Type::Reference { inner, .. }
        | ast::Type::OwnedPtr { inner }
        | ast::Type::Rc { inner }
        | ast::Type::Arc { inner }
        | ast::Type::Cow { inner }
        | ast::Type::Cell { inner }
        | ast::Type::RefCell { inner }
        | ast::Type::Mutex { inner }
        | ast::Type::RwLock { inner }
        | ast::Type::Slice { inner }
        | ast::Type::Array { inner, .. }
        | ast::Type::Vec { inner }
        | ast::Type::Option { inner } => type_is_continuous(inner),
        _ => false,
    }
}

// ── Operators ────────────────────────────────────────────────────────

/// Binary operators whose result is `bool`: a comparison has no derivative,
/// and the discreteness enters the program here rather than at the `if`.
fn is_comparison(op: &str) -> bool {
    matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||")
}

/// Operators that are smooth on their domain.
fn is_smooth_arith(op: &str) -> bool {
    matches!(op, "+" | "-" | "*" | "/")
}

/// Builtins with a kink: differentiable off a measure-zero set.
const KINKED: &[&str] = &[
    "abs", "relu", "max", "min", "clamp", "sign", "floor", "ceil", "round", "trunc",
];

/// Builtins that are smooth wherever they are defined.
const SMOOTH_BUILTINS: &[&str] = &[
    "exp", "ln", "log", "sqrt", "sin", "cos", "tan", "tanh", "sigmoid", "softmax", "gelu", "silu",
    "sum", "mean", "dot", "matmul",
];

/// Builtins that produce a discrete result.
const DISCRETE_BUILTINS: &[&str] = &[
    "len", "count", "keys", "argmax", "argmin", "index", "any", "all", "freq",
];

// ── The pass ─────────────────────────────────────────────────────────

/// Infer differentiability for every function in a module.
///
/// Takes the effect inference result rather than recomputing it: the
/// necessary condition (a differentiable function's output is a function of its
/// inputs) is *already* computed by `effects.rs`, and duplicating that
/// judgement here would be a second copy of a list that can drift — which this
/// repository has enough of.
pub fn infer(module: &ast::Module, effects: &EffectInfer) -> DiffInfer {
    let mut engine = DiffInfer::new();

    // Pass 1: local status and the call graph.
    for item in &module.items {
        if let ast::ItemKind::Function(fd) = &item.kind {
            let mut callees = Vec::new();
            collect_calls_block(&fd.body, &mut callees);
            if let Some(be) = &fd.body_expr {
                collect_calls_expr(be, &mut callees);
            }
            engine.call_graph.insert(fd.name.clone(), callees);
            engine
                .local
                .insert(fd.name.clone(), local_status(fd, effects));
        }
    }

    // Pass 2: fold callees in, worst-case, with cycle detection.
    let names: Vec<String> = engine.local.keys().cloned().collect();
    for name in names {
        let d = resolve(&mut engine, &name);
        engine.inferred.insert(name, d);
    }

    // Pass 3: the `net` DSL, where the numerical code in this language lives.
    // Nets first, because a `train` block's verdict is built on its net's.
    for item in &module.items {
        if let ast::ItemKind::Net(net) = &item.kind {
            let d = net_diff(net);
            engine.nets.push(d);
        }
    }
    for item in &module.items {
        if let ast::ItemKind::Train(td) = &item.kind {
            let d = train_diff(td, &engine.nets);
            engine.trains.push(d);
        }
    }
    engine
}

fn resolve(engine: &mut DiffInfer, name: &str) -> Diff {
    if let Some(d) = engine.inferred.get(name) {
        return d.clone();
    }
    // A recursive call contributes nothing on the way down. The fixed point is
    // the join of everything else, which is how `effects.rs` treats the same
    // situation.
    if engine.in_progress.iter().any(|n| n == name) {
        return Diff::Smooth;
    }
    engine.in_progress.push(name.to_string());

    let mut d = engine
        .local
        .get(name)
        .cloned()
        .unwrap_or_else(|| Diff::Unknown(format!("no body for `{name}`")));

    for callee in engine.call_graph.get(name).cloned().unwrap_or_default() {
        // A call to something outside this module is not assumed smooth.
        let cd = if engine.local.contains_key(&callee) {
            resolve(engine, &callee)
        } else {
            builtin_status(&callee)
        };
        d = d.join(cd);
    }

    engine.in_progress.retain(|n| n != name);
    engine.inferred.insert(name.to_string(), d.clone());
    d
}

/// A function's status from its own signature, body and effects, before callees.
fn local_status(fd: &ast::FunctionDef, effects: &EffectInfer) -> Diff {
    // The effect condition first: it disqualifies regardless of the arithmetic.
    let es = effects.effects_of(&fd.name);
    for e in NON_FUNCTIONAL {
        if es.contains(e) {
            return Diff::No(format!(
                "performs the `{e:?}` effect, so its output is not a function of its inputs"
            ));
        }
    }

    // A derivative needs something continuous to move.
    if !fd.params.iter().any(|p| type_is_continuous(&p.ty)) {
        return Diff::No("no floating-point parameter to differentiate with respect to".into());
    }
    match &fd.return_type {
        Some(t) if !type_is_continuous(t) => {
            return Diff::No("returns a discrete type".into());
        }
        None => return Diff::No("returns nothing".into()),
        _ => {}
    }

    let mut d = block_status(&fd.body);
    if let Some(be) = &fd.body_expr {
        d = d.join(expr_status(be));
    }
    d
}

fn stmt_status(s: &ast::Stmt) -> Diff {
    match s {
        ast::Stmt::Expr { expr } | ast::Stmt::Defer { expr } => expr_status(expr),
        ast::Stmt::Let { value, .. } => expr_status(value),
        // A guard's condition selects; its else-block diverges. Neither is
        // differentiated, but the boundary is real, so it is a.e. like `if`.
        ast::Stmt::Guard { else_block, .. } => {
            block_status(else_block).join(Diff::AlmostEverywhere)
        }
        ast::Stmt::Item { .. } => Diff::Smooth,
    }
}

fn block_status(b: &ast::Block) -> Diff {
    let mut d = Diff::Smooth;
    for s in &b.stmts {
        d = d.join(stmt_status(s));
    }
    // The tail expression is the block's *value*, and omitting it made every
    // one-expression function read as Smooth -- including `x > y`. Three tests
    // caught it; without them the pass would have reported a comparison as
    // differentiable, which is the exact false-positive it exists to prevent.
    if let Some(t) = &b.tail_expr {
        d = d.join(expr_status(t));
    }
    d
}

fn expr_status(e: &ast::Expr) -> Diff {
    match e {
        // A constant's derivative is zero, which is perfectly smooth.
        ast::Expr::Literal { .. } | ast::Expr::Ident { .. } => Diff::Smooth,

        ast::Expr::Binary { op, left, right } => {
            if is_comparison(op) {
                // The discreteness enters here, not at the `if` that consumes it.
                Diff::No(format!("`{op}` produces a discrete result"))
            } else if is_smooth_arith(op) {
                expr_status(left).join(expr_status(right))
            } else {
                // Bitwise, shifts, modulo: integral operations.
                Diff::No(format!("`{op}` is not defined on a continuum"))
            }
        }

        ast::Expr::Unary { op, operand } => match op.as_str() {
            "-" => expr_status(operand),
            "!" => Diff::No("`!` produces a discrete result".into()),
            _ => Diff::Unknown(format!("unary `{op}`")),
        },

        // A branch on a continuous quantity is differentiable off the boundary,
        // which is measure zero. The condition itself is *not* differentiated —
        // it selects. So a comparison in a condition is expected and does not
        // disqualify the `if`, unlike a comparison whose value is returned.
        ast::Expr::If {
            // Deliberately not consulted — see the comment below the join.
            cond: _,
            then_block,
            else_block,
        } => {
            let branches = block_status(then_block).join(
                else_block
                    .as_ref()
                    .map(block_status)
                    .unwrap_or(Diff::Smooth),
            );
            // The boundary is `AlmostEverywhere` whatever the condition is.
            // This was written as an `if` on whether the condition is a
            // comparison, with both arms returning the same value — clippy's
            // `if_same_then_else`, and a reader would reasonably assume the
            // two cases differ and go looking for how. They do not, and the
            // reason is the interesting part: a condition on a `bool` variable
            // still has a boundary, it was just drawn where the `bool` was
            // made, and that is already accounted for there.
            branches.join(Diff::AlmostEverywhere)
        }

        ast::Expr::Block { block } => block_status(block),
        ast::Expr::Return { value } => {
            value.as_ref().map(|v| expr_status(v)).unwrap_or(Diff::Smooth)
        }

        ast::Expr::Call { func, args } => {
            let mut d = Diff::Smooth;
            for a in args {
                d = d.join(expr_status(a));
            }
            // The callee's own status is folded in by `resolve`, from the call
            // graph. Here only the arguments are judged, so a call is not
            // double-counted.
            let _ = func;
            d
        }

        ast::Expr::ArrayLit { elements } | ast::Expr::TupleLit { elements } => {
            let mut d = Diff::Smooth;
            for i in elements {
                d = d.join(expr_status(i));
            }
            d
        }

        ast::Expr::Cast { expr, ty } => {
            if type_is_continuous(ty) {
                expr_status(expr)
            } else {
                Diff::No("a cast to a discrete type destroys the derivative".into())
            }
        }

        ast::Expr::For { body, .. } => block_status(body),

        // A data-dependent trip count means the number of compositions depends
        // on the value being differentiated. That is analysable and this pass
        // does not analyse it.
        ast::Expr::Loop { .. } => Diff::Unknown("`loop` — trip count not analysed".into()),
        ast::Expr::While { .. } => Diff::Unknown("`while` — trip count not analysed".into()),

        ast::Expr::Index { object, .. } | ast::Expr::FieldAccess { object, .. } => {
            expr_status(object)
        }
        ast::Expr::Assign { value, .. } => expr_status(value),

        // Everything else is unanalysed rather than assumed fine. Naming the
        // variant makes the gap actionable instead of silent.
        other => Diff::Unknown(format!("{} not analysed", variant_name(other))),
    }
}

fn builtin_status(name: &str) -> Diff {
    let bare = name.rsplit('.').next().unwrap_or(name);
    if SMOOTH_BUILTINS.contains(&bare) {
        Diff::Smooth
    } else if KINKED.contains(&bare) {
        Diff::AlmostEverywhere
    } else if DISCRETE_BUILTINS.contains(&bare) {
        Diff::No(format!("`{bare}` produces a discrete result"))
    } else {
        Diff::Unknown(format!("`{bare}` is not in the differentiability tables"))
    }
}

// ── The `net` DSL ────────────────────────────────────────────────────
//
// The pass above answers a question about `f` functions, and measured against
// this repository it reported 0 of 155 differentiable — 125 of them for want of
// a floating-point parameter. That is not the analysis being strict; it is the
// corpus. MAGE's numerical surface is `net` / `layer` / `train`, and this half
// of the pass is where the analysis has a subject.
//
// **A net is judged with respect to its parameters, not its inputs.** That is
// the difference that matters and it is not a technicality: `train` optimises
// weights, so the derivative anyone wants from a net is ∂loss/∂w. It is why
// `Embedding` is `Smooth` here — its *input* is a discrete token id, with no
// derivative at all, while its table is an ordinary dense parameter and the
// gradient that reaches it is the one training uses.
//
// **The verdicts are keyed on the surface layer type, not the opcode it lowers
// to.** `abl_bridge` maps `HardSigmoid` and `Sigmoid` onto one `Op::SIGMOID`
// and says so — "close-enough lowering" — and only one of the two has a kink.
// Reading the verdict off the opcode would report the kinked one as `Smooth`,
// which is the overclaim this whole pass exists to prevent.

/// Layer types that are smooth on their domain.
///
/// Attention is here because softmax and the matmuls around it are smooth; the
/// causal mask is a constant, not a branch on a value. The recurrent and
/// state-space cells are here because their gates are `tanh`/`sigmoid`. The
/// PEFT adapters are low-rank affine maps.
const SMOOTH_LAYERS: &[&str] = &[
    // Affine, convolution, embedding
    "Linear", "Dense", "FullyConnected", "MatMul", "Conv2D", "Conv",
    "Embedding", "Embed", "SinusoidalPE", "PositionalEncoding", "PE",
    "LearnedPE", "LearnedPositionalEmbedding", "PositionalEmbedding",
    "RotaryEmbedding", "RoPE",
    // Attention, in every variant the bridge recognises
    "Attention", "MultiHeadAttention", "Attn", "FlashAttention",
    "SlidingWindowAttention", "LongformerAttention", "LinearAttention",
    "PerformerAttention", "GroupedQueryAttention", "GQA",
    "MultiQueryAttention", "MQA", "CrossAttention", "Softmax",
    // Smooth activations
    "GELU", "Gelu", "SiLU", "Silu", "Swish", "Sigmoid", "Tanh", "Mish",
    "Softplus", "SwiGLU", "GeGLU",
    // Normalisations — all are rational functions of the batch statistics
    "LayerNorm", "BatchNorm", "RMSNorm", "RmsNorm", "GroupNorm", "InstanceNorm",
    // Averaging pools
    "AvgPool", "AveragePool", "GlobalPool", "GlobalAvgPool", "GlobalMeanPool",
    "GlobalSumPool",
    // PEFT adapters: low-rank or full affine addends
    "LoRA", "QLoRA", "DoRA", "IA3", "Adapter", "PrefixTuning", "PromptTuning",
    // Recurrent and state-space cells
    "RNNCell", "LSTMCell", "GRUCell", "RNN", "LSTM", "GRU",
    "S4Layer", "S5Layer", "MambaBlock", "H3Layer",
    // Graph layers: an affine map over aggregated neighbour features
    "GCNLayer", "GATLayer", "GraphSAGELayer", "EdgeConv",
    // One expert is a Linear; the router is not (see KINKED_LAYERS)
    "Expert",
    // Losses
    "MSE", "MseLoss", "CrossEntropy", "BCE", "BceLoss", "NLL", "NllLoss",
    "KLDiv", "KlDiv",
];

/// Layer types differentiable off a measure-zero set, and the reason each one
/// is here. Four separate reasons live in this list, and they are worth keeping
/// distinct even though the verdict is the same:
///
/// * **A kink.** `ReLU`, `LeakyReLU`, `ELU`, `SELU`, `HardSwish`,
///   `HardSigmoid`, `MaxPool`, `GlobalMaxPool`, `Huber` — one non-smooth point,
///   measure zero. `AdaptivePool` is here because the name does not say whether
///   it averages or maxes, and `AlmostEverywhere` is the correct join over both
///   readings rather than a hedge.
///
/// * **A selection boundary.** `SparseMoE` and the routers pick top-k experts.
///   The choice is piecewise constant, so the composed function is smooth
///   inside each cell and the cell boundaries are measure zero.
///
/// * **A rounding step.** `Int8Linear`, `Int4Linear`, `BitNetLinear` quantise.
///   Their derivative is zero almost everywhere — *defined, and useless*, the
///   same honest-awkward row as `floor` in `DIFFERENTIABILITY.md`. The
///   straight-through estimator is a change of program, not a change of
///   verdict.
///
/// * **A sampled mask.** `Dropout` and friends. This is the one decision here
///   that deserves an argument rather than a table entry, so it has one below.
const KINKED_LAYERS: &[&str] = &[
    "ReLU", "Relu", "LeakyReLU", "ELU", "SELU", "HardSwish", "HardSigmoid",
    "MaxPool", "GlobalMaxPool", "AdaptivePool",
    "Huber", "SmoothL1",
    "SparseMoE", "TopKRouter", "SwitchRouter", "ExpertChoiceRouter",
    "Int8Linear", "Int4Linear", "BitNetLinear",
    // Dropout: the layer computes `mask ⊙ x / (1-p)`, which is *linear* given
    // the mask, and the mask is noise drawn independently of the input. So the
    // conditional derivative — the one every AD implementation actually
    // computes — exists and is the standard object.
    //
    // It is reported one grade below `Smooth` rather than as `Smooth` because
    // the function that includes the sampling step is not a function of its
    // inputs alone, which is the same rule `NON_FUNCTIONAL` applies to `Rng`
    // for ordinary functions. Ranking it `AlmostEverywhere` says "there is a
    // derivative, and it is not unconditional". Ranking it `No` would make
    // nearly every real network non-differentiable and would be wrong about
    // what training does; ranking it `Smooth` would hide the conditioning.
    "Dropout", "Drop", "Dropout2D", "DropPath", "StochasticDepth",
];

/// The differentiability of one surface layer type.
///
/// Anything absent from both tables is `Unknown` — never `Smooth`. A layer type
/// the bridge does not recognise lowers to `Op::IDENTITY`, and an identity is
/// perfectly smooth, so reading the verdict off the lowered form would report a
/// layer nobody has ever analysed as the best case in the lattice. That is
/// precisely the silently-wrong-answer shape this repository keeps finding.
pub fn layer_type_diff(name: &str) -> Diff {
    if SMOOTH_LAYERS.contains(&name) {
        Diff::Smooth
    } else if KINKED_LAYERS.contains(&name) {
        Diff::AlmostEverywhere
    } else {
        Diff::Unknown(format!(
            "layer type `{name}` is not in the differentiability tables"
        ))
    }
}

/// The verdict for one `net`, with the working shown.
#[derive(Debug, Clone)]
pub struct NetDiff {
    pub name: String,
    /// Join over the applied layers and the composition structure.
    pub verdict: Diff,
    /// Surface layer types the lowered forward pass applies, each with its
    /// verdict and how many times it is applied — in first-application order.
    pub layers: Vec<(String, usize, Diff)>,
    /// How many layers the net declares.
    pub declared: usize,
    /// How many layer applications the lowered forward pass contains. Larger
    /// than `declared` when a layer is applied more than once (a `wrap`
    /// sandwich, a reused stage); smaller when a declared layer is never
    /// reached, in which case it contributes no gradient path and is not part
    /// of the verdict.
    pub applied: usize,
}

/// The verdict for one `train` block.
#[derive(Debug, Clone)]
pub struct TrainDiff {
    pub name: String,
    /// Join over the net, the loss and the block body.
    pub verdict: Diff,
    /// The net this block trains.
    pub net: String,
    /// The net's own verdict, before the loss is folded in.
    pub net_verdict: Diff,
    /// The loss function's verdict, and the name it was written as.
    pub loss: Option<(String, Diff)>,
}

/// Infer differentiability for one `net`.
///
/// The set of layers judged is **the lowering's own answer**, taken from
/// `abl_bridge::NetTranslation::applied_layer_types`, not re-derived here.
/// Which layers a net applies is decided by a heuristic — a `forward` block
/// that names fewer stages than the net declares means "run them all in
/// declaration order" — and a second copy of that heuristic in this file would
/// be a second thing that has to stay true. It would also mean this pass could
/// report on a program the compiler does not build, which is the failure this
/// repository has spent five sessions removing.
pub fn net_diff(net: &ast::NetDef) -> NetDiff {
    let t = crate::abl_bridge::NetTranslator::translate(net);

    // Count applications per type, keeping first-application order.
    let mut order: Vec<String> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for ty in &t.applied_layer_types {
        if !counts.contains_key(ty) {
            order.push(ty.clone());
        }
        *counts.entry(ty.clone()).or_insert(0) += 1;
    }

    let layers: Vec<(String, usize, Diff)> = order
        .iter()
        .map(|ty| (ty.clone(), counts[ty], layer_type_diff(ty)))
        .collect();

    // A net whose forward pass applies nothing lowers to the identity, and the
    // identity is smooth — which would be a true sentence and a useless
    // verdict. There is no computation here to have a derivative.
    let verdict = if layers.is_empty() {
        Diff::Unknown("the lowered forward pass applies no layer".into())
    } else {
        layers
            .iter()
            .fold(Diff::Smooth, |acc, (_, _, d)| acc.join(d.clone()))
    };

    NetDiff {
        name: net.name.clone(),
        verdict,
        layers,
        declared: net.layers.len(),
        applied: t.applied_layer_types.len(),
    }
}

/// The name a loss was written as, when it is one the tables can judge.
///
/// `loss: MSE` parses as an identifier and `loss: Huber(1.0)` as a call; both
/// name a loss. Anything else is left to `expr_status`.
fn loss_name(e: &ast::Expr) -> Option<&str> {
    match e {
        ast::Expr::Ident { name } => Some(name.as_str()),
        ast::Expr::Call { func, .. } => match func.as_ref() {
            ast::Expr::Ident { name } => Some(name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// Infer differentiability for one `train` block.
///
/// The verdict is the join of the net, the loss and the block body. The
/// optimiser is deliberately absent: it *consumes* gradients rather than
/// contributing to the function being differentiated, so `SGD` versus `Adam`
/// cannot change whether a derivative exists.
///
/// A `train` naming a net this module does not define is `Unknown`, not
/// `NotDifferentiable` — nothing was analysed, so there is no verdict to give.
pub fn train_diff(td: &ast::TrainDef, nets: &[NetDiff]) -> TrainDiff {
    let net_verdict = nets
        .iter()
        .find(|n| n.name == td.net)
        .map(|n| n.verdict.clone())
        .unwrap_or_else(|| {
            Diff::Unknown(format!("trains net `{}`, which is not in this module", td.net))
        });

    let loss = td.loss.as_ref().map(|e| match loss_name(e) {
        Some(n) => (n.to_string(), layer_type_diff(n)),
        None => ("<expression>".to_string(), expr_status(e)),
    });

    let mut verdict = net_verdict.clone();
    if let Some((_, d)) = &loss {
        verdict = verdict.join(d.clone());
    } else {
        // No loss is not a gap in the analysis — it is a training block with
        // nothing to take a gradient of.
        verdict = verdict.join(Diff::No("no loss to differentiate".into()));
    }
    verdict = verdict.join(block_status(&td.body));

    TrainDiff {
        name: td.name.clone(),
        verdict,
        net: td.net.clone(),
        net_verdict,
        loss,
    }
}

fn variant_name(e: &ast::Expr) -> &'static str {
    match e {
        ast::Expr::MethodCall { .. } => "a method call",
        ast::Expr::Closure { .. } => "a closure",
        ast::Expr::Match { .. } => "`match`",
        ast::Expr::Handle { .. } => "`handle`",
        ast::Expr::Try { .. } => "`?`",
        ast::Expr::Await { .. } => "`.await`",
        ast::Expr::Range { .. } => "a range",
        ast::Expr::Pipeline { .. } => "a pipeline",
        ast::Expr::StructLit { .. } => "a struct literal",
        ast::Expr::MapLit { .. } => "a map literal",
        _ => "this expression",
    }
}

// ── Reporting ────────────────────────────────────────────────────────
//
// In the library rather than in `main.rs`, for the reason the binary's own
// module doc gives: the reference surface should be `pub` API, and a format
// that only exists inside a CLI arm cannot be tested. Both renderings are
// deterministic — fixed order, no map iteration — so a diff of two runs is a
// diff of two programs.

/// Counts by verdict, in lattice order: smooth, a.e., unknown, not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub smooth: usize,
    pub almost_everywhere: usize,
    pub unknown: usize,
    pub not: usize,
}

impl Tally {
    pub fn add(&mut self, d: &Diff) {
        match d {
            Diff::Smooth => self.smooth += 1,
            Diff::AlmostEverywhere => self.almost_everywhere += 1,
            Diff::Unknown(_) => self.unknown += 1,
            Diff::No(_) => self.not += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.smooth + self.almost_everywhere + self.unknown + self.not
    }

    /// Subjects with a derivative, a.e. or better.
    pub fn differentiable(&self) -> usize {
        self.smooth + self.almost_everywhere
    }
}

/// Tallies for each of the three subjects the pass analyses, kept apart.
///
/// Kept apart because merging them is how "0 of 155 differentiable" would get
/// quoted about a repository whose nets are all differentiable: functions and
/// nets are different populations asking different questions, and one number
/// over both answers neither.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tallies {
    pub functions: Tally,
    pub nets: Tally,
    pub trains: Tally,
}

pub fn tally(engine: &DiffInfer) -> Tallies {
    let mut t = Tallies::default();
    for (_, d) in engine.all() {
        t.functions.add(&d);
    }
    for n in &engine.nets {
        t.nets.add(&n.verdict);
    }
    for tr in &engine.trains {
        t.trains.add(&tr.verdict);
    }
    t
}

fn verdict_cell(d: &Diff) -> String {
    match d.reason() {
        Some(r) => format!("{} — {r}", d.label()),
        None => d.label().to_string(),
    }
}

/// A human-readable, deterministic report over one module.
pub fn report(engine: &DiffInfer, path: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("// differentiability — {path}\n"));

    for n in &engine.nets {
        s.push_str(&format!(
            "net   {:<28} {}\n",
            n.name,
            verdict_cell(&n.verdict)
        ));
        for (ty, count, d) in &n.layers {
            s.push_str(&format!(
                "        {:<20} ×{:<3} {}\n",
                ty,
                count,
                verdict_cell(d)
            ));
        }
        if n.applied != n.declared {
            s.push_str(&format!(
                "        {} of {} declared layers reached by the forward pass\n",
                n.applied, n.declared
            ));
        }
    }

    for t in &engine.trains {
        s.push_str(&format!(
            "train {:<28} {}\n",
            t.name,
            verdict_cell(&t.verdict)
        ));
        s.push_str(&format!(
            "        net {:<16}     {}\n",
            t.net,
            verdict_cell(&t.net_verdict)
        ));
        match &t.loss {
            Some((name, d)) => s.push_str(&format!(
                "        loss {:<15}     {}\n",
                name,
                verdict_cell(d)
            )),
            None => s.push_str("        loss —               none declared\n"),
        }
    }

    for (name, d) in engine.all() {
        s.push_str(&format!("f     {:<28} {}\n", name, verdict_cell(&d)));
    }

    let t = tally(engine);
    for (label, c) in [
        ("nets", t.nets),
        ("trains", t.trains),
        ("functions", t.functions),
    ] {
        if c.total() > 0 {
            s.push_str(&format!(
                "{label}: {} of {} differentiable — {} smooth, {} almost everywhere, {} unknown, {} not\n",
                c.differentiable(),
                c.total(),
                c.smooth,
                c.almost_everywhere,
                c.unknown,
                c.not
            ));
        }
    }
    if t.functions.total() + t.nets.total() + t.trains.total() == 0 {
        s.push_str("nothing to analyse: no function, net or train in this module\n");
    }
    s
}

fn diff_json(d: &Diff) -> serde_json::Value {
    match d.reason() {
        Some(r) => serde_json::json!({ "status": d.label(), "reason": r }),
        None => serde_json::json!({ "status": d.label() }),
    }
}

fn tally_json(t: &Tally) -> serde_json::Value {
    serde_json::json!({
        "smooth": t.smooth,
        "almost_everywhere": t.almost_everywhere,
        "unknown": t.unknown,
        "not_differentiable": t.not,
        "total": t.total(),
        "differentiable": t.differentiable(),
    })
}

/// The same report as machine-readable JSON, for an agent that should parse
/// structure rather than scrape prose.
pub fn report_json(engine: &DiffInfer, path: &str) -> serde_json::Value {
    let t = tally(engine);
    serde_json::json!({
        "path": path,
        "nets": engine.nets.iter().map(|n| serde_json::json!({
            "name": n.name,
            "verdict": diff_json(&n.verdict),
            "declared_layers": n.declared,
            "applied_layers": n.applied,
            "layers": n.layers.iter().map(|(ty, c, d)| serde_json::json!({
                "type": ty,
                "applications": c,
                "verdict": diff_json(d),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "trains": engine.trains.iter().map(|tr| serde_json::json!({
            "name": tr.name,
            "verdict": diff_json(&tr.verdict),
            "net": tr.net,
            "net_verdict": diff_json(&tr.net_verdict),
            "loss": tr.loss.as_ref().map(|(n, d)| serde_json::json!({
                "name": n, "verdict": diff_json(d),
            })),
        })).collect::<Vec<_>>(),
        "functions": engine.all().iter().map(|(name, d)| serde_json::json!({
            "name": name,
            "verdict": diff_json(d),
        })).collect::<Vec<_>>(),
        "summary": {
            "nets": tally_json(&t.nets),
            "trains": tally_json(&t.trains),
            "functions": tally_json(&t.functions),
        },
    })
}

// ── Call collection ──────────────────────────────────────────────────
//
// Deliberately the same shape as `effects.rs::collect_calls_in_*`. Only direct
// calls by name are collected; a method call has no receiver type at this stage
// and is left to `expr_status`, which reports it as unanalysed rather than
// attributing it to the wrong function.

fn collect_calls_block(b: &ast::Block, out: &mut Vec<String>) {
    for s in &b.stmts {
        collect_calls_stmt(s, out);
    }
    if let Some(t) = &b.tail_expr {
        collect_calls_expr(t, out);
    }
}

fn collect_calls_stmt(s: &ast::Stmt, out: &mut Vec<String>) {
    match s {
        ast::Stmt::Expr { expr } | ast::Stmt::Defer { expr } => collect_calls_expr(expr, out),
        ast::Stmt::Let { value, .. } => collect_calls_expr(value, out),
        ast::Stmt::Guard { cond, else_block } => {
            collect_calls_expr(cond, out);
            collect_calls_block(else_block, out);
        }
        ast::Stmt::Item { .. } => {}
    }
}

fn collect_calls_expr(e: &ast::Expr, out: &mut Vec<String>) {
    match e {
        ast::Expr::Call { func, args } => {
            if let ast::Expr::Ident { name } = func.as_ref() {
                out.push(name.clone());
            }
            for a in args {
                collect_calls_expr(a, out);
            }
        }
        ast::Expr::Binary { left, right, .. } => {
            collect_calls_expr(left, out);
            collect_calls_expr(right, out);
        }
        ast::Expr::Unary { operand, .. } => collect_calls_expr(operand, out),
        ast::Expr::If {
            cond,
            then_block,
            else_block,
        } => {
            collect_calls_expr(cond, out);
            collect_calls_block(then_block, out);
            if let Some(b) = else_block {
                collect_calls_block(b, out);
            }
        }
        ast::Expr::Block { block } => collect_calls_block(block, out),
        ast::Expr::Return { value: Some(inner) } => collect_calls_expr(inner, out),
        ast::Expr::For { body, .. } => collect_calls_block(body, out),
        ast::Expr::ArrayLit { elements } | ast::Expr::TupleLit { elements } => {
            for i in elements {
                collect_calls_expr(i, out);
            }
        }
        ast::Expr::Cast { expr, .. } => collect_calls_expr(expr, out),
        ast::Expr::Index { object, .. } | ast::Expr::FieldAccess { object, .. } => {
            collect_calls_expr(object, out)
        }
        ast::Expr::Assign { value, .. } => collect_calls_expr(value, out),
        _ => {}
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{effects, lexer, parser};

    fn infer_src(src: &str) -> DiffInfer {
        let tokens = lexer::lex(src);
        let module = parser::parse(&tokens).expect("parse failed");
        let eff = effects::infer_effects(&module);
        infer(&module, &eff)
    }

    #[test]
    fn float_arithmetic_is_smooth() {
        let d = infer_src("f scale(x: f32, k: f32) -> f32 { x * k + 1.0 }");
        assert_eq!(d.diff_of("scale"), Diff::Smooth, "{:?}", d.diff_of("scale"));
    }

    #[test]
    fn a_branch_is_differentiable_off_the_boundary() {
        let d = infer_src("f relu(x: f32) -> f32 { ? x > 0.0 { x } : { 0.0 } }");
        assert_eq!(d.diff_of("relu"), Diff::AlmostEverywhere, "{:?}", d.diff_of("relu"));
    }

    /// The discreteness enters where the `bool` is made, not where it is used.
    #[test]
    fn returning_a_comparison_is_not_differentiable() {
        let d = infer_src("f gt(x: f32, y: f32) -> f32 { x > y }");
        assert!(!d.diff_of("gt").is_differentiable());
        assert!(d.diff_of("gt").reason().unwrap().contains('>'));
    }

    #[test]
    fn a_discrete_signature_is_refused_on_both_sides() {
        let d = infer_src("f n(a: i32, b: i32) -> i32 { a + b }");
        assert!(d.diff_of("n").reason().unwrap().contains("no floating-point parameter"));

        let d = infer_src("f trunc(x: f32) -> i32 { 0 }");
        assert!(d.diff_of("trunc").reason().unwrap().contains("discrete"));
    }

    /// The necessary condition that comes free from the effect pass: an output
    /// that also depends on a clock or a socket is not a function of its inputs,
    /// so there is nothing to differentiate regardless of the arithmetic.
    #[test]
    fn an_effectful_function_is_not_differentiable() {
        let d = infer_src("+f noisy(x: f32) -> f32 / io { println(\"hi\"); x * 2.0 }");
        assert!(!d.diff_of("noisy").is_differentiable(), "{:?}", d.diff_of("noisy"));
        assert!(d.diff_of("noisy").reason().unwrap().contains("effect"));
    }

    /// Worst-case join across the call graph: a smooth caller of a
    /// non-differentiable callee is not differentiable.
    #[test]
    fn non_differentiability_propagates_through_calls() {
        let d = infer_src(
            "f inner(x: f32) -> f32 { x > 1.0 }\n\
             f outer(x: f32) -> f32 { inner(x) * 2.0 }",
        );
        assert!(!d.diff_of("outer").is_differentiable(), "{:?}", d.diff_of("outer"));
    }

    /// **The property this pass exists for.** An unanalysed construct must not
    /// read as smooth. `while`'s trip count can depend on the value being
    /// differentiated, and this pass does not analyse that -- so it says so.
    #[test]
    fn an_unanalysed_construct_is_unknown_not_smooth() {
        let d = infer_src("f spin(x: f32) -> f32 { @w x > 1.0 { x } x }");
        let s = d.diff_of("spin");
        assert!(
            matches!(s, Diff::Unknown(_)),
            "an unanalysed loop must be Unknown, not {:?}",
            s
        );
        assert!(!s.is_differentiable(), "Unknown must not count as differentiable");
    }

    /// A name the pass never saw is Unknown, never Smooth. The default answer
    /// for "no analysis" is the absence of a verdict.
    #[test]
    fn an_unseen_name_is_unknown() {
        let d = infer_src("f id(x: f32) -> f32 { x }");
        assert!(matches!(d.diff_of("nonexistent"), Diff::Unknown(_)));
    }

    /// Join order: a definite negative outranks an unknown, because a stated
    /// reason is more useful than silence; and unknown outranks a positive,
    /// because the alternative is claiming what was not checked.
    #[test]
    fn the_join_prefers_a_stated_negative_then_unknown() {
        let no = Diff::No("r".into());
        let unk = Diff::Unknown("u".into());
        assert_eq!(unk.clone().join(no.clone()), no);
        assert_eq!(no.clone().join(unk.clone()), no);
        assert_eq!(Diff::Smooth.join(unk.clone()), unk);
        assert_eq!(Diff::AlmostEverywhere.join(Diff::Smooth), Diff::AlmostEverywhere);
    }

    // ── The `net` DSL ────────────────────────────────────────────────

    #[test]
    fn a_linear_stack_is_smooth() {
        let d = infer_src(
            "net Affine {\n\
                 layer fc1: Linear(3, 8);\n\
                 layer fc2: Linear(8, 1);\n\
                 forward { fc1 }\n\
             }",
        );
        assert_eq!(d.net_of("Affine"), Diff::Smooth, "{:?}", d.net_of("Affine"));
    }

    /// One `ReLU` costs the whole net a grade, and only a grade.
    #[test]
    fn a_relu_in_the_stack_is_almost_everywhere() {
        let d = infer_src(
            "net Mlp {\n\
                 layer fc1: Linear(3, 8);\n\
                 layer act: ReLU;\n\
                 layer fc2: Linear(8, 1);\n\
                 forward { fc1 }\n\
             }",
        );
        assert_eq!(d.net_of("Mlp"), Diff::AlmostEverywhere, "{:?}", d.net_of("Mlp"));
    }

    /// **The property this half of the pass exists for.** A layer type nothing
    /// recognises lowers to `Op::IDENTITY`, and an identity is smooth — so a
    /// verdict read off the lowered form would report a layer no one has ever
    /// analysed as the best state in the lattice.
    #[test]
    fn an_unrecognised_layer_is_unknown_not_smooth() {
        let d = infer_src(
            "net Custom {\n\
                 layer fc: Linear(3, 8);\n\
                 layer mystery: MyFancyLayer;\n\
                 forward { fc }\n\
             }",
        );
        let v = d.net_of("Custom");
        assert!(matches!(v, Diff::Unknown(_)), "{v:?}");
        assert!(v.reason().unwrap().contains("MyFancyLayer"), "{v:?}");
        assert!(!v.is_differentiable(), "Unknown must not count as differentiable");
    }

    /// The verdict is keyed on the surface type, not the opcode. `HardSigmoid`
    /// and `Sigmoid` are one `Op::SIGMOID` in the bridge — "close-enough
    /// lowering", it says so — and only one of them has a kink. Reading the
    /// verdict off the opcode reports the kinked one as `Smooth`.
    #[test]
    fn a_lossy_lowering_does_not_upgrade_the_verdict() {
        let smooth = infer_src("net S { layer a: Sigmoid; forward { a } }");
        assert_eq!(smooth.net_of("S"), Diff::Smooth);

        let kinked = infer_src("net H { layer a: HardSigmoid; forward { a } }");
        assert_eq!(
            kinked.net_of("H"),
            Diff::AlmostEverywhere,
            "HardSigmoid lowers to Op::SIGMOID and must not inherit its verdict"
        );
    }

    /// A quantised linear rounds, and rounding's derivative is zero almost
    /// everywhere — *defined, and useless*, the same honest-awkward row as
    /// `floor`. The straight-through estimator is a different program.
    #[test]
    fn a_quantised_layer_is_not_smooth() {
        let d = infer_src("net Q { layer fc: Int8Linear(4, 4); forward { fc } }");
        assert_eq!(d.net_of("Q"), Diff::AlmostEverywhere);
    }

    /// Dropout's derivative exists *given the sampled mask*, which is the one
    /// every AD implementation computes. It is a grade below `Smooth` because
    /// the function including the sampling is not a function of its inputs
    /// alone — the same rule `NON_FUNCTIONAL` applies to `Rng`.
    #[test]
    fn dropout_is_conditional_not_smooth() {
        let d = infer_src(
            "net D { layer fc: Linear(4, 4); layer drop: Dropout(0.1); forward { fc } }",
        );
        assert_eq!(d.net_of("D"), Diff::AlmostEverywhere);
    }

    /// A net with nothing in it lowers to the identity, and the identity is
    /// smooth. Reporting that would be a true sentence and a useless verdict:
    /// there is no computation here to have a derivative.
    #[test]
    fn a_net_that_applies_nothing_is_unknown() {
        let d = infer_src("net Empty { forward { } }");
        assert!(matches!(d.net_of("Empty"), Diff::Unknown(_)));
    }

    /// The set of layers judged is the lowering's own answer. `forward { fc1 }`
    /// names one layer and means "run all three in declaration order" — so the
    /// `ReLU` the forward block never mentions still costs the verdict.
    #[test]
    fn the_declaration_order_fallback_is_what_gets_judged() {
        let d = infer_src(
            "net Fallback {\n\
                 layer fc1: Linear(3, 8);\n\
                 layer act: ReLU;\n\
                 layer fc2: Linear(8, 1);\n\
                 forward { fc1 }\n\
             }",
        );
        let n = d.nets.iter().find(|n| n.name == "Fallback").unwrap();
        assert_eq!(n.applied, 3, "all three layers lower, not just the named one");
        assert_eq!(n.declared, 3);
        assert_eq!(n.verdict, Diff::AlmostEverywhere);
    }

    // ── `train` ──────────────────────────────────────────────────────

    #[test]
    fn a_train_block_joins_its_net_and_its_loss() {
        let d = infer_src(
            "net Affine { layer fc: Linear(3, 1); forward { fc } }\n\
             train Fit { net: Affine; optimizer: SGD(0.05); loss: MSE; epochs: 10; }",
        );
        assert_eq!(d.train_of("Fit"), Diff::Smooth, "{:?}", d.train_of("Fit"));

        let d = infer_src(
            "net Affine { layer fc: Linear(3, 1); forward { fc } }\n\
             train Fit { net: Affine; optimizer: SGD(0.05); loss: Huber; epochs: 10; }",
        );
        assert_eq!(d.train_of("Fit"), Diff::AlmostEverywhere);
    }

    /// The optimiser consumes gradients rather than contributing to the
    /// function being differentiated, so it cannot change the verdict.
    #[test]
    fn the_optimiser_does_not_change_the_verdict() {
        let sgd = infer_src(
            "net A { layer fc: Linear(3, 1); forward { fc } }\n\
             train T { net: A; optimizer: SGD(0.05); loss: MSE; epochs: 1; }",
        );
        let adam = infer_src(
            "net A { layer fc: Linear(3, 1); forward { fc } }\n\
             train T { net: A; optimizer: Adam(0.001); loss: MSE; epochs: 1; }",
        );
        assert_eq!(sgd.train_of("T"), adam.train_of("T"));
    }

    /// A `train` naming a net this module does not define analysed nothing, so
    /// it has no verdict — not a negative one.
    #[test]
    fn a_train_naming_a_missing_net_is_unknown() {
        let d = infer_src("train Fit { net: Nowhere; optimizer: SGD(0.1); loss: MSE; epochs: 1; }");
        let v = d.train_of("Fit");
        assert!(matches!(v, Diff::Unknown(_)), "{v:?}");
        assert!(v.reason().unwrap().contains("Nowhere"));
    }

    // ── Reporting ────────────────────────────────────────────────────

    /// The summary lines are a **format contract**, not prose:
    /// `scripts/measure-differentiability.sh` parses them to aggregate the
    /// corpus figures quoted in `DIFFERENTIABILITY.md`. Changing the wording
    /// silently would leave the script matching nothing and reporting zero —
    /// a checker that stops reaching its subject, which is the shape this
    /// repository keeps finding. Change the line and this test tells you to
    /// change the script.
    #[test]
    fn the_summary_line_shape_is_a_contract() {
        let d = infer_src(
            "net A { layer fc: Linear(3, 1); layer r: ReLU; forward { fc } }\n\
             train T { net: A; optimizer: SGD(0.1); loss: MSE; epochs: 1; }\n\
             f id(x: f32) -> f32 { x }",
        );
        let r = report(&d, "t.mg");
        assert!(
            r.contains("nets: 1 of 1 differentiable — 0 smooth, 1 almost everywhere, 0 unknown, 0 not"),
            "{r}"
        );
        assert!(
            r.contains("trains: 1 of 1 differentiable — 0 smooth, 1 almost everywhere, 0 unknown, 0 not"),
            "{r}"
        );
        assert!(
            r.contains("functions: 1 of 1 differentiable — 1 smooth, 0 almost everywhere, 0 unknown, 0 not"),
            "{r}"
        );
    }

    #[test]
    fn the_report_is_deterministic() {
        let d = infer_src(
            "net A { layer fc: Linear(3, 1); forward { fc } }\n\
             f b(x: f32) -> f32 { x }\n\
             f a(x: f32) -> f32 { x }",
        );
        assert_eq!(report(&d, "t.mg"), report(&d, "t.mg"));
        let j = report_json(&d, "t.mg");
        assert_eq!(j["summary"]["nets"]["smooth"], 1);
        assert_eq!(j["nets"][0]["layers"][0]["type"], "Linear");
    }

    /// Mutual recursion must terminate rather than blow the stack.
    #[test]
    fn mutual_recursion_terminates() {
        let d = infer_src(
            "f a(x: f32) -> f32 { b(x) }\n\
             f b(x: f32) -> f32 { a(x) }",
        );
        assert!(d.inferred.contains_key("a") && d.inferred.contains_key("b"));
    }
}
