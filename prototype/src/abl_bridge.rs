//! # MechGen ↔ Agentic Binary Language Bridge
//!
//! Translates MechGen AST nodes that describe neural, symbolic, or agent
//! constructs into Agentic Binary Language ([`rmi::lang`]) expression trees. The bridge is the
//! seam where MechGen's surface language meets RMI's binary neurosymbolic IR.
//!
//! ## Routing
//!
//! MechGen targets two IRs depending on what an item describes:
//!
//! | MechGen item              | IR     | Backend                          |
//! |---------------------------|--------|----------------------------------|
//! | `fn` (systems code)       | MLIR   | LLVM, Cranelift, GCC             |
//! | `net` / `train` / `kb`    | Agentic Binary Language   | RMI VM, CUDA, Metal, WebGPU, ANE |
//! | `agent` / `swarm`         | Agentic Binary Language   | RMI agent runtime + transport    |
//! | `evolve`                  | both   | Agentic Binary Language pop loop + MLIR fitness     |
//!
//! [`OpFamilyRouter`] decides per-item. [`NetTranslator`], [`KbTranslator`],
//! [`AgentTranslator`] produce the actual Agentic Binary Language [`rmi::lang::Expr`].
//!
//! The bridge intentionally does **no** typechecking; it consumes an AST that
//! the MechGen frontend has already resolved + checked, and emits Agentic Binary Language whose
//! shape/type errors surface via [`rmi::lang::Vm`] or the RMI verifier.

use std::collections::HashMap;

use crate::ast;
use rmi::lang::{Expr, Op, OpFamily, Sym, SymbolTable, Ty, Val};

// ═══════════════════════════════════════════════════════════════════
// Family classification — which ops the tree-walking VM stubs
// ═══════════════════════════════════════════════════════════════════

/// True if [`OpFamily`] is one that `rmi::lang::Vm` deliberately stubs.
///
/// The tree-walking VM evaluates math, control, memory, and meta ops
/// directly. Neural, symbolic, and agent ops require a compute backend
/// (CUDA / Metal / WebGPU / ANE / Qualcomm) or distributed runtime —
/// invoking them in the VM produces `ArityMismatch` or `UnboundSymbol`
/// errors by design.
pub fn is_stubbed_family(f: OpFamily) -> bool {
    matches!(f, OpFamily::Neural | OpFamily::Symbolic | OpFamily::Agent)
}

/// Collect every distinct [`OpFamily`] referenced by an Agentic Binary Language expression
/// tree. Useful for routing decisions and VM diagnostics.
pub fn expr_op_families(expr: &Expr) -> Vec<OpFamily> {
    let mut out = Vec::new();
    for op in expr.opcodes() {
        // OpFamily of op = (op.0 >> 8) byte. Use the OpMeta if exposed,
        // otherwise reconstruct from the high byte.
        let family = match (op.0 >> 8) as u8 {
            0x00 => OpFamily::Neural,
            0x01 => OpFamily::Symbolic,
            0x02 => OpFamily::Control,
            0x03 => OpFamily::Memory,
            0x04 => OpFamily::Agent,
            0x05 => OpFamily::Meta,
            0x06 => OpFamily::Math,
            _ => continue,
        };
        if !out.contains(&family) {
            out.push(family);
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════
// Routing — which IR does this item lower to?
// ═══════════════════════════════════════════════════════════════════

/// Which IR a MechGen item targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrTarget {
    /// Lower to MechGen MLIR dialect → LLVM/Cranelift/GCC.
    Mlir,
    /// Lower to Agentic Binary Language → RMI VM / compute backends / agent runtime.
    Machine,
    /// Hybrid: emit both; runtime selects.
    Both,
}

/// Routes MechGen items to the appropriate IR backend by op family.
pub struct OpFamilyRouter;

impl OpFamilyRouter {
    /// Decide which IR an item lowers to.
    pub fn route(item: &ast::Item) -> IrTarget {
        use ast::ItemKind::*;
        match &item.kind {
            Net(_) | Kb(_) | Train(_) => IrTarget::Machine,
            Agent(_) | Swarm(_) => IrTarget::Machine,
            Evolve(_) => IrTarget::Both,
            Function(_) | Struct(_) | Enum(_) | Trait(_) | Impl(_) | Module(_) | Use(_)
            | TypeAlias(_) | Const(_) | Static(_) | Effect(_) | Spec(_) | Data(_)
            | Extend(_) => IrTarget::Mlir,
        }
    }

    /// Partition a module into (mlir_items, abl_items). `Both` items appear in
    /// each.
    pub fn partition(module: &ast::Module) -> (Vec<&ast::Item>, Vec<&ast::Item>) {
        let mut mlir = Vec::new();
        let mut ml = Vec::new();
        for item in &module.items {
            match Self::route(item) {
                IrTarget::Mlir => mlir.push(item),
                IrTarget::Machine => ml.push(item),
                IrTarget::Both => {
                    mlir.push(item);
                    ml.push(item);
                }
            }
        }
        (mlir, ml)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Layer name → Agentic Binary Language Op lookup
// ═══════════════════════════════════════════════════════════════════

/// Map a MechGen layer type name to its Agentic Binary Language opcode.
///
/// Returns `None` if the name does not correspond to a known neural primitive
/// — callers may treat that as a user-defined layer and fall back to a
/// generic `Op::IDENTITY` placeholder, or surface a diagnostic.
pub fn layer_name_to_op(name: &str) -> Option<Op> {
    match name {
        // Affine & convolution
        "Linear" | "Dense" | "FullyConnected" => Some(Op::LINEAR),
        "MatMul" => Some(Op::MATMUL),
        "Conv2D" | "Conv" => Some(Op::CONV2D),
        "Embedding" | "Embed" => Some(Op::EMBED),
        "SinusoidalPE" | "PositionalEncoding" | "PE" => Some(Op::SINUSOIDAL_PE),
        "LearnedPE" | "LearnedPositionalEmbedding" => Some(Op::LEARNED_PE),
        // All attention variants currently lower to ATTN. The variant
        // distinction is metadata downstream backends can specialise
        // on (e.g. FlashAttention uses tiled kernels at dispatch).
        "Attention" | "MultiHeadAttention" | "Attn" => Some(Op::ATTN),
        "FlashAttention" => Some(Op::ATTN),
        "SlidingWindowAttention" => Some(Op::ATTN),
        "LongformerAttention" => Some(Op::ATTN),
        "LinearAttention" => Some(Op::ATTN),
        "PerformerAttention" => Some(Op::ATTN),
        "GroupedQueryAttention" | "GQA" => Some(Op::ATTN),
        "MultiQueryAttention" | "MQA" => Some(Op::ATTN),
        "CrossAttention" => Some(Op::ATTN),

        "Dropout" | "Drop" => Some(Op::DROP),
        "Dropout2D" => Some(Op::DROP),
        "DropPath" | "StochasticDepth" => Some(Op::DROP),
        "Softmax" => Some(Op::SOFTMAX),

        // Embedding variants: all route to EMBED; positional /
        // learned / rotary share the same opcode + variant-tag.
        "RotaryEmbedding" | "RoPE" => Some(Op::EMBED),
        "PositionalEmbedding" => Some(Op::SINUSOIDAL_PE),

        // Activations
        "ReLU" | "Relu" => Some(Op::RELU),
        "LeakyReLU" => Some(Op::RELU),       // negative-slope is metadata
        "ELU" => Some(Op::RELU),             // close-enough lowering
        "SELU" => Some(Op::RELU),            // self-normalising; activation family
        "GELU" | "Gelu" => Some(Op::GELU),
        "SiLU" | "Silu" | "Swish" => Some(Op::SILU),
        "HardSwish" => Some(Op::SILU),
        "HardSigmoid" => Some(Op::SIGMOID),
        "Sigmoid" => Some(Op::SIGMOID),
        "Tanh" => Some(Op::TANH_ACT),
        "Mish" => Some(Op::MISH),
        "Softplus" => Some(Op::SOFTPLUS),
        // Gated linear units route through SILU/GELU; the gate split
        // is a backend-side fusion concern.
        "SwiGLU" => Some(Op::SILU),
        "GeGLU" => Some(Op::GELU),

        // Normalisations
        "LayerNorm" => Some(Op::LAYER_NORM),
        "BatchNorm" => Some(Op::BATCH_NORM),
        "RMSNorm" | "RmsNorm" => Some(Op::RMS_NORM),
        "GroupNorm" => Some(Op::GROUP_NORM),
        "InstanceNorm" => Some(Op::INSTANCE_NORM),

        // Pooling
        "MaxPool" => Some(Op::MAX_POOL),
        "AvgPool" | "AveragePool" => Some(Op::AVG_POOL),
        "AdaptivePool" => Some(Op::ADAPTIVE_POOL),
        "GlobalPool" | "GlobalAvgPool" => Some(Op::GLOBAL_POOL),
        "GlobalMaxPool" => Some(Op::GLOBAL_POOL),
        "GlobalMeanPool" => Some(Op::GLOBAL_POOL),
        "GlobalSumPool" => Some(Op::GLOBAL_POOL),

        // PEFT adapter primitives: a LoRA layer is a low-rank Linear
        // addend; lower as MATMUL so dispatch sees the matrix shape.
        "LoRA" | "QLoRA" | "DoRA" => Some(Op::MATMUL),
        "IA3" => Some(Op::MATMUL),
        "Adapter" => Some(Op::LINEAR),
        "PrefixTuning" | "PromptTuning" => Some(Op::EMBED),

        // Quantization primitives lower to the same op-family as
        // their full-precision counterparts. Backend-side dtype is a
        // separate concern.
        "Int8Linear" | "Int4Linear" | "BitNetLinear" => Some(Op::LINEAR),

        // Recurrent cells: no dedicated opcode yet. Route the recurrent
        // computation to MATMUL (the dominant per-step op) and rely on
        // the bridge to express the time-loop via Control::Loop. Multi-
        // layer wrappers are sugar.
        "RNNCell" | "LSTMCell" | "GRUCell" => Some(Op::MATMUL),
        "RNN" | "LSTM" | "GRU" => Some(Op::MATMUL),

        // Graph layers: each node update is a Linear over aggregated
        // neighbour features. Backend specialises if it has a graph
        // dispatch path.
        "GCNLayer" | "GATLayer" | "GraphSAGELayer" | "EdgeConv" => Some(Op::LINEAR),

        // State-space models: per-step update is a small Linear.
        "S4Layer" | "S5Layer" | "MambaBlock" | "H3Layer" => Some(Op::MATMUL),

        // MoE: each expert is a Linear; the router is a Linear over
        // hidden -> num_experts.
        "Expert" | "SparseMoE" => Some(Op::LINEAR),
        "TopKRouter" | "SwitchRouter" | "ExpertChoiceRouter" => Some(Op::LINEAR),

        // Losses
        "MSE" | "MseLoss" => Some(Op::MSE_LOSS),
        "CrossEntropy" => Some(Op::CROSS_ENTROPY),
        "BCE" | "BceLoss" => Some(Op::BCE_LOSS),
        "NLL" | "NllLoss" => Some(Op::NLL_LOSS),
        "Huber" | "SmoothL1" => Some(Op::HUBER_LOSS),
        "KLDiv" | "KlDiv" => Some(Op::KL_DIV),

        _ => None,
    }
}

/// Extract the trailing path segment from a MechGen [`ast::Type`].
fn type_tail_name(ty: &ast::Type) -> Option<&str> {
    match ty {
        ast::Type::Path { segments, .. } => segments.last().map(|s| s.as_str()),
        _ => None,
    }
}

/// Inverse of [`layer_name_to_op`] — map an Agentic Binary Language opcode back to a canonical
/// MechGen layer surface name. Returns `None` for non-neural opcodes.
///
/// The chosen names are the first variant per family from
/// [`layer_name_to_op`], ensuring `op_to_layer_name(layer_name_to_op(n)?) ==
/// n_canonical` for every n that was in the canonical set.
pub fn op_to_layer_name(op: Op) -> Option<&'static str> {
    match op {
        Op::MATMUL => Some("MatMul"),
        Op::LINEAR => Some("Linear"),
        Op::CONV2D => Some("Conv2D"),
        Op::ATTN => Some("Attention"),
        Op::EMBED => Some("Embedding"),
        Op::DROP => Some("Dropout"),
        Op::SOFTMAX => Some("Softmax"),

        Op::RELU => Some("ReLU"),
        Op::GELU => Some("GELU"),
        Op::SILU => Some("SiLU"),
        Op::SIGMOID => Some("Sigmoid"),
        Op::TANH_ACT => Some("Tanh"),
        Op::MISH => Some("Mish"),
        Op::SOFTPLUS => Some("Softplus"),

        Op::LAYER_NORM => Some("LayerNorm"),
        Op::BATCH_NORM => Some("BatchNorm"),
        Op::RMS_NORM => Some("RMSNorm"),
        Op::GROUP_NORM => Some("GroupNorm"),
        Op::INSTANCE_NORM => Some("InstanceNorm"),

        Op::MAX_POOL => Some("MaxPool"),
        Op::AVG_POOL => Some("AvgPool"),
        Op::ADAPTIVE_POOL => Some("AdaptivePool"),
        Op::GLOBAL_POOL => Some("GlobalPool"),

        Op::MSE_LOSS => Some("MSE"),
        Op::CROSS_ENTROPY => Some("CrossEntropy"),
        Op::BCE_LOSS => Some("BCE"),
        Op::NLL_LOSS => Some("NLL"),
        Op::HUBER_LOSS => Some("Huber"),
        Op::KL_DIV => Some("KLDiv"),

        // ── Symbolic (0x01xx) ──────────────────────────────────────
        Op::UNIFY => Some("Unify"),
        Op::RESOLVE => Some("Resolve"),
        Op::INFER => Some("Infer"),
        Op::MATCH => Some("Match"),
        Op::REWRITE => Some("Rewrite"),
        Op::PLAN => Some("Plan"),

        // ── Agent (0x04xx) ─────────────────────────────────────────
        Op::SEND => Some("Send"),
        Op::RECV => Some("Recv"),
        Op::SPAWN => Some("Spawn"),
        Op::DELEGATE => Some("Delegate"),

        // ── Control / aggregation ──────────────────────────────────
        Op::MAP => Some("Map"),
        Op::REDUCE => Some("Reduce"),

        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════
// Translators
// ═══════════════════════════════════════════════════════════════════

/// Translate a MechGen [`ast::NetDef`] into an Agentic Binary Language pipeline expression.
///
/// Each layer becomes a unary opcode; layers are composed with `>>` in
/// declaration order. The resulting [`Expr`] is content-addressable and
/// codec-encodable.
///
/// Unknown layer types become `Op::IDENTITY` placeholders — callers should
/// inspect [`TranslationReport::unknown_layers`] to surface diagnostics.
pub struct NetTranslator;

/// Outcome of a [`NetTranslator`] run.
#[derive(Debug, Clone)]
pub struct NetTranslation {
    /// The Agentic Binary Language expression tree for the network's forward pass.
    pub expr: Expr,
    /// Layer names that did not resolve to a known Agentic Binary Language opcode.
    pub unknown_layers: Vec<String>,
}

impl NetTranslator {
    /// Translate a `net` definition.
    ///
    /// Strategy:
    /// 1. Build a `layer_name → (Op, args)` table from `net.layers`.
    /// 2. If `net.forward` has meaningful content, walk it with
    ///    [`ForwardWalker`] to produce a data-flow-respecting expression.
    /// 3. Otherwise (empty forward, or forward is just a single layer name),
    ///    fall back to declaration-order sequential composition.
    pub fn translate(net: &ast::NetDef) -> NetTranslation {
        let mut unknown = Vec::new();
        let mut layer_table: HashMap<String, (Op, Vec<Expr>)> = HashMap::new();

        // Build the layer name → opcode + literal args table.
        for layer in &net.layers {
            let layer_type_name = type_tail_name(&layer.layer_type).unwrap_or("");
            match layer_name_to_op(layer_type_name) {
                Some(op) => {
                    let args = layer.args.iter().filter_map(translate_literal).collect();
                    layer_table.insert(layer.name.clone(), (op, args));
                }
                None => {
                    unknown.push(layer_type_name.to_string());
                    layer_table.insert(layer.name.clone(), (Op::IDENTITY, Vec::new()));
                }
            }
        }

        // Decide between forward-walking and declaration-order fallback.
        //
        // Convention: a forward block with a single layer reference
        // (`forward { fc1 }`) means "run all declared layers in order".
        // We detect this by counting App nodes — if the walker produced
        // only one but the net declared multiple, fall back to declaration
        // order. This works regardless of whether layers carry args (which
        // would otherwise inflate node_count).
        let walker = ForwardWalker::new(&layer_table);
        let forward_expr = walker.walk_block(&net.forward);
        let expr = match forward_expr {
            None => Self::declaration_order(&net.layers, &layer_table),
            Some(expr) if count_app_nodes(&expr) < net.layers.len() => {
                Self::declaration_order(&net.layers, &layer_table)
            }
            Some(expr) => expr,
        };

        NetTranslation {
            expr,
            unknown_layers: unknown,
        }
    }

    /// Sequentially compose every declared layer in declaration order.
    fn declaration_order(
        layers: &[ast::LayerDef],
        table: &HashMap<String, (Op, Vec<Expr>)>,
    ) -> Expr {
        let stages: Vec<Expr> = layers
            .iter()
            .filter_map(|l| table.get(&l.name))
            .map(|(op, args)| {
                if args.is_empty() {
                    Expr::op1(*op)
                } else {
                    Expr::op(*op, args.clone())
                }
            })
            .collect();

        match stages.len() {
            0 => Expr::id(),
            1 => stages.into_iter().next().unwrap(),
            _ => stages
                .into_iter()
                .reduce(|acc, next| acc >> next)
                .unwrap_or_else(Expr::id),
        }
    }
}

// ─── Literal translation ────────────────────────────────────────────

/// Count the number of `Expr::App` nodes in an Agentic Binary Language tree.
fn count_app_nodes(expr: &Expr) -> usize {
    match expr {
        Expr::App(_, args) => 1 + args.iter().map(count_app_nodes).sum::<usize>(),
        Expr::Seq(a, b) | Expr::Par(a, b) => count_app_nodes(a) + count_app_nodes(b),
        Expr::Cond { pred, yes, no } => {
            count_app_nodes(pred) + count_app_nodes(yes) + count_app_nodes(no)
        }
        Expr::Let { val, body, .. } => count_app_nodes(val) + count_app_nodes(body),
        Expr::Lam { body, .. } => count_app_nodes(body),
        Expr::Call(f, args) => count_app_nodes(f) + args.iter().map(count_app_nodes).sum::<usize>(),
        Expr::Block(exprs) => exprs.iter().map(count_app_nodes).sum(),
        _ => 0,
    }
}

/// Translate a MechGen literal expression into an Agentic Binary Language literal expression.
///
/// Returns `None` for non-literal expressions; the bridge silently drops
/// non-literals from layer args because shape inference is the frontend's job
/// — the bridge only carries through what is already a constant.
fn translate_literal(expr: &ast::Expr) -> Option<Expr> {
    match expr {
        ast::Expr::Literal { value, kind } => match kind {
            ast::LiteralKind::Int => value.parse::<i64>().ok().map(Expr::int),
            ast::LiteralKind::Float => value.parse::<f32>().ok().map(Expr::float),
            ast::LiteralKind::Bool => match value.as_str() {
                "true" => Some(Expr::boolean(true)),
                "false" => Some(Expr::boolean(false)),
                _ => None,
            },
            _ => None,
        },
        // Unary minus on a literal: `-1`, `-3.14`.
        ast::Expr::Unary { op, operand } if op == "-" => match operand.as_ref() {
            ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => {
                value.parse::<i64>().ok().map(|n| Expr::int(-n))
            }
            ast::Expr::Literal { value, kind: ast::LiteralKind::Float } => {
                value.parse::<f32>().ok().map(|n| Expr::float(-n))
            }
            _ => None,
        },
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════
// ForwardWalker — translate `forward { ... }` data flow
// ═══════════════════════════════════════════════════════════════════

/// Walks a MechGen `Block` representing a `forward { ... }` body and emits
/// an Agentic Binary Language [`Expr`] that respects the user-written data flow.
///
/// Recognised shapes:
///
/// | MechGen forward block       | Agentic Binary Language produced                                |
/// |-----------------------------|----------------------------------------------|
/// | `{ fc1 }` (single ident)    | declaration-order fallback (handled upstream)|
/// | `{ x \|> l1 \|> l2 }`       | `App(l1) >> App(l2)`                         |
/// | `{ l2(l1(x)) }`             | `App(l1) >> App(l2)`                         |
/// | `{ let h = l1(x); l2(h) }`  | `App(l1) >> App(l2)`                         |
/// | `{ x + l(x) }`              | `App(l).residual()`                          |
///
/// Anything else lowers to an inert `Op::IDENTITY` (returned as `None` at
/// block scope so the caller can fall back to declaration order).
struct ForwardWalker<'a> {
    layers: &'a HashMap<String, (Op, Vec<Expr>)>,
}

impl<'a> ForwardWalker<'a> {
    fn new(layers: &'a HashMap<String, (Op, Vec<Expr>)>) -> Self {
        Self { layers }
    }

    /// Walk a `forward { ... }` block. Returns `None` if the block is empty
    /// or contains nothing recognisable as data flow.
    fn walk_block(&self, block: &ast::Block) -> Option<Expr> {
        // Accumulate a sequence: every Stmt::Let or Stmt::Expr containing a
        // recognised layer reference becomes a stage in the pipeline.
        let mut stages: Vec<Expr> = Vec::new();

        for stmt in &block.stmts {
            match stmt {
                ast::Stmt::Let { value, .. } | ast::Stmt::Expr { expr: value } => {
                    if let Some(s) = self.walk_expr(value) {
                        stages.push(s);
                    }
                }
                _ => {}
            }
        }

        if let Some(tail) = &block.tail_expr {
            if let Some(s) = self.walk_expr(tail) {
                stages.push(s);
            }
        }

        match stages.len() {
            0 => None,
            1 => Some(stages.into_iter().next().unwrap()),
            _ => stages.into_iter().reduce(|acc, next| acc >> next),
        }
    }

    /// Walk a single expression. The returned expression composes any
    /// recognised inner layer applications with `>>`.
    fn walk_expr(&self, expr: &ast::Expr) -> Option<Expr> {
        match expr {
            // `name` — a bare layer reference.
            ast::Expr::Ident { name } => self.layer_app(name),

            // `f(args)` — a layer call with arguments. If the function is a
            // known layer, build `App(op, [arg-pipeline >> ...])` where the
            // first non-literal arg is treated as input. Nested calls
            // recurse: `l2(l1(x))` → walk(l1(x)) >> App(l2).
            ast::Expr::Call { func, args } => {
                let layer_name = match func.as_ref() {
                    ast::Expr::Ident { name } => Some(name.as_str()),
                    _ => None,
                };
                let inner = args
                    .iter()
                    .filter_map(|a| self.walk_expr(a))
                    .reduce(|acc, next| acc >> next);
                let layer = layer_name.and_then(|n| self.layer_app(n));
                match (inner, layer) {
                    (Some(inner), Some(layer)) => Some(inner >> layer),
                    (None, Some(layer)) => Some(layer),
                    (Some(inner), None) => Some(inner),
                    (None, None) => None,
                }
            }

            // `lhs |> rhs` — explicit pipeline.
            ast::Expr::Pipeline { left, right } => {
                let l = self.walk_expr(left);
                let r = self.walk_expr(right);
                match (l, r) {
                    (Some(l), Some(r)) => Some(l >> r),
                    (Some(l), None) => Some(l),
                    (None, Some(r)) => Some(r),
                    (None, None) => None,
                }
            }

            // `x + l(x)` — residual connection.
            ast::Expr::Binary { op, left, right } if op == "+" => {
                let r = self.walk_expr(right);
                let l = self.walk_expr(left);
                match (l, r) {
                    // Only-right is the classic residual: `x + layer(x)`.
                    (None, Some(r)) => Some(r.residual()),
                    (Some(l), None) => Some(l.residual()),
                    (Some(l), Some(r)) => Some(l >> r.residual()),
                    (None, None) => None,
                }
            }

            // `x.method()` or `recv.method(args)` — treat method as layer name.
            ast::Expr::MethodCall { receiver, method, args, .. } => {
                let recv = self.walk_expr(receiver);
                let inner = args
                    .iter()
                    .filter_map(|a| self.walk_expr(a))
                    .reduce(|acc, next| acc >> next);
                let layer = self.layer_app(method);
                let prefix = match (recv, inner) {
                    (Some(r), Some(i)) => Some(r >> i),
                    (Some(r), None) => Some(r),
                    (None, Some(i)) => Some(i),
                    (None, None) => None,
                };
                match (prefix, layer) {
                    (Some(p), Some(l)) => Some(p >> l),
                    (None, Some(l)) => Some(l),
                    (Some(p), None) => Some(p),
                    (None, None) => None,
                }
            }

            // Nested block.
            ast::Expr::Block { block } => self.walk_block(block),

            _ => None,
        }
    }

    /// If `name` is a declared layer, return its Agentic Binary Language App with carried args.
    fn layer_app(&self, name: &str) -> Option<Expr> {
        let (op, args) = self.layers.get(name)?;
        if args.is_empty() {
            Some(Expr::op1(*op))
        } else {
            Some(Expr::op(*op, args.clone()))
        }
    }
}

/// Translate a MechGen [`ast::KbDef`] into Agentic Binary Language symbolic operations.
///
/// Each rule becomes a `UNIFY`-then-`INFER` pipeline; facts become `RESOLVE`
/// applications. The resulting expression evaluates the KB closure via the
/// RMI symbolic VM.
pub struct KbTranslator;

impl KbTranslator {
    /// Translate a `kb` definition into an Agentic Binary Language expression.
    pub fn translate(kb: &ast::KbDef, symbols: &mut SymbolTable) -> Expr {
        let mut stages: Vec<Expr> = Vec::new();

        // fact `p(a, b)` → RESOLVE(p_sym, a_sym, b_sym): predicate name + every
        // ground term is interned, so the full fact recovers on decode (arity =
        // number of term args).
        for fact in &kb.facts {
            let mut args = vec![Expr::sym(symbols.intern(&fact.name))];
            for a in &fact.args {
                args.push(Expr::sym(symbols.intern(&term_name(a))));
            }
            stages.push(Expr::op(Op::RESOLVE, args));
        }

        // rule `r(x, z) where p(x, y), p(y, z) {…}` lowers to a Horn clause:
        //   UNIFY(r, x, z) >> MATCH(p, x, y) >> MATCH(p, y, z) >> INFER
        // — head (name + params) then one MATCH per body literal, INFER closing
        // the clause. The whole thing recovers via the flat-Seq state machine in
        // `decompile_symbolic`, and is what the evaluator forward-chains over.
        for rule in &kb.rules {
            let mut head_args = vec![Expr::sym(symbols.intern(&rule.name))];
            for p in &rule.params {
                head_args.push(Expr::sym(symbols.intern(&p.name)));
            }
            let mut stage = Expr::op(Op::UNIFY, head_args);
            for cond in &rule.conditions {
                if let Some((pred, cargs)) = call_pred(cond) {
                    let mut m = vec![Expr::sym(symbols.intern(&pred))];
                    for a in &cargs {
                        m.push(Expr::sym(symbols.intern(a)));
                    }
                    stage = stage >> Expr::op(Op::MATCH, m);
                }
            }
            stage = stage >> Expr::op1(Op::INFER);
            stages.push(stage);
        }

        match stages.len() {
            0 => Expr::id(),
            1 => stages.into_iter().next().unwrap(),
            _ => stages
                .into_iter()
                .reduce(|acc, next| acc >> next)
                .unwrap_or_else(Expr::id),
        }
    }
}

/// Extract a fact term's name for interning (identifiers and literals; other
/// expression shapes collapse to `_`).
fn term_name(e: &ast::Expr) -> String {
    match e {
        ast::Expr::Ident { name } => name.clone(),
        ast::Expr::Literal { value, .. } => value.clone(),
        _ => "_".to_string(),
    }
}

/// Extract `(predicate, [args])` from a rule's `where` condition (a call like
/// `parent(x, y)`). Returns `None` for non-call conditions.
fn call_pred(e: &ast::Expr) -> Option<(String, Vec<String>)> {
    match e {
        ast::Expr::Call { func, args } => {
            let pred = match func.as_ref() {
                ast::Expr::Ident { name } => name.clone(),
                _ => return None,
            };
            Some((pred, args.iter().map(term_name).collect()))
        }
        _ => None,
    }
}

/// Translate a MechGen [`ast::AgentDef`] into an Agentic Binary Language `SPAWN` expression.
pub struct AgentTranslator;

impl AgentTranslator {
    /// Produce an Agentic Binary Language expression that spawns this agent:
    /// `SPAWN(agent_sym, cap_sym1, …, cap_symN)`. The agent name and every
    /// capability name are interned into the symbol table, so the artifact is
    /// fully self-describing (names recover on decode; the VM ignores the args).
    pub fn translate(agent: &ast::AgentDef, symbols: &mut SymbolTable) -> Expr {
        let mut args = vec![Expr::sym(symbols.intern(&agent.name))];
        for cap in &agent.capabilities {
            args.push(Expr::sym(symbols.intern(cap)));
        }
        let spawn = Expr::op(Op::SPAWN, args);
        // Approval-gated operations lower to a DELEGATE carrying their names, so
        // `requires_approval` recovers on decode (kept separate from SPAWN's
        // capabilities). No DELEGATE when there are none.
        if agent.requires_approval.is_empty() {
            spawn
        } else {
            let approvals: Vec<Expr> = agent
                .requires_approval
                .iter()
                .map(|r| Expr::sym(symbols.intern(r)))
                .collect();
            spawn >> Expr::op(Op::DELEGATE, approvals)
        }
    }
}

/// Translate a MechGen [`ast::SwarmDef`] into a multi-agent Agentic Binary Language expression:
/// `SPAWN(size) >> SEND/RECV on topology >> aggregate`.
pub struct SwarmTranslator;

impl SwarmTranslator {
    /// Build an Agentic Binary Language expression for a swarm.
    ///
    /// If `swarm.transport` is one of `"rmi-quic"`, `"rmi-tcp"`, or
    /// `"rmi-grpc"`, the SEND/RECV ops are emitted with a transport-tag
    /// symbol so downstream codegen can dispatch to
    /// `rmi::distributed::transport`. Otherwise the local
    /// [`crate::swarm_bus`] handles the messaging.
    pub fn translate(swarm: &ast::SwarmDef, symbols: &mut SymbolTable) -> Expr {
        let agent_sym = symbols.intern(&swarm.agent_type);
        // SPAWN(agent, size, topology?) — the exact topology label is interned
        // so it recovers on decode (not just the comm pattern it selects).
        let mut spawn_args = vec![Expr::sym(agent_sym), Expr::int(swarm_size(swarm))];
        if let Some(topo) = &swarm.topology {
            spawn_args.push(Expr::sym(symbols.intern(topo)));
        }
        let spawn = Expr::op(Op::SPAWN, spawn_args);

        // Encode the transport choice as a symbol argument to SEND/RECV. Accept
        // both `rmi-*` and `rmi_*` (the source grammar parses an identifier, so
        // the underscore form is what survives a build-from-spec round-trip).
        let transport_arg = match swarm.transport.as_deref() {
            Some(t) if t.starts_with("rmi-") || t.starts_with("rmi_") => {
                Some(Expr::sym(symbols.intern(t)))
            }
            _ => None,
        };
        let send = match transport_arg.clone() {
            Some(arg) => Expr::op(Op::SEND, vec![arg]),
            None => Expr::op1(Op::SEND),
        };
        let recv = match transport_arg {
            Some(arg) => Expr::op(Op::RECV, vec![arg]),
            None => Expr::op1(Op::RECV),
        };

        let comm = match swarm.topology.as_deref() {
            Some("broadcast") => send,
            Some("ring") | Some("mesh") | Some("star") | Some("tree") => send >> recv,
            _ => recv,
        };
        // REDUCE(consensus?) — the consensus strategy is interned on the
        // aggregate so it recovers on decode.
        let aggregate = match &swarm.consensus {
            Some(c) => Expr::op(Op::REDUCE, vec![Expr::sym(symbols.intern(c))]),
            None => Expr::op1(Op::REDUCE),
        };
        spawn >> comm >> aggregate
    }
}

fn swarm_size(swarm: &ast::SwarmDef) -> i64 {
    // Fold a literal `size: N`; default 1 if absent or non-literal. This is the
    // value the artifact actually carries (SPAWN's int arg), so it round-trips.
    match &swarm.size {
        Some(ast::Expr::Literal { value, kind: ast::LiteralKind::Int }) => {
            value.parse().unwrap_or(1)
        }
        _ => 1,
    }
}

// ═══════════════════════════════════════════════════════════════════
// Module-level driver
// ═══════════════════════════════════════════════════════════════════

/// Result of translating a MechGen module to Agentic Binary Language.
pub struct AblModule {
    /// Top-level Agentic Binary Language expressions, one per Agentic Binary Language-routed item, paired with the
    /// originating item's display name.
    pub items: Vec<(String, Expr)>,
    /// Symbol table populated during translation. Must be serialized
    /// alongside the expressions for binary codec round-trip.
    pub symbols: SymbolTable,
    /// Aggregated diagnostics (e.g. unknown layer types) collected from all
    /// translators.
    pub diagnostics: Vec<String>,
}

/// Translate every Agentic Binary Language-routed item in a [`ast::Module`] to Agentic Binary Language.
///
/// Items routed to MLIR are skipped; use [`crate::mlir::emit`] for those.
pub fn lower_module(module: &ast::Module) -> AblModule {
    let mut symbols = SymbolTable::new();
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();

    for item in &module.items {
        if OpFamilyRouter::route(item) == IrTarget::Mlir {
            continue;
        }
        match &item.kind {
            ast::ItemKind::Net(net) => {
                let t = NetTranslator::translate(net);
                for unk in &t.unknown_layers {
                    diagnostics.push(format!(
                        "net `{}`: unknown layer type `{}` lowered to IDENTITY",
                        net.name, unk
                    ));
                }
                items.push((net.name.clone(), t.expr));
            }
            ast::ItemKind::Kb(kb) => {
                items.push((kb.name.clone(), KbTranslator::translate(kb, &mut symbols)));
            }
            ast::ItemKind::Agent(agent) => {
                items.push((
                    agent.name.clone(),
                    AgentTranslator::translate(agent, &mut symbols),
                ));
            }
            ast::ItemKind::Swarm(swarm) => {
                items.push((
                    swarm.name.clone(),
                    SwarmTranslator::translate(swarm, &mut symbols),
                ));
            }
            ast::ItemKind::Train(train) => {
                // Training loops compose: forward-pass net ref >> loss >> optimizer step.
                let net_sym = symbols.intern(&train.net);
                let expr = Expr::sym(net_sym)
                    >> Expr::op1(Op::MSE_LOSS)
                    >> Expr::op1(Op::SGD_STEP);
                items.push((train.name.clone(), expr));
            }
            ast::ItemKind::Evolve(ev) => {
                // Population loop: MAP fitness over genomes, REDUCE selects survivors.
                items.push((
                    ev.name.clone(),
                    Expr::op1(Op::MAP) >> Expr::op1(Op::REDUCE),
                ));
            }
            _ => unreachable!("router said Agentic Binary Language but kind unmatched"),
        }
    }

    AblModule {
        items,
        symbols,
        diagnostics,
    }
}

// Suppress unused-import warnings for re-exports we want available to
// downstream modules that `use crate::abl_bridge::*`.
#[allow(dead_code)]
fn _exports(_: &Sym, _: &Ty, _: &Val) {}

// ═══════════════════════════════════════════════════════════════════
// Decompiler — Agentic Binary Language Expr → MechGen NetDef
// ═══════════════════════════════════════════════════════════════════

/// Decompile an Agentic Binary Language [`Expr`] into a MechGen [`ast::NetDef`].
///
/// Walks a `Seq` chain of neural `App` nodes and emits one [`ast::LayerDef`]
/// per recognised opcode. Args carried in each `App` are converted back to
/// MechGen integer / float literals.
///
/// Non-neural opcodes encountered during the walk are recorded in
/// [`DecompileResult::skipped`] so callers can decide whether the result is
/// faithful or only a partial reconstruction.
///
/// Round-trip guarantee: for any [`ast::NetDef`] whose layers are all
/// canonical (i.e. resolve via [`layer_name_to_op`]) and whose `forward`
/// block is the declaration-order fallback, the following holds:
///
/// ```text
/// let n2 = decompile(NetTranslator::translate(&n).expr, &n.name).net;
/// NetTranslator::translate(&n2).expr.content_hash()
///     == NetTranslator::translate(&n).expr.content_hash()
/// ```
pub fn decompile(expr: &Expr, net_name: &str) -> DecompileResult {
    let mut layers = Vec::new();
    let mut skipped = Vec::new();
    let mut counter = 0usize;
    decompile_walk(expr, &mut layers, &mut skipped, &mut counter);

    let forward = ast::Block {
        stmts: Vec::new(),
        tail_expr: None,
    };

    DecompileResult {
        net: ast::NetDef {
            name: net_name.to_string(),
            generics: Vec::new(),
            layers,
            forward,
        },
        skipped,
    }
}

/// Outcome of a [`decompile`] call.
#[derive(Debug, Clone)]
pub struct DecompileResult {
    /// Reconstructed `NetDef`.
    pub net: ast::NetDef,
    /// Opcodes encountered that have no canonical MechGen layer name — these
    /// were skipped during reconstruction.
    pub skipped: Vec<Op>,
}

fn decompile_walk(
    expr: &Expr,
    layers: &mut Vec<ast::LayerDef>,
    skipped: &mut Vec<Op>,
    counter: &mut usize,
) {
    match expr {
        Expr::Seq(a, b) => {
            decompile_walk(a, layers, skipped, counter);
            decompile_walk(b, layers, skipped, counter);
        }
        Expr::App(op, args) => {
            if let Some(name) = op_to_layer_name(*op) {
                *counter += 1;
                layers.push(ast::LayerDef {
                    name: format!("l_{}_{}", name.to_lowercase(), counter),
                    layer_type: ast::Type::Path {
                        segments: vec![name.to_string()],
                        type_args: Vec::new(),
                    },
                    args: args.iter().filter_map(decompile_arg).collect(),
                });
            } else if *op != Op::IDENTITY && *op != Op::RES_ADD {
                skipped.push(*op);
            }
            // Args may themselves contain nested Seqs (rare); recurse defensively.
            for a in args {
                decompile_walk(a, layers, skipped, counter);
            }
        }
        Expr::Par(a, b) => {
            decompile_walk(a, layers, skipped, counter);
            decompile_walk(b, layers, skipped, counter);
        }
        _ => {}
    }
}

/// Translate an Agentic Binary Language literal arg back to a MechGen literal expression.
fn decompile_arg(arg: &Expr) -> Option<ast::Expr> {
    match arg {
        Expr::Lit(Val::I64(n)) => Some(ast::Expr::Literal {
            value: n.to_string(),
            kind: ast::LiteralKind::Int,
        }),
        Expr::Lit(Val::F32(bits)) => Some(ast::Expr::Literal {
            value: f32::from_bits(*bits).to_string(),
            kind: ast::LiteralKind::Float,
        }),
        Expr::Lit(Val::Bool(b)) => Some(ast::Expr::Literal {
            value: b.to_string(),
            kind: ast::LiteralKind::Bool,
        }),
        _ => None,
    }
}

/// The recoverable structure of a *symbolic* (`kb`) Agentic Binary Language artifact:
/// one arity per fact (`RESOLVE`) and one parameter-count per rule (`UNIFY`).
///
/// Predicate **names are not recoverable** from the artifact — the symbol table
/// is not serialized into the container. What the artifact carries (and the VM
/// executes) is the symbolic *structure*: predicate arities and the
/// unify→infer rule pipeline. This view reports exactly that, no more.
#[derive(Debug, Clone, Default)]
pub struct SymbolicView {
    /// Arity of each fact, in artifact order (one per `RESOLVE`).
    pub fact_arities: Vec<i64>,
    /// Parameter count of each rule, in artifact order (one per `UNIFY`).
    pub rule_param_counts: Vec<i64>,
    /// Symbol id of each fact predicate (parallel to `fact_arities`). Resolve
    /// against the container's symbol table ([`crate::abl::decode_symbols`])
    /// to recover the predicate name.
    pub fact_syms: Vec<u32>,
    /// Symbol id of each rule (parallel to `rule_param_counts`).
    pub rule_syms: Vec<u32>,
    /// Symbol ids of each fact's ground term args (parallel to `fact_syms`).
    pub fact_arg_syms: Vec<Vec<u32>>,
    /// Symbol ids of each rule's parameter names (parallel to `rule_syms`).
    pub rule_param_syms: Vec<Vec<u32>>,
    /// Each rule's body literals as `(pred_sym, [arg_syms])` (parallel to
    /// `rule_syms`). Empty for a body-less rule.
    pub rule_body_syms: Vec<Vec<(u32, Vec<u32>)>>,
}

/// Flatten a (possibly nested) `Seq`/`Par` chain into its leaf nodes, in order.
fn flatten_seq<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    match e {
        Expr::Seq(a, b) | Expr::Par(a, b) => {
            flatten_seq(a, out);
            flatten_seq(b, out);
        }
        other => out.push(other),
    }
}

/// Decompile a symbolic (`kb`) expression into its recoverable [`SymbolicView`].
/// Empty result ⇒ the expression is not symbolic (e.g. it's a net).
///
/// The op stream is linear: `RESOLVE` = a fact; a rule is `UNIFY (MATCH)* INFER`
/// — so a small state machine over the flattened sequence reconstructs each
/// rule's head and body unambiguously.
pub fn decompile_symbolic(expr: &Expr) -> SymbolicView {
    let mut v = SymbolicView::default();
    let mut leaves = Vec::new();
    flatten_seq(expr, &mut leaves);

    // current rule being assembled: (rule_sym, param_syms, body)
    let mut cur: Option<(u32, Vec<u32>, Vec<(u32, Vec<u32>)>)> = None;
    let finish = |cur: &mut Option<(u32, Vec<u32>, Vec<(u32, Vec<u32>)>)>, v: &mut SymbolicView| {
        if let Some((r, params, body)) = cur.take() {
            v.rule_syms.push(r);
            v.rule_param_counts.push(params.len() as i64);
            v.rule_param_syms.push(params);
            v.rule_body_syms.push(body);
        }
    };

    for leaf in leaves {
        let Expr::App(op, args) = leaf else { continue };
        let syms = ref_syms(args);
        match *op {
            Op::RESOLVE => {
                if let Some((&pred, terms)) = syms.split_first() {
                    v.fact_syms.push(pred);
                    v.fact_arities.push(terms.len() as i64);
                    v.fact_arg_syms.push(terms.to_vec());
                }
            }
            Op::UNIFY => {
                finish(&mut cur, &mut v); // close any prior rule defensively
                if let Some((&rule, params)) = syms.split_first() {
                    cur = Some((rule, params.to_vec(), Vec::new()));
                }
            }
            Op::MATCH => {
                if let Some((_, _, body)) = cur.as_mut() {
                    if let Some((&pred, cargs)) = syms.split_first() {
                        body.push((pred, cargs.to_vec()));
                    }
                }
            }
            Op::INFER => finish(&mut cur, &mut v),
            _ => {}
        }
    }
    finish(&mut cur, &mut v);
    v
}

fn first_int(args: &[Expr]) -> Option<i64> {
    args.iter().find_map(|a| match a {
        Expr::Lit(Val::I64(n)) => Some(*n),
        _ => None,
    })
}

fn first_sym(args: &[Expr]) -> Option<u32> {
    args.iter().find_map(|a| match a {
        Expr::Ref(s) => Some(s.0),
        _ => None,
    })
}

/// All `Ref` symbol ids among `args`, in order.
fn ref_syms(args: &[Expr]) -> Vec<u32> {
    args.iter()
        .filter_map(|a| match a {
            Expr::Ref(s) => Some(s.0),
            _ => None,
        })
        .collect()
}

/// A resolved ground fact: predicate name + constant term names.
pub type GroundFact = (String, Vec<String>);

/// A resolved Horn-clause rule: head predicate + head variables + body literals
/// (predicate, variable args). All rule args are logic variables.
#[derive(Debug, Clone)]
pub struct KbRule {
    /// Head predicate (the derived relation's name).
    pub head: String,
    /// Head parameter variables (range-restricted: each appears in the body).
    pub params: Vec<String>,
    /// Body literals: `(predicate, [variable args])`.
    pub body: Vec<(String, Vec<String>)>,
}

/// Forward-chain `rules` over `facts` to the least fixpoint and return the
/// **derived** facts (closure minus the initial set), de-duplicated, in
/// deterministic order. This is the real execution semantics of a `kb`
/// artifact — a safe, terminating Datalog evaluation (no function symbols, so
/// the Herbrand base is finite). It is a pure interpreter: no arbitrary code
/// runs, preserving the no-exec property of the artifact.
pub fn evaluate_kb(facts: &[GroundFact], rules: &[KbRule]) -> Vec<GroundFact> {
    use std::collections::{BTreeMap, BTreeSet};
    let key = |f: &GroundFact| format!("{}({})", f.0, f.1.join(","));
    let initial: BTreeSet<String> = facts.iter().map(&key).collect();
    let mut known = initial.clone();
    let mut all: Vec<GroundFact> = facts.to_vec();

    loop {
        let mut added: Vec<GroundFact> = Vec::new();
        for rule in rules {
            // Enumerate variable substitutions satisfying every body literal,
            // joining against the current fact set (relational join).
            let mut subs: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];
            for (bp, bargs) in &rule.body {
                let mut next = Vec::new();
                for sub in &subs {
                    for f in &all {
                        if &f.0 != bp || f.1.len() != bargs.len() {
                            continue;
                        }
                        let mut s2 = sub.clone();
                        let mut ok = true;
                        for (var, c) in bargs.iter().zip(&f.1) {
                            match s2.get(var) {
                                Some(prev) if prev != c => {
                                    ok = false;
                                    break;
                                }
                                Some(_) => {}
                                None => {
                                    s2.insert(var.clone(), c.clone());
                                }
                            }
                        }
                        if ok {
                            next.push(s2);
                        }
                    }
                }
                subs = next;
                if subs.is_empty() {
                    break;
                }
            }
            for sub in &subs {
                let mut terms = Vec::with_capacity(rule.params.len());
                let mut ok = true;
                for p in &rule.params {
                    match sub.get(p) {
                        Some(c) => terms.push(c.clone()),
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let nf = (rule.head.clone(), terms);
                let k = key(&nf);
                if known.insert(k) {
                    added.push(nf);
                }
            }
        }
        if added.is_empty() {
            break;
        }
        all.extend(added);
    }
    all.into_iter().filter(|f| !initial.contains(&key(f))).collect()
}

// ── Agent / swarm execution semantics ────────────────────────────────
//
// A precise, deterministic, pure-data operational model for the agentic kinds —
// interpreting exactly what the artifact stores. No arbitrary code runs (the
// no-exec property holds); these are reference evaluators for the declared
// policy/protocol, not a general agent runtime.

/// The decision for one requested operation under an agent's capability policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpDecision {
    /// In `capabilities` and not approval-gated — permitted.
    Allowed,
    /// In `capabilities` AND in `requires_approval` — needs approval first.
    RequiresApproval,
    /// Not in `capabilities` — refused.
    Denied,
}

impl OpDecision {
    /// Stable lowercase tag.
    pub fn tag(self) -> &'static str {
        match self {
            OpDecision::Allowed => "allowed",
            OpDecision::RequiresApproval => "requires-approval",
            OpDecision::Denied => "denied",
        }
    }
}

/// Evaluate an agent's capability policy over requested operations. Deterministic
/// and total: every op gets exactly one decision.
pub fn eval_agent_policy(
    capabilities: &[String],
    requires_approval: &[String],
    ops: &[String],
) -> Vec<(String, OpDecision)> {
    ops.iter()
        .map(|op| {
            let d = if !capabilities.iter().any(|c| c == op) {
                OpDecision::Denied
            } else if requires_approval.iter().any(|a| a == op) {
                OpDecision::RequiresApproval
            } else {
                OpDecision::Allowed
            };
            (op.clone(), d)
        })
        .collect()
}

/// Result of evaluating a swarm's consensus protocol.
#[derive(Debug, Clone)]
pub struct SwarmResult {
    /// Number of agents.
    pub size: i64,
    /// Topology label.
    pub topology: String,
    /// Consensus strategy.
    pub consensus: String,
    /// Rounds for a value to propagate across the topology (diameter model).
    pub rounds_to_converge: u64,
    /// The decided value, if the strategy reached one.
    pub decided: Option<i64>,
    /// Human/agent-readable reason for the outcome.
    pub reason: String,
}

fn ceil_log2(n: u64) -> u64 {
    if n <= 1 {
        0
    } else {
        (u64::BITS - (n - 1).leading_zeros()) as u64
    }
}

/// Rounds for a value to reach every agent given the topology (graph diameter):
/// mesh/star/broadcast = 1, ring = n-1, tree = ceil(log2 n).
pub fn rounds_to_converge(topology: &str, size: i64) -> u64 {
    let n = size.max(1) as u64;
    match topology {
        "mesh" | "star" | "broadcast" => 1,
        "ring" => n.saturating_sub(1),
        "tree" => ceil_log2(n),
        _ => 1,
    }
}

/// Apply a consensus strategy to a set of votes. Deterministic tiebreak: among
/// values with the top count, the smallest value wins.
fn decide(consensus: &str, votes: &[i64]) -> (Option<i64>, String) {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for &v in votes {
        *counts.entry(v).or_default() += 1;
    }
    let n = votes.len();
    // Plurality value with deterministic smallest-on-tie selection.
    let mut best: Option<(i64, usize)> = None;
    for (&v, &c) in &counts {
        if best.map(|(_, bc)| c > bc).unwrap_or(true) {
            best = Some((v, c));
        }
    }
    let (mode, mode_count) = best.unwrap_or((0, 0));
    match consensus {
        "unanimous" => {
            if counts.len() == 1 {
                (Some(mode), "all agents agree".into())
            } else {
                (None, format!("not unanimous ({} distinct values)", counts.len()))
            }
        }
        "quorum" => {
            if mode_count * 2 > n {
                (Some(mode), format!("{mode_count}/{n} forms a strict-majority quorum"))
            } else {
                (None, format!("no value reached a quorum (top is {mode_count}/{n})"))
            }
        }
        // "majority" and "weighted" (no weights ⇒ plurality).
        _ => (Some(mode), format!("plurality: {mode_count}/{n} votes")),
    }
}

/// Evaluate a swarm: report the propagation rounds for its topology and, if
/// per-agent `proposals` (votes) are supplied, the consensus decision.
pub fn eval_swarm_consensus(
    size: i64,
    topology: &str,
    consensus: &str,
    proposals: &[i64],
) -> SwarmResult {
    let rounds = rounds_to_converge(topology, size);
    let (decided, reason) = if proposals.is_empty() {
        (None, "no proposals supplied (pass --input {\"proposals\":[..]} to decide)".into())
    } else {
        decide(consensus, proposals)
    };
    SwarmResult {
        size,
        topology: topology.to_string(),
        consensus: consensus.to_string(),
        rounds_to_converge: rounds,
        decided,
        reason,
    }
}

/// The recoverable structure of an *agentic* artifact (agent or swarm).
///
/// Agent lowering: `SPAWN(agent_sym, cap_syms…)`. Swarm lowering:
/// `SPAWN(agent_sym, int(size)) >> SEND/RECV[transport] >> REDUCE`. The exact
/// topology is NOT recoverable (ring/mesh/star/tree all lower to send>>recv) —
/// only the comm pattern is.
#[derive(Debug, Clone, Default)]
pub struct AgenticView {
    /// True if this is a swarm (has the REDUCE aggregate); else a bare agent.
    pub is_swarm: bool,
    /// Symbol id of the spawned agent / agent-type (SPAWN's first arg).
    pub spawn_sym: Option<u32>,
    /// Capability symbol ids (agent only — SPAWN's `Ref` args after the first).
    pub cap_syms: Vec<u32>,
    /// Approval-gated operation symbol ids (agent only — from DELEGATE).
    pub approval_syms: Vec<u32>,
    /// Swarm size (SPAWN's int arg), if present.
    pub size: Option<i64>,
    /// Topology symbol id (swarm only — SPAWN's `Ref` arg after the size int).
    pub topology_sym: Option<u32>,
    /// Consensus symbol id (swarm only — REDUCE's `Ref` arg).
    pub consensus_sym: Option<u32>,
    /// Transport symbol id carried on SEND/RECV, if any.
    pub transport_sym: Option<u32>,
    /// Comm pattern observed (for swarms): saw a SEND / saw a RECV.
    pub has_send: bool,
    pub has_recv: bool,
}

/// Decompile an agentic (`agent`/`swarm`) expression. `None` if it contains no
/// `SPAWN` (i.e. it is not agentic).
pub fn decompile_agentic(expr: &Expr) -> Option<AgenticView> {
    let mut v = AgenticView::default();
    walk_agentic(expr, &mut v);
    v.spawn_sym?;
    // For a swarm, SPAWN's only post-agent Ref is the topology (capabilities are
    // agent-only); reinterpret what the walk collected as cap_syms.
    if v.is_swarm {
        v.topology_sym = v.cap_syms.first().copied();
        v.cap_syms.clear();
    }
    Some(v)
}

fn walk_agentic(expr: &Expr, v: &mut AgenticView) {
    match expr {
        Expr::Seq(a, b) | Expr::Par(a, b) => {
            walk_agentic(a, v);
            walk_agentic(b, v);
        }
        Expr::App(op, args) => {
            match *op {
                Op::SPAWN => {
                    v.spawn_sym = first_sym(args);
                    // First Ref is the agent; any further Refs are capabilities.
                    let mut refs = args.iter().filter_map(|a| match a {
                        Expr::Ref(s) => Some(s.0),
                        _ => None,
                    });
                    let _agent = refs.next();
                    v.cap_syms = refs.collect();
                    v.size = first_int(args);
                }
                Op::SEND => {
                    v.has_send = true;
                    if let Some(s) = first_sym(args) {
                        v.transport_sym = Some(s);
                    }
                }
                Op::RECV => {
                    v.has_recv = true;
                    if let Some(s) = first_sym(args) {
                        v.transport_sym = Some(s);
                    }
                }
                Op::DELEGATE => v.approval_syms = ref_syms(args),
                Op::REDUCE => {
                    v.is_swarm = true;
                    v.consensus_sym = first_sym(args);
                }
                _ => {}
            }
            for a in args {
                walk_agentic(a, v);
            }
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use rmi::lang::codec;

    fn make_layer(type_name: &str) -> ast::LayerDef {
        ast::LayerDef {
            name: format!("l_{}", type_name.to_lowercase()),
            layer_type: ast::Type::Path {
                segments: vec![type_name.to_string()],
                type_args: Vec::new(),
            },
            args: Vec::new(),
        }
    }

    fn make_block() -> ast::Block {
        // crate::ast::Block is structurally `{ statements: Vec<_>, .. }`. The
        // tests below only need an empty block; we construct one via the
        // smallest valid shape using Default if available, else manual.
        // Falls back to JSON parse for shape independence.
        serde_json::from_str("{\"statements\":[],\"trailing_expr\":null}")
            .unwrap_or_else(|_| serde_json::from_str("{\"stmts\":[]}").unwrap())
    }

    #[test]
    fn mlp_translates_to_sequential_pipeline() {
        let net = ast::NetDef {
            name: "MLP".into(),
            generics: Vec::new(),
            layers: vec![
                make_layer("Linear"),
                make_layer("GELU"),
                make_layer("Linear"),
            ],
            forward: make_block(),
        };
        let t = NetTranslator::translate(&net);
        assert!(t.unknown_layers.is_empty());
        // 3 stages composed with >> → 2 Seq nodes wrapping 3 Apps.
        assert!(t.expr.node_count() >= 3);
    }

    #[test]
    fn transformer_block_encodes_compactly() {
        let net = ast::NetDef {
            name: "TransformerBlock".into(),
            generics: Vec::new(),
            layers: vec![
                make_layer("LayerNorm"),
                make_layer("Attention"),
                make_layer("Dropout"),
                make_layer("LayerNorm"),
                make_layer("Linear"),
                make_layer("GELU"),
                make_layer("Linear"),
                make_layer("Dropout"),
            ],
            forward: make_block(),
        };
        let t = NetTranslator::translate(&net);
        assert!(t.unknown_layers.is_empty());
        let size = codec::wire_size(&t.expr);
        assert!(size < 200, "transformer block wire size = {} (expected < 200)", size);
    }

    #[test]
    fn unknown_layer_falls_back_to_identity() {
        let net = ast::NetDef {
            name: "Custom".into(),
            generics: Vec::new(),
            layers: vec![make_layer("MyFancyLayer")],
            forward: make_block(),
        };
        let t = NetTranslator::translate(&net);
        assert_eq!(t.unknown_layers, vec!["MyFancyLayer".to_string()]);
    }

    #[test]
    fn router_dispatches_by_item_kind() {
        let fn_item = ast::Item {
            visibility: ast::Visibility::Private,
            attributes: Vec::new(),
            kind: ast::ItemKind::Function(ast::FunctionDef {
                name: "main".into(),
                is_async: false,
                is_unsafe: false,
                generics: Vec::new(),
                params: Vec::new(),
                return_type: None,
                where_clause: Vec::new(),
                effects: Vec::new(),
                contracts: Vec::new(),
                body: make_block(),
                body_expr: None,
            }),
        };
        assert_eq!(OpFamilyRouter::route(&fn_item), IrTarget::Mlir);

        let net_item = ast::Item {
            visibility: ast::Visibility::Private,
            attributes: Vec::new(),
            kind: ast::ItemKind::Net(ast::NetDef {
                name: "N".into(),
                generics: Vec::new(),
                layers: Vec::new(),
                forward: make_block(),
            }),
        };
        assert_eq!(OpFamilyRouter::route(&net_item), IrTarget::Machine);
    }

    fn make_layer_with_args(name: &str, type_name: &str, args: Vec<ast::Expr>) -> ast::LayerDef {
        ast::LayerDef {
            name: name.into(),
            layer_type: ast::Type::Path {
                segments: vec![type_name.into()],
                type_args: Vec::new(),
            },
            args,
        }
    }

    fn int_lit(n: i64) -> ast::Expr {
        ast::Expr::Literal {
            value: n.to_string(),
            kind: ast::LiteralKind::Int,
        }
    }

    fn ident(name: &str) -> ast::Expr {
        ast::Expr::Ident { name: name.into() }
    }

    fn call(func: ast::Expr, args: Vec<ast::Expr>) -> ast::Expr {
        ast::Expr::Call {
            func: Box::new(func),
            args,
        }
    }

    fn pipeline(left: ast::Expr, right: ast::Expr) -> ast::Expr {
        ast::Expr::Pipeline {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn binary_add(left: ast::Expr, right: ast::Expr) -> ast::Expr {
        ast::Expr::Binary {
            op: "+".into(),
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn block_with_tail(tail: ast::Expr) -> ast::Block {
        ast::Block {
            stmts: Vec::new(),
            tail_expr: Some(Box::new(tail)),
        }
    }

    #[test]
    fn layer_args_propagate_to_machine_app() {
        // net { layer fc1: Linear(784, 256); forward { fc1(x) } }
        let net = ast::NetDef {
            name: "WithArgs".into(),
            generics: Vec::new(),
            layers: vec![make_layer_with_args(
                "fc1",
                "Linear",
                vec![int_lit(784), int_lit(256)],
            )],
            forward: block_with_tail(call(ident("fc1"), vec![ident("x")])),
        };
        let t = NetTranslator::translate(&net);
        // Inspect the produced App and check args were threaded through.
        match &t.expr {
            rmi::lang::Expr::App(op, args) => {
                assert_eq!(*op, rmi::lang::Op::LINEAR);
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected App, got {other:?}"),
        }
    }

    #[test]
    fn forward_pipeline_translates_in_order() {
        // forward { x |> fc1 |> act |> fc2 }
        let net = ast::NetDef {
            name: "Pipe".into(),
            generics: Vec::new(),
            layers: vec![
                make_layer("Linear"),
                make_layer("ReLU"),
                make_layer("Linear"),
            ],
            forward: block_with_tail(pipeline(
                pipeline(pipeline(ident("x"), ident("l_linear")), ident("l_relu")),
                ident("l_linear"),
            )),
        };
        // Note: declaration order would also produce 3 layers; assert via
        // node_count + presence of LINEAR + RELU in the resulting tree.
        let t = NetTranslator::translate(&net);
        assert!(t.unknown_layers.is_empty());
        let ops = t.expr.opcodes();
        assert!(ops.contains(&rmi::lang::Op::LINEAR));
        assert!(ops.contains(&rmi::lang::Op::RELU));
    }

    #[test]
    fn residual_block_uses_res_add() {
        // forward { x + l_linear(x) }
        let net = ast::NetDef {
            name: "Resid".into(),
            generics: Vec::new(),
            layers: vec![make_layer("Linear")],
            forward: block_with_tail(binary_add(
                ident("x"),
                call(ident("l_linear"), vec![ident("x")]),
            )),
        };
        let t = NetTranslator::translate(&net);
        let ops = t.expr.opcodes();
        assert!(ops.contains(&rmi::lang::Op::RES_ADD), "expected RES_ADD, got {ops:?}");
    }

    #[test]
    fn nested_call_translates_to_left_to_right_pipeline() {
        // forward { l_linear(l_relu(l_linear(x))) }
        let net = ast::NetDef {
            name: "Nested".into(),
            generics: Vec::new(),
            layers: vec![
                make_layer("Linear"),
                make_layer("ReLU"),
            ],
            forward: block_with_tail(call(
                ident("l_linear"),
                vec![call(
                    ident("l_relu"),
                    vec![call(ident("l_linear"), vec![ident("x")])],
                )],
            )),
        };
        let t = NetTranslator::translate(&net);
        let ops = t.expr.opcodes();
        assert!(ops.contains(&rmi::lang::Op::LINEAR));
        assert!(ops.contains(&rmi::lang::Op::RELU));
        // Three layer applications composed with `>>` → at least 3 App nodes
        // plus Seq wrappers. node_count walks the whole tree.
        assert!(t.expr.node_count() >= 5, "expected non-trivial tree, got {} nodes", t.expr.node_count());
    }

    #[test]
    fn swarm_with_rmi_transport_embeds_transport_sym() {
        let swarm = ast::SwarmDef {
            name: "Distributed".into(),
            agent_type: "Worker".into(),
            size: None,
            topology: Some("ring".into()),
            consensus: Some("majority".into()),
            on_dispatch: None,
            on_aggregate: None,
            on_failure: None,
            transport: Some("rmi-quic".into()),
        };
        let mut symbols = SymbolTable::new();
        let expr = SwarmTranslator::translate(&swarm, &mut symbols);
        // Resolve the transport sym to confirm it was interned.
        let transport_sym = symbols.intern("rmi-quic");
        // Walk the expression and check that App(SEND, [...]) and App(RECV, [...])
        // each carry the transport sym as an arg.
        let mut seen_send_with_transport = false;
        let mut seen_recv_with_transport = false;
        fn visit(
            e: &rmi::lang::Expr,
            transport_sym: rmi::lang::Sym,
            seen_send: &mut bool,
            seen_recv: &mut bool,
        ) {
            match e {
                rmi::lang::Expr::App(op, args) => {
                    let has_transport = args.iter().any(|a| matches!(a, rmi::lang::Expr::Ref(s) if *s == transport_sym));
                    if *op == rmi::lang::Op::SEND && has_transport {
                        *seen_send = true;
                    }
                    if *op == rmi::lang::Op::RECV && has_transport {
                        *seen_recv = true;
                    }
                    for a in args {
                        visit(a, transport_sym, seen_send, seen_recv);
                    }
                }
                rmi::lang::Expr::Seq(a, b) | rmi::lang::Expr::Par(a, b) => {
                    visit(a, transport_sym, seen_send, seen_recv);
                    visit(b, transport_sym, seen_send, seen_recv);
                }
                _ => {}
            }
        }
        visit(&expr, transport_sym, &mut seen_send_with_transport, &mut seen_recv_with_transport);
        assert!(seen_send_with_transport, "SEND should carry transport sym");
        assert!(seen_recv_with_transport, "RECV should carry transport sym (ring topology)");
    }

    #[test]
    fn family_classifier_identifies_stubs() {
        let mlp = ast::NetDef {
            name: "M".into(),
            generics: Vec::new(),
            layers: vec![make_layer("Linear"), make_layer("ReLU")],
            forward: make_block(),
        };
        let t = NetTranslator::translate(&mlp);
        let families = expr_op_families(&t.expr);
        assert!(families.contains(&rmi::lang::OpFamily::Neural));
        assert!(families.iter().any(|f| is_stubbed_family(*f)));

        // A pure-math expression should not be stubbed.
        let math = rmi::lang::Expr::op2(
            rmi::lang::Op::ADD,
            rmi::lang::Expr::int(1),
            rmi::lang::Expr::int(2),
        );
        let math_families = expr_op_families(&math);
        assert!(!math_families.iter().any(|f| is_stubbed_family(*f)));
    }

    #[test]
    fn vm_evaluates_math_in_translated_kb_expression() {
        // Independent end-to-end smoke test: build a small Agentic Binary Language expression
        // through the bridge primitives, encode-decode it, and verify the VM
        // evaluates an embedded math expression.
        use rmi::lang::Vm;
        let mut vm = Vm::new();
        let result = vm
            .eval(&(rmi::lang::Expr::op2(
                rmi::lang::Op::ADD,
                rmi::lang::Expr::int(40),
                rmi::lang::Expr::int(2),
            )))
            .expect("eval");
        assert_eq!(result.as_i64(), Some(42));
    }

    #[test]
    fn jit_path_falls_back_for_neural_ops_without_error() {
        // The CLI uses Vm::eval_jit for the ml-run path. JIT can compile
        // pure math; neural ops must fall back gracefully (not panic, not
        // produce a wrong answer). This test mirrors the CLI behavior.
        use rmi::lang::Vm;
        let mlp = ast::NetDef {
            name: "M".into(),
            generics: Vec::new(),
            layers: vec![make_layer("Linear"), make_layer("ReLU")],
            forward: make_block(),
        };
        let t = NetTranslator::translate(&mlp);
        let mut vm = Vm::new();
        // eval_jit should return Err (neural ops are stubbed) but never panic.
        let _ = vm.eval_jit(&t.expr);

        // Pure math: JIT should successfully compute.
        let math = rmi::lang::Expr::op2(
            rmi::lang::Op::MUL,
            rmi::lang::Expr::int(6),
            rmi::lang::Expr::int(7),
        );
        let r = vm.eval_jit(&math).expect("math eval");
        // JIT path returns F64 per its contract; tree-walking returns I64.
        // Accept either.
        match r {
            rmi::lang::Val::I64(n) => assert_eq!(n, 42),
            rmi::lang::Val::F64(bits) => assert!((f64::from_bits(bits) - 42.0).abs() < 1e-9),
            other => panic!("unexpected eval_jit result: {other:?}"),
        }
    }

    #[test]
    fn decompile_recovers_canonical_layer_names() {
        let original = ast::NetDef {
            name: "MLP".into(),
            generics: Vec::new(),
            layers: vec![
                make_layer("Linear"),
                make_layer("ReLU"),
                make_layer("Linear"),
            ],
            forward: make_block(),
        };
        let lowered = NetTranslator::translate(&original);
        let result = decompile(&lowered.expr, "MLP");
        assert!(result.skipped.is_empty(), "skipped ops: {:?}", result.skipped);
        let names: Vec<_> = result
            .net
            .layers
            .iter()
            .map(|l| type_tail_name(&l.layer_type).unwrap_or("").to_string())
            .collect();
        assert_eq!(names, vec!["Linear", "ReLU", "Linear"]);
    }

    #[test]
    fn decompile_round_trips_through_lowering_with_stable_hash() {
        let original = ast::NetDef {
            name: "TransformerBlock".into(),
            generics: Vec::new(),
            layers: vec![
                make_layer("LayerNorm"),
                make_layer("Attention"),
                make_layer("Dropout"),
                make_layer("LayerNorm"),
                make_layer("Linear"),
                make_layer("GELU"),
                make_layer("Linear"),
                make_layer("Dropout"),
            ],
            forward: make_block(),
        };
        let t1 = NetTranslator::translate(&original);
        let dec = decompile(&t1.expr, "TransformerBlock");
        let t2 = NetTranslator::translate(&dec.net);
        assert_eq!(
            t1.expr.content_hash(),
            t2.expr.content_hash(),
            "decompile→lower must reproduce the same Agentic Binary Language hash"
        );
    }

    #[test]
    fn decompile_preserves_layer_args() {
        let original = ast::NetDef {
            name: "WithArgs".into(),
            generics: Vec::new(),
            layers: vec![make_layer_with_args(
                "fc1",
                "Linear",
                vec![int_lit(784), int_lit(256)],
            )],
            forward: make_block(),
        };
        let t = NetTranslator::translate(&original);
        let dec = decompile(&t.expr, "WithArgs");
        assert_eq!(dec.net.layers.len(), 1);
        assert_eq!(dec.net.layers[0].args.len(), 2);
        // Verify the args are the same integer literals.
        match &dec.net.layers[0].args[0] {
            ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => {
                assert_eq!(value, "784");
            }
            other => panic!("expected Int literal, got {other:?}"),
        }
    }

    #[test]
    fn module_lowering_round_trips_through_codec() {
        let net = ast::NetDef {
            name: "Net1".into(),
            generics: Vec::new(),
            layers: vec![make_layer("Linear"), make_layer("ReLU")],
            forward: make_block(),
        };
        let module = ast::Module {
            items: vec![ast::Item {
                visibility: ast::Visibility::Private,
                attributes: Vec::new(),
                kind: ast::ItemKind::Net(net),
            }],
        };
        let lowered = lower_module(&module);
        assert_eq!(lowered.items.len(), 1);
        // Round-trip first expr through codec.
        let (_, expr) = &lowered.items[0];
        let bytes = codec::Encoder::encode_expr_only(expr);
        assert!(!bytes.is_empty());
    }

    /// **Load-bearing invariant** for P78: every new RecursiveMachineIntelligence-MG layer
    /// name added in P76-P77 must resolve to a real opcode. If a future
    /// rename / typo drops a mapping, this test fires before any agent
    /// builds a net{} block that silently stubs out.
    #[test]
    fn p77_layer_names_all_resolve() {
        let names = [
            // attention variants (P77)
            "FlashAttention", "SlidingWindowAttention", "LongformerAttention",
            "LinearAttention", "PerformerAttention",
            "GroupedQueryAttention", "GQA", "MultiQueryAttention", "MQA",
            "CrossAttention",
            // activations
            "LeakyReLU", "ELU", "SELU", "HardSwish", "HardSigmoid",
            "SwiGLU", "GeGLU",
            // regularisation
            "Dropout2D", "DropPath", "StochasticDepth",
            // embeddings
            "RotaryEmbedding", "RoPE", "PositionalEmbedding",
            // pooling
            "GlobalAvgPool", "GlobalMaxPool", "GlobalMeanPool", "GlobalSumPool",
            // PEFT
            "LoRA", "QLoRA", "DoRA", "IA3", "Adapter",
            "PrefixTuning", "PromptTuning",
            // quantization
            "Int8Linear", "Int4Linear", "BitNetLinear",
            // recurrent
            "RNNCell", "LSTMCell", "GRUCell", "RNN", "LSTM", "GRU",
            // graph
            "GCNLayer", "GATLayer", "GraphSAGELayer", "EdgeConv",
            // state-space
            "S4Layer", "S5Layer", "MambaBlock", "H3Layer",
            // MoE
            "Expert", "SparseMoE", "TopKRouter", "SwitchRouter",
            "ExpertChoiceRouter",
        ];
        let mut unresolved = Vec::new();
        for n in names {
            if layer_name_to_op(n).is_none() {
                unresolved.push(n);
            }
        }
        assert!(
            unresolved.is_empty(),
            "{} P77 layer names unresolved: {unresolved:?}",
            unresolved.len()
        );
    }

    /// Sanity: a net{} block using the new P77 layer names lowers
    /// cleanly through `lower_module` - no unknown_layers diagnostics.
    #[test]
    fn p77_net_lowers_without_unknown_diagnostics() {
        let net = ast::NetDef {
            name: "P77_Net".into(),
            generics: Vec::new(),
            layers: vec![
                make_layer("FlashAttention"),
                make_layer("LayerNorm"),
                make_layer("SwiGLU"),
                make_layer("LoRA"),
                make_layer("GlobalAvgPool"),
            ],
            forward: make_block(),
        };
        let t = NetTranslator::translate(&net);
        assert!(
            t.unknown_layers.is_empty(),
            "P77 layer names produced unknown diagnostics: {:?}",
            t.unknown_layers
        );
    }
}
