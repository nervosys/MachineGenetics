//! RMIL opcodes — the instruction set.
//!
//! Every operation in RMIL is a [`Op`] — a `u16` organized by family:
//!
//! | Range     | Family   | Purpose                          |
//! |-----------|----------|----------------------------------|
//! | `0x00xx`  | Neural   | Differentiable layer operations  |
//! | `0x01xx`  | Symbolic | Logic, unification, planning     |
//! | `0x02xx`  | Control  | Composition, flow, aggregation   |
//! | `0x03xx`  | Memory   | Allocation, reshape, copy        |
//! | `0x04xx`  | Agent    | Send, recv, spawn, delegate      |
//! | `0x05xx`  | Meta     | Introspection, hashing, mutation |
//! | `0x06xx`  | Math     | Elementwise arithmetic           |
//!
//! An AI agent discovers the full instruction set by iterating [`Op::ALL`].
//! Each op carries queryable metadata: arity, differentiability, statefulness.

/// An RMIL opcode — a u16 instruction identifier.
///
/// Two bytes on the wire. The high byte selects the family,
/// the low byte selects the specific operation within that family.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Op(pub u16);

/// Opcode family (high byte of [`Op`]).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OpFamily {
    /// Differentiable neural-network operations.
    Neural = 0x00,
    /// Symbolic reasoning: unification, inference, planning.
    Symbolic = 0x01,
    /// Control flow and composition: seq, par, cond, loop.
    Control = 0x02,
    /// Memory management: alloc, reshape, transpose.
    Memory = 0x03,
    /// Agent communication: send, recv, spawn, delegate.
    Agent = 0x04,
    /// Meta / introspection: hash, typeof, mutate, decompose.
    Meta = 0x05,
    /// Elementwise math: add, mul, exp, log, sin, …
    Math = 0x06,
}

impl Op {
    // ── Neural 0x00xx ────────────────────────────────────────────────────

    /// Matrix multiply: `[m,k] × [k,n] → [m,n]`.
    pub const MATMUL: Op = Op(0x0001);
    /// Affine transform: `x·Wᵀ + b`.
    pub const LINEAR: Op = Op(0x0002);
    /// 2-D convolution.
    pub const CONV2D: Op = Op(0x0003);
    /// Multi-head attention.
    pub const ATTN: Op = Op(0x0004);
    /// Token embedding lookup.
    pub const EMBED: Op = Op(0x0005);
    /// Stochastic dropout.
    pub const DROP: Op = Op(0x0006);
    /// Softmax normalisation.
    pub const SOFTMAX: Op = Op(0x0007);

    // Activations 0x0010–
    /// ReLU: `max(0, x)`.
    pub const RELU: Op = Op(0x0010);
    /// GELU: `x · Φ(x)`.
    pub const GELU: Op = Op(0x0011);
    /// SiLU / Swish: `x · σ(x)`.
    pub const SILU: Op = Op(0x0012);
    /// Sigmoid: `1 / (1 + e⁻ˣ)`.
    pub const SIGMOID: Op = Op(0x0013);
    /// Tanh.
    pub const TANH_ACT: Op = Op(0x0014);
    /// Mish: `x · tanh(softplus(x))`.
    pub const MISH: Op = Op(0x0015);
    /// Softplus: `ln(1 + eˣ)`.
    pub const SOFTPLUS: Op = Op(0x0016);

    // Normalisations 0x0020–
    /// Layer normalisation.
    pub const LAYER_NORM: Op = Op(0x0020);
    /// Batch normalisation.
    pub const BATCH_NORM: Op = Op(0x0021);
    /// RMS normalisation.
    pub const RMS_NORM: Op = Op(0x0022);
    /// Group normalisation.
    pub const GROUP_NORM: Op = Op(0x0023);
    /// Instance normalisation.
    pub const INSTANCE_NORM: Op = Op(0x0024);

    // Pooling 0x0030–
    /// Max pooling.
    pub const MAX_POOL: Op = Op(0x0030);
    /// Average pooling.
    pub const AVG_POOL: Op = Op(0x0031);
    /// Adaptive pooling (output size specified).
    pub const ADAPTIVE_POOL: Op = Op(0x0032);
    /// Global average pool → scalar per channel.
    pub const GLOBAL_POOL: Op = Op(0x0033);

    // Loss 0x0040–
    /// Mean squared error.
    pub const MSE_LOSS: Op = Op(0x0040);
    /// Cross-entropy.
    pub const CROSS_ENTROPY: Op = Op(0x0041);
    /// Binary cross-entropy.
    pub const BCE_LOSS: Op = Op(0x0042);
    /// Negative log-likelihood.
    pub const NLL_LOSS: Op = Op(0x0043);
    /// Smooth L1 / Huber.
    pub const HUBER_LOSS: Op = Op(0x0044);
    /// KL divergence.
    pub const KL_DIV: Op = Op(0x0045);

    // Optimiser steps 0x0050–
    /// SGD parameter update.
    pub const SGD_STEP: Op = Op(0x0050);
    /// Adam parameter update.
    pub const ADAM_STEP: Op = Op(0x0051);
    /// AdamW parameter update.
    pub const ADAMW_STEP: Op = Op(0x0052);
    /// RMSprop parameter update.
    pub const RMSPROP_STEP: Op = Op(0x0053);

    // Recurrent 0x0060–
    /// Vanilla RNN cell.
    pub const RNN: Op = Op(0x0060);
    /// LSTM cell.
    pub const LSTM: Op = Op(0x0061);
    /// GRU cell.
    pub const GRU: Op = Op(0x0062);

    // Positional encoding 0x0070–
    /// Sinusoidal positional encoding.
    pub const SINUSOIDAL_PE: Op = Op(0x0070);
    /// Rotary positional encoding.
    pub const ROPE: Op = Op(0x0071);
    /// Learned positional embedding.
    pub const LEARNED_PE: Op = Op(0x0072);
    /// ALiBi (attention with linear biases).
    pub const ALIBI: Op = Op(0x0073);

    // ── Symbolic 0x01xx ──────────────────────────────────────────────────

    /// Unification of two terms.
    pub const UNIFY: Op = Op(0x0100);
    /// Resolution / SLD.
    pub const RESOLVE: Op = Op(0x0101);
    /// Forward-chain inference.
    pub const INFER: Op = Op(0x0102);
    /// Pattern match.
    pub const MATCH: Op = Op(0x0103);
    /// Term rewriting.
    pub const REWRITE: Op = Op(0x0104);
    /// Assert fact into knowledge base.
    pub const ASSERT: Op = Op(0x0105);
    /// Query knowledge base.
    pub const QUERY_KB: Op = Op(0x0106);
    /// Planning (goal → action sequence).
    pub const PLAN: Op = Op(0x0107);
    /// Variable binding.
    pub const BIND_SYM: Op = Op(0x0108);
    /// Subsumption check.
    pub const SUBSUME: Op = Op(0x0109);

    // ── Control / Composition 0x02xx ─────────────────────────────────────

    /// Sequential composition: `a >> b`.
    pub const SEQ: Op = Op(0x0200);
    /// Parallel composition: `a ‖ b`.
    pub const PAR: Op = Op(0x0201);
    /// Conditional: `if p then a else b`.
    pub const COND: Op = Op(0x0202);
    /// Bounded loop.
    pub const LOOP: Op = Op(0x0203);
    /// Map function over collection.
    pub const MAP: Op = Op(0x0204);
    /// Reduce collection to scalar.
    pub const REDUCE: Op = Op(0x0205);
    /// Prefix scan.
    pub const SCAN: Op = Op(0x0206);
    /// Left fold.
    pub const FOLD: Op = Op(0x0207);
    /// Zip two streams.
    pub const ZIP: Op = Op(0x0208);
    /// Fork execution into branches.
    pub const FORK: Op = Op(0x0209);
    /// Join forked branches.
    pub const JOIN: Op = Op(0x020A);
    /// Residual (additive skip): `x + f(x)`.
    pub const RES_ADD: Op = Op(0x0210);
    /// Residual (concat skip): `cat(x, f(x))`.
    pub const RES_CAT: Op = Op(0x0211);
    /// Identity: passthrough.
    pub const IDENTITY: Op = Op(0x0212);
    /// Repeat N times.
    pub const REPEAT: Op = Op(0x0213);

    // ── Memory 0x03xx ────────────────────────────────────────────────────

    /// Allocate tensor buffer.
    pub const ALLOC: Op = Op(0x0300);
    /// Free tensor buffer.
    pub const FREE: Op = Op(0x0301);
    /// Load from address.
    pub const LOAD: Op = Op(0x0302);
    /// Store to address.
    pub const STORE: Op = Op(0x0303);
    /// Copy buffer.
    pub const COPY: Op = Op(0x0304);
    /// Reshape (zero-copy view change).
    pub const RESHAPE: Op = Op(0x0305);
    /// Transpose (permute axes).
    pub const TRANSPOSE: Op = Op(0x0306);
    /// Slice (sub-tensor view).
    pub const SLICE: Op = Op(0x0307);
    /// Concatenate tensors along axis.
    pub const CONCAT: Op = Op(0x0308);
    /// Gather (index select).
    pub const GATHER: Op = Op(0x0309);
    /// Scatter (index put).
    pub const SCATTER: Op = Op(0x030A);

    // ── Agent 0x04xx ─────────────────────────────────────────────────────

    /// Send message to agent.
    pub const SEND: Op = Op(0x0400);
    /// Receive message (blocking).
    pub const RECV: Op = Op(0x0401);
    /// Spawn child agent.
    pub const SPAWN: Op = Op(0x0402);
    /// Kill agent.
    pub const KILL: Op = Op(0x0403);
    /// Publish to topic.
    pub const PUBLISH: Op = Op(0x0404);
    /// Subscribe to topic.
    pub const SUBSCRIBE: Op = Op(0x0405);
    /// Delegate task to capable agent.
    pub const DELEGATE: Op = Op(0x0406);
    /// Broadcast to all agents.
    pub const BROADCAST: Op = Op(0x0407);

    // ── Meta / Introspection 0x05xx ──────────────────────────────────────

    /// Content hash of expression.
    pub const HASH: Op = Op(0x0500);
    /// Type-of query.
    pub const TYPE_OF: Op = Op(0x0501);
    /// Shape-of query.
    pub const SHAPE_OF: Op = Op(0x0502);
    /// Compose two functions: `f ∘ g`.
    pub const COMPOSE: Op = Op(0x0503);
    /// Decompose expression into sub-expressions.
    pub const DECOMPOSE: Op = Op(0x0504);
    /// Self-referential access (quine-like).
    pub const SELF_REF: Op = Op(0x0505);
    /// Mutate expression structurally.
    pub const MUTATE: Op = Op(0x0506);
    /// Introspect (query framework ontology).
    pub const INTROSPECT: Op = Op(0x0507);

    // ── Math / Elementwise 0x06xx ────────────────────────────────────────

    /// Elementwise add.
    pub const ADD: Op = Op(0x0600);
    /// Elementwise subtract.
    pub const SUB: Op = Op(0x0601);
    /// Elementwise multiply.
    pub const MUL: Op = Op(0x0602);
    /// Elementwise divide.
    pub const DIV: Op = Op(0x0603);
    /// Negate.
    pub const NEG: Op = Op(0x0604);
    /// Absolute value.
    pub const ABS: Op = Op(0x0605);
    /// Exponential.
    pub const EXP: Op = Op(0x0606);
    /// Natural logarithm.
    pub const LOG: Op = Op(0x0607);
    /// Square root.
    pub const SQRT: Op = Op(0x0608);
    /// Power.
    pub const POW: Op = Op(0x0609);
    /// Elementwise max.
    pub const MAX: Op = Op(0x060A);
    /// Elementwise min.
    pub const MIN: Op = Op(0x060B);
    /// Clamp to `[lo, hi]`.
    pub const CLAMP: Op = Op(0x060C);
    /// Sine.
    pub const SIN: Op = Op(0x060D);
    /// Cosine.
    pub const COS: Op = Op(0x060E);

    // ── Static catalogue ─────────────────────────────────────────────────

    /// Every opcode in the language. An agent iterates this for full
    /// instruction-set discovery.
    pub const ALL: &[Op] = &[
        // Neural
        Self::MATMUL,
        Self::LINEAR,
        Self::CONV2D,
        Self::ATTN,
        Self::EMBED,
        Self::DROP,
        Self::SOFTMAX,
        Self::RELU,
        Self::GELU,
        Self::SILU,
        Self::SIGMOID,
        Self::TANH_ACT,
        Self::MISH,
        Self::SOFTPLUS,
        Self::LAYER_NORM,
        Self::BATCH_NORM,
        Self::RMS_NORM,
        Self::GROUP_NORM,
        Self::INSTANCE_NORM,
        Self::MAX_POOL,
        Self::AVG_POOL,
        Self::ADAPTIVE_POOL,
        Self::GLOBAL_POOL,
        Self::MSE_LOSS,
        Self::CROSS_ENTROPY,
        Self::BCE_LOSS,
        Self::NLL_LOSS,
        Self::HUBER_LOSS,
        Self::KL_DIV,
        Self::SGD_STEP,
        Self::ADAM_STEP,
        Self::ADAMW_STEP,
        Self::RMSPROP_STEP,
        Self::RNN,
        Self::LSTM,
        Self::GRU,
        Self::SINUSOIDAL_PE,
        Self::ROPE,
        Self::LEARNED_PE,
        Self::ALIBI,
        // Symbolic
        Self::UNIFY,
        Self::RESOLVE,
        Self::INFER,
        Self::MATCH,
        Self::REWRITE,
        Self::ASSERT,
        Self::QUERY_KB,
        Self::PLAN,
        Self::BIND_SYM,
        Self::SUBSUME,
        // Control
        Self::SEQ,
        Self::PAR,
        Self::COND,
        Self::LOOP,
        Self::MAP,
        Self::REDUCE,
        Self::SCAN,
        Self::FOLD,
        Self::ZIP,
        Self::FORK,
        Self::JOIN,
        Self::RES_ADD,
        Self::RES_CAT,
        Self::IDENTITY,
        Self::REPEAT,
        // Memory
        Self::ALLOC,
        Self::FREE,
        Self::LOAD,
        Self::STORE,
        Self::COPY,
        Self::RESHAPE,
        Self::TRANSPOSE,
        Self::SLICE,
        Self::CONCAT,
        Self::GATHER,
        Self::SCATTER,
        // Agent
        Self::SEND,
        Self::RECV,
        Self::SPAWN,
        Self::KILL,
        Self::PUBLISH,
        Self::SUBSCRIBE,
        Self::DELEGATE,
        Self::BROADCAST,
        // Meta
        Self::HASH,
        Self::TYPE_OF,
        Self::SHAPE_OF,
        Self::COMPOSE,
        Self::DECOMPOSE,
        Self::SELF_REF,
        Self::MUTATE,
        Self::INTROSPECT,
        // Math
        Self::ADD,
        Self::SUB,
        Self::MUL,
        Self::DIV,
        Self::NEG,
        Self::ABS,
        Self::EXP,
        Self::LOG,
        Self::SQRT,
        Self::POW,
        Self::MAX,
        Self::MIN,
        Self::CLAMP,
        Self::SIN,
        Self::COS,
    ];

    // ── Queries ──────────────────────────────────────────────────────────

    /// Family (high byte).
    pub const fn family(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// Op within family (low byte).
    pub const fn local(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Decode family enum.
    pub fn family_enum(self) -> Option<OpFamily> {
        match self.family() {
            0x00 => Some(OpFamily::Neural),
            0x01 => Some(OpFamily::Symbolic),
            0x02 => Some(OpFamily::Control),
            0x03 => Some(OpFamily::Memory),
            0x04 => Some(OpFamily::Agent),
            0x05 => Some(OpFamily::Meta),
            0x06 => Some(OpFamily::Math),
            _ => None,
        }
    }

    /// Whether this op is differentiable.
    pub fn is_differentiable(self) -> bool {
        match self.family() {
            0x00 => self != Self::DROP, // all neural except dropout
            0x06 => true,               // all math
            _ => false,
        }
    }

    /// Whether this op has learnable parameters.
    pub fn has_params(self) -> bool {
        matches!(
            self,
            Self::LINEAR
                | Self::CONV2D
                | Self::ATTN
                | Self::EMBED
                | Self::LAYER_NORM
                | Self::BATCH_NORM
                | Self::RMS_NORM
                | Self::GROUP_NORM
                | Self::INSTANCE_NORM
                | Self::RNN
                | Self::LSTM
                | Self::GRU
                | Self::LEARNED_PE
        )
    }

    /// Whether the op is stateful (mutable internal state beyond parameters).
    pub fn is_stateful(self) -> bool {
        matches!(
            self,
            Self::BATCH_NORM | Self::DROP | Self::RNN | Self::LSTM | Self::GRU
        )
    }

    /// Number of required positional inputs (0 = variadic).
    pub fn arity(self) -> u8 {
        match self {
            // Unary
            Self::RELU
            | Self::GELU
            | Self::SILU
            | Self::SIGMOID
            | Self::TANH_ACT
            | Self::MISH
            | Self::SOFTPLUS
            | Self::SOFTMAX
            | Self::LAYER_NORM
            | Self::BATCH_NORM
            | Self::RMS_NORM
            | Self::GROUP_NORM
            | Self::INSTANCE_NORM
            | Self::MAX_POOL
            | Self::AVG_POOL
            | Self::ADAPTIVE_POOL
            | Self::GLOBAL_POOL
            | Self::DROP
            | Self::NEG
            | Self::ABS
            | Self::EXP
            | Self::LOG
            | Self::SQRT
            | Self::SIN
            | Self::COS
            | Self::TYPE_OF
            | Self::SHAPE_OF
            | Self::HASH
            | Self::SELF_REF
            | Self::DECOMPOSE
            | Self::INTROSPECT
            | Self::FREE
            | Self::RESHAPE
            | Self::TRANSPOSE
            | Self::IDENTITY => 1,
            // Binary
            Self::MATMUL
            | Self::LINEAR
            | Self::ADD
            | Self::SUB
            | Self::MUL
            | Self::DIV
            | Self::POW
            | Self::MAX
            | Self::MIN
            | Self::MSE_LOSS
            | Self::CROSS_ENTROPY
            | Self::BCE_LOSS
            | Self::NLL_LOSS
            | Self::HUBER_LOSS
            | Self::KL_DIV
            | Self::RES_ADD
            | Self::RES_CAT
            | Self::SEQ
            | Self::PAR
            | Self::ZIP
            | Self::SEND
            | Self::PUBLISH
            | Self::UNIFY
            | Self::RESOLVE
            | Self::BIND_SYM
            | Self::SUBSUME
            | Self::COMPOSE
            | Self::MUTATE
            | Self::STORE
            | Self::COPY
            | Self::SLICE
            | Self::CONCAT => 2,
            // Ternary
            Self::COND | Self::CLAMP | Self::CONV2D => 3,
            // Variadic / context-dependent
            _ => 0,
        }
    }

    /// Machine-readable name (no allocation, static str).
    pub fn name(self) -> &'static str {
        match self {
            Self::MATMUL => "matmul",
            Self::LINEAR => "linear",
            Self::CONV2D => "conv2d",
            Self::ATTN => "attn",
            Self::EMBED => "embed",
            Self::DROP => "drop",
            Self::SOFTMAX => "softmax",
            Self::RELU => "relu",
            Self::GELU => "gelu",
            Self::SILU => "silu",
            Self::SIGMOID => "sigmoid",
            Self::TANH_ACT => "tanh",
            Self::MISH => "mish",
            Self::SOFTPLUS => "softplus",
            Self::LAYER_NORM => "layer_norm",
            Self::BATCH_NORM => "batch_norm",
            Self::RMS_NORM => "rms_norm",
            Self::GROUP_NORM => "group_norm",
            Self::INSTANCE_NORM => "instance_norm",
            Self::MAX_POOL => "max_pool",
            Self::AVG_POOL => "avg_pool",
            Self::ADAPTIVE_POOL => "adaptive_pool",
            Self::GLOBAL_POOL => "global_pool",
            Self::MSE_LOSS => "mse_loss",
            Self::CROSS_ENTROPY => "cross_entropy",
            Self::BCE_LOSS => "bce_loss",
            Self::NLL_LOSS => "nll_loss",
            Self::HUBER_LOSS => "huber_loss",
            Self::KL_DIV => "kl_div",
            Self::SGD_STEP => "sgd_step",
            Self::ADAM_STEP => "adam_step",
            Self::ADAMW_STEP => "adamw_step",
            Self::RMSPROP_STEP => "rmsprop_step",
            Self::RNN => "rnn",
            Self::LSTM => "lstm",
            Self::GRU => "gru",
            Self::SINUSOIDAL_PE => "sinusoidal_pe",
            Self::ROPE => "rope",
            Self::LEARNED_PE => "learned_pe",
            Self::ALIBI => "alibi",
            Self::UNIFY => "unify",
            Self::RESOLVE => "resolve",
            Self::INFER => "infer",
            Self::MATCH => "match",
            Self::REWRITE => "rewrite",
            Self::ASSERT => "assert",
            Self::QUERY_KB => "query_kb",
            Self::PLAN => "plan",
            Self::BIND_SYM => "bind",
            Self::SUBSUME => "subsume",
            Self::SEQ => "seq",
            Self::PAR => "par",
            Self::COND => "cond",
            Self::LOOP => "loop",
            Self::MAP => "map",
            Self::REDUCE => "reduce",
            Self::SCAN => "scan",
            Self::FOLD => "fold",
            Self::ZIP => "zip",
            Self::FORK => "fork",
            Self::JOIN => "join",
            Self::RES_ADD => "res_add",
            Self::RES_CAT => "res_cat",
            Self::IDENTITY => "identity",
            Self::REPEAT => "repeat",
            Self::ALLOC => "alloc",
            Self::FREE => "free",
            Self::LOAD => "load",
            Self::STORE => "store",
            Self::COPY => "copy",
            Self::RESHAPE => "reshape",
            Self::TRANSPOSE => "transpose",
            Self::SLICE => "slice",
            Self::CONCAT => "concat",
            Self::GATHER => "gather",
            Self::SCATTER => "scatter",
            Self::SEND => "send",
            Self::RECV => "recv",
            Self::SPAWN => "spawn",
            Self::KILL => "kill",
            Self::PUBLISH => "publish",
            Self::SUBSCRIBE => "subscribe",
            Self::DELEGATE => "delegate",
            Self::BROADCAST => "broadcast",
            Self::HASH => "hash",
            Self::TYPE_OF => "type_of",
            Self::SHAPE_OF => "shape_of",
            Self::COMPOSE => "compose",
            Self::DECOMPOSE => "decompose",
            Self::SELF_REF => "self_ref",
            Self::MUTATE => "mutate",
            Self::INTROSPECT => "introspect",
            Self::ADD => "add",
            Self::SUB => "sub",
            Self::MUL => "mul",
            Self::DIV => "div",
            Self::NEG => "neg",
            Self::ABS => "abs",
            Self::EXP => "exp",
            Self::LOG => "log",
            Self::SQRT => "sqrt",
            Self::POW => "pow",
            Self::MAX => "max",
            Self::MIN => "min",
            Self::CLAMP => "clamp",
            Self::SIN => "sin",
            Self::COS => "cos",
            _ => "?",
        }
    }

    /// Compact description suitable for machine consumption.
    pub fn desc(self) -> &'static str {
        match self {
            Self::MATMUL => "[m,k]×[k,n]→[m,n]",
            Self::LINEAR => "x·Wᵀ+b",
            Self::CONV2D => "2d cross-correlation",
            Self::ATTN => "scaled dot-product attention",
            Self::EMBED => "lookup table",
            Self::DROP => "stochastic zero-mask",
            Self::SOFTMAX => "exp(x)/Σexp(x)",
            Self::RELU => "max(0,x)",
            Self::GELU => "x·Φ(x)",
            Self::SILU => "x·σ(x)",
            Self::SIGMOID => "1/(1+e⁻ˣ)",
            Self::TANH_ACT => "(eˣ-e⁻ˣ)/(eˣ+e⁻ˣ)",
            Self::MISH => "x·tanh(softplus(x))",
            Self::SOFTPLUS => "ln(1+eˣ)",
            Self::LAYER_NORM => "(x-μ)/σ (layer)",
            Self::BATCH_NORM => "(x-μ)/σ (batch)",
            Self::RMS_NORM => "x/√(mean(x²))",
            Self::MSE_LOSS => "mean((y-ŷ)²)",
            Self::CROSS_ENTROPY => "-Σy·log(ŷ)",
            Self::SEQ => "a>>b (category composition)",
            Self::PAR => "a‖b (product)",
            Self::COND => "if p then a else b",
            Self::RES_ADD => "x+f(x) (skip connection)",
            Self::RES_CAT => "cat(x,f(x))",
            Self::IDENTITY => "passthrough",
            Self::UNIFY => "mgu(t1,t2)",
            Self::INFER => "fwd-chain KB",
            Self::PLAN => "goal→actions",
            Self::SEND => "msg→agent",
            Self::SPAWN => "create child agent",
            Self::DELEGATE => "route task by capability",
            Self::HASH => "xxh3(expr)",
            Self::TYPE_OF => "structural type query",
            Self::SELF_REF => "quine access",
            Self::MUTATE => "structural expr rewrite",
            _ => "",
        }
    }

    /// Full metadata bundle.
    pub fn meta(self) -> OpMeta {
        OpMeta {
            op: self,
            name: self.name(),
            arity: self.arity(),
            differentiable: self.is_differentiable(),
            has_params: self.has_params(),
            stateful: self.is_stateful(),
            desc: self.desc(),
        }
    }
}

/// Queryable metadata for an opcode.
#[derive(Debug, Clone)]
pub struct OpMeta {
    /// The opcode.
    pub op: Op,
    /// Machine-readable name.
    pub name: &'static str,
    /// Positional input count (0 = variadic).
    pub arity: u8,
    /// Supports gradient computation.
    pub differentiable: bool,
    /// Has learnable weight tensors.
    pub has_params: bool,
    /// Has mutable internal state.
    pub stateful: bool,
    /// Compact description.
    pub desc: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_size() {
        assert_eq!(std::mem::size_of::<Op>(), 2);
    }

    #[test]
    fn family_decode() {
        assert_eq!(Op::MATMUL.family(), 0x00);
        assert_eq!(Op::UNIFY.family(), 0x01);
        assert_eq!(Op::SEQ.family(), 0x02);
        assert_eq!(Op::ALLOC.family(), 0x03);
        assert_eq!(Op::SEND.family(), 0x04);
        assert_eq!(Op::HASH.family(), 0x05);
        assert_eq!(Op::ADD.family(), 0x06);
    }

    #[test]
    fn all_ops_have_names() {
        for &op in Op::ALL {
            assert_ne!(op.name(), "?", "Op {:#06x} has no name", op.0);
        }
    }

    #[test]
    fn differentiable_correctness() {
        assert!(Op::LINEAR.is_differentiable());
        assert!(Op::GELU.is_differentiable());
        assert!(Op::ADD.is_differentiable());
        assert!(!Op::DROP.is_differentiable());
        assert!(!Op::SEND.is_differentiable());
        assert!(!Op::UNIFY.is_differentiable());
    }

    #[test]
    fn param_correctness() {
        assert!(Op::LINEAR.has_params());
        assert!(Op::CONV2D.has_params());
        assert!(!Op::RELU.has_params());
        assert!(!Op::ADD.has_params());
    }

    #[test]
    fn catalogue_completeness() {
        // Ensure ALL covers a substantial number of ops
        assert!(Op::ALL.len() >= 95);
    }

    #[test]
    fn meta_roundtrip() {
        let m = Op::ATTN.meta();
        assert_eq!(m.name, "attn");
        assert!(m.differentiable);
        assert!(m.has_params);
    }
}
