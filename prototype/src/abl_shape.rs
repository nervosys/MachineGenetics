//! # Agentic Binary Language Shape Inference
//!
//! A pre-flight pass that walks an Agentic Binary Language [`Expr`] pipeline and threads
//! tensor shapes layer-by-layer, catching dimension mismatches before the
//! first compute-backend dispatch.
//!
//! The inferer handles the same op set as [`crate::abl_compute`]:
//!
//! | Op          | Shape rule                                            |
//! |-------------|-------------------------------------------------------|
//! | `LINEAR`    | `[..., in]` → `[..., out]` (args `[in, out]`)        |
//! | `MATMUL`    | `[..., k]` → `[..., n]` (args `[k, n]` or `[m, k, n]`)|
//! | Activations | shape preserved                                       |
//! | `LAYER_NORM`/`RMS_NORM` | shape preserved                             |
//! | `MSE_LOSS`  | reduced to `[1]`                                      |
//! | `SGD_STEP`  | shape preserved (forward no-op)                       |
//! | unknown     | shape preserved + diagnostic                          |
//!
//! 1-D inputs are auto-reshaped to `[1, dim]` for matmul (matching
//! `abl_compute::ensure_2d`).

use rmi::lang::{Expr, Op, Val};

/// Outcome of a [`infer_shape`] run.
#[derive(Debug, Clone)]
pub struct ShapeReport {
    /// Final output shape after the pipeline executes.
    pub output_shape: Vec<usize>,
    /// Mismatches encountered (op + the conflicting shape vs expected dim).
    pub mismatches: Vec<ShapeMismatch>,
    /// Unknown opcodes that the inferer treated as shape-preserving.
    pub unknown: Vec<Op>,
}

/// A single dimension mismatch detected during inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeMismatch {
    /// The opcode where the mismatch was detected.
    pub op: Op,
    /// The shape that was being fed in.
    pub got: Vec<usize>,
    /// The dimension the op expected to find as the last axis.
    pub expected_last: usize,
}

/// Infer the output shape of a pipeline starting from `input_shape`.
pub fn infer_shape(expr: &Expr, input_shape: &[usize]) -> ShapeReport {
    let mut current = input_shape.to_vec();
    let mut mismatches = Vec::new();
    let mut unknown = Vec::new();
    walk(expr, &mut current, &mut mismatches, &mut unknown);
    ShapeReport {
        output_shape: current,
        mismatches,
        unknown,
    }
}

fn walk(
    expr: &Expr,
    current: &mut Vec<usize>,
    mismatches: &mut Vec<ShapeMismatch>,
    unknown: &mut Vec<Op>,
) {
    match expr {
        Expr::Seq(a, b) => {
            walk(a, current, mismatches, unknown);
            walk(b, current, mismatches, unknown);
        }
        // `residual { f }` lowers to `App(RES_ADD, [id, f])`: out = x + f(x).
        // For the add to type-check, f must map the input shape to itself, so we
        // thread a copy through the body and flag a non-shape-preserving body —
        // the classic residual mistake (`residual { Linear(d, d') }`, d' ≠ d).
        Expr::App(op, args) if *op == Op::RES_ADD => {
            let input = current.clone();
            if let Some(body) = args.get(1) {
                let mut body_shape = input.clone();
                walk(body, &mut body_shape, mismatches, unknown);
                if !input.is_empty() && !body_shape.is_empty() && body_shape != input {
                    mismatches.push(ShapeMismatch {
                        op: Op::RES_ADD,
                        got: body_shape,
                        expected_last: *input.last().unwrap(),
                    });
                }
            }
            // The residual preserves the input shape (`current` unchanged).
        }
        Expr::App(op, args) => apply_op(*op, args, current, mismatches, unknown),
        Expr::Par(a, b) => {
            // Branch: both paths run on the same input. Validate each path's
            // internal shapes (on a copy); the left determines the output shape,
            // matching `abl_compute::walk`. Paths may legitimately differ, so we
            // don't require them to agree.
            let mut right = current.clone();
            walk(b, &mut right, mismatches, unknown);
            walk(a, current, mismatches, unknown);
        }
        _ => {}
    }
}

fn extract_int_args(args: &[Expr]) -> Vec<i64> {
    args.iter()
        .filter_map(|a| match a {
            Expr::Lit(Val::I64(n)) => Some(*n),
            _ => None,
        })
        .collect()
}

fn apply_op(
    op: Op,
    args: &[Expr],
    current: &mut Vec<usize>,
    mismatches: &mut Vec<ShapeMismatch>,
    unknown: &mut Vec<Op>,
) {
    match op {
        // ── Shape-preserving ops ───────────────────────────────────
        Op::RELU
        | Op::GELU
        | Op::SIGMOID
        | Op::TANH_ACT
        | Op::SILU
        | Op::MISH
        | Op::SOFTPLUS
        | Op::SOFTMAX
        | Op::IDENTITY
        | Op::LAYER_NORM
        | Op::BATCH_NORM
        | Op::RMS_NORM
        | Op::GROUP_NORM
        | Op::INSTANCE_NORM
        | Op::DROP
        | Op::SGD_STEP
        | Op::ADAM_STEP
        | Op::ADAMW_STEP
        | Op::RMSPROP_STEP => {}

        // ── Reduction to scalar ────────────────────────────────────
        Op::MSE_LOSS
        | Op::CROSS_ENTROPY
        | Op::BCE_LOSS
        | Op::NLL_LOSS
        | Op::HUBER_LOSS
        | Op::KL_DIV => {
            *current = vec![1];
        }

        // ── Embedding: token ids [.., seq] → [.., seq, dim] ─────────
        // Appends the embedding dim (`Embedding(vocab, dim)` → args[1]) as a new
        // last axis, so a downstream `Linear(dim, …)` sees the right input dim
        // instead of tripping the typed-composition gate.
        Op::EMBED => {
            let dims = extract_int_args(args);
            if let Some(&edim) = dims.get(1) {
                if edim > 0 {
                    current.push(edim as usize);
                }
            }
        }

        // ── Weighted ops with explicit dims ─────────────────────────
        Op::LINEAR => {
            let dims = extract_int_args(args);
            if dims.len() == 2 && dims[0] > 0 && dims[1] > 0 {
                let in_dim = dims[0] as usize;
                let out_dim = dims[1] as usize;
                apply_matmul_like(op, current, in_dim, out_dim, mismatches);
            }
        }
        // ── Conv2D: shrinks spatial dims by (k-1). ──────────────────
        Op::CONV2D => {
            // Arg schema matches abl_compute::dispatch_conv2d:
            // [in_ch, out_ch, kernel] (+ optional bias, stride, padding).
            let dims = extract_int_args(args);
            if dims.len() >= 3 && dims[1] > 0 && dims[2] > 0 {
                let out_ch = dims[1] as usize;
                let k = dims[2] as usize;
                let stride = dims.get(4).copied().filter(|s| *s > 0).unwrap_or(1) as usize;
                let padding = dims.get(5).copied().filter(|p| *p >= 0).unwrap_or(0) as usize;
                let dilation = dims.get(6).copied().filter(|d| *d > 0).unwrap_or(1) as usize;
                // Reshape input to [N, C, H, W] like dispatch_conv2d.
                let (n, _c, h, w) = match current.as_slice() {
                    [h, w] => (1usize, 1usize, *h, *w),
                    [c, h, w] => (1usize, *c, *h, *w),
                    [n, c, h, w] => (*n, *c, *h, *w),
                    _ => return,
                };
                let eff_k = dilation * (k - 1) + 1;
                if h + 2 * padding < eff_k || w + 2 * padding < eff_k {
                    return;
                }
                let out_h = (h + 2 * padding - eff_k) / stride + 1;
                let out_w = (w + 2 * padding - eff_k) / stride + 1;
                *current = if n == 1 && current.len() <= 3 {
                    vec![out_ch, out_h, out_w]
                } else {
                    vec![n, out_ch, out_h, out_w]
                };
            }
        }

        // ── Pooling: shrinks last axis. ────────────────────────────
        Op::MAX_POOL | Op::AVG_POOL => {
            let dims = extract_int_args(args);
            let (kernel, stride) = match dims.as_slice() {
                [k] if *k > 0 => (*k as usize, *k as usize),
                [k, s] if *k > 0 && *s > 0 => (*k as usize, *s as usize),
                _ => return,
            };
            if let Some(&last) = current.last() {
                if kernel <= last {
                    let out_len = (last - kernel) / stride + 1;
                    let len = current.len();
                    current[len - 1] = out_len;
                }
            }
        }

        Op::MATMUL => {
            let dims = extract_int_args(args);
            let (k, n) = match dims.as_slice() {
                [k, n] if *k > 0 && *n > 0 => (*k as usize, *n as usize),
                [_m, k, n] if *k > 0 && *n > 0 => (*k as usize, *n as usize),
                _ => return,
            };
            apply_matmul_like(op, current, k, n, mismatches);
        }

        // ── Composition / control ops are shape-preserving for the
        // running tensor (their args are not the running data). ─────
        Op::SEQ | Op::PAR | Op::REPEAT | Op::MAP | Op::REDUCE | Op::RES_ADD => {}

        // Agent / symbolic ops — out of scope for shape inference; treat
        // as shape-preserving rather than poisoning the report.
        _ => {
            if !unknown.contains(&op) {
                unknown.push(op);
            }
        }
    }
}

/// Helper for ops that consume `[..., k]` and emit `[..., n]`. Handles
/// auto-reshape of 1D `[k * m]` into `[m, k]` to match
/// `abl_compute::ensure_2d`.
fn apply_matmul_like(
    op: Op,
    current: &mut Vec<usize>,
    k: usize,
    n: usize,
    mismatches: &mut Vec<ShapeMismatch>,
) {
    if current.len() == 1 && current[0] % k == 0 {
        let m = current[0] / k;
        *current = vec![m, n];
        return;
    }
    if current.len() >= 2 && current.last().copied() == Some(k) {
        let mut new_shape = current.clone();
        let last = new_shape.len() - 1;
        new_shape[last] = n;
        *current = new_shape;
        return;
    }
    mismatches.push(ShapeMismatch {
        op,
        got: current.clone(),
        expected_last: k,
    });
}

// ─── Module-level typed-composition gate (§4.5) ─────────────────────
//
// Composability is only useful if it is *safe*. This turns the shape inferer
// into a `--check`-time gate: every `net` is lowered and its pipeline threaded,
// so a shape-mismatched composition (`stack`/`residual`/`branch` of layers whose
// dims don't line up) is rejected with an actionable diagnostic instead of
// failing at the first compute dispatch.

/// One shape inconsistency in a `net`'s composed pipeline.
#[derive(Debug, Clone)]
pub struct NetShapeDiag {
    /// The offending `net`'s name.
    pub net: String,
    /// Actionable, human-readable explanation + fix hint.
    pub message: String,
}

/// Check every `net` in a module for shape consistency across its composed
/// pipeline. Conservative by design: a net whose entry shape can't be inferred
/// (no weighted layer to anchor the dims) is skipped, and only *definite*
/// dimension conflicts are reported — unknown ops never trip the gate.
pub fn check_module_shapes(module: &crate::ast::Module) -> Vec<NetShapeDiag> {
    let mut out = Vec::new();
    for item in &module.items {
        let crate::ast::ItemKind::Net(net) = &item.kind else {
            continue;
        };
        let expr = crate::abl_bridge::NetTranslator::translate(net).expr;
        let Some(input) = crate::abl_compute::infer_input_shape(&expr) else {
            continue;
        };
        let report = infer_shape(&expr, &input);
        for m in &report.mismatches {
            out.push(NetShapeDiag {
                net: net.name.clone(),
                message: describe_mismatch(&net.name, m),
            });
        }
    }
    out
}

/// Render a [`ShapeMismatch`] as an actionable diagnostic.
fn describe_mismatch(net: &str, m: &ShapeMismatch) -> String {
    if m.op == Op::RES_ADD {
        format!(
            "net `{net}`: residual body is not shape-preserving — it outputs {:?}, \
             but a residual `x + f(x)` requires the body to return the shape it \
             received (last dim {}). Make the body's output dim equal its input dim.",
            m.got, m.expected_last
        )
    } else {
        format!(
            "net `{net}`: shape mismatch into a `{}` layer — it expects last dim {}, \
             but the preceding layer produced {:?}. Make the producing layer's output \
             dim equal {}.",
            m.op.name(),
            m.expected_last,
            m.got,
            m.expected_last
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmi::lang::Expr;

    #[test]
    fn linear_threads_dims_through_pipeline() {
        // Linear(8, 16) >> ReLU >> Linear(16, 4) >> Sigmoid: [8] → [1, 4]
        let expr = Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(16)])
            >> Expr::op1(Op::RELU)
            >> Expr::op(Op::LINEAR, vec![Expr::int(16), Expr::int(4)])
            >> Expr::op1(Op::SIGMOID);
        let report = infer_shape(&expr, &[8]);
        assert!(report.mismatches.is_empty(), "{:?}", report.mismatches);
        assert_eq!(report.output_shape, vec![1, 4]);
    }

    #[test]
    fn detects_mismatched_inner_dim() {
        // Linear(8, 16) >> Linear(32, 4): the second Linear expects last=32,
        // but the running shape is [1, 16].
        let expr = Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(16)])
            >> Expr::op(Op::LINEAR, vec![Expr::int(32), Expr::int(4)]);
        let report = infer_shape(&expr, &[8]);
        assert_eq!(report.mismatches.len(), 1);
        assert_eq!(report.mismatches[0].op, Op::LINEAR);
        assert_eq!(report.mismatches[0].expected_last, 32);
        assert_eq!(report.mismatches[0].got, vec![1, 16]);
    }

    #[test]
    fn mse_loss_reduces_to_scalar() {
        let expr = Expr::op(Op::LINEAR, vec![Expr::int(4), Expr::int(2)]) >> Expr::op1(Op::MSE_LOSS);
        let report = infer_shape(&expr, &[4]);
        assert_eq!(report.output_shape, vec![1]);
    }

    #[test]
    fn residual_body_must_preserve_shape() {
        // residual { Linear(8, 16) }: body maps 8→16, so x + f(x) is ill-typed.
        let expr = (Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(16)])).residual();
        let report = infer_shape(&expr, &[1, 8]);
        assert_eq!(report.mismatches.len(), 1, "{:?}", report.mismatches);
        assert_eq!(report.mismatches[0].op, Op::RES_ADD);
        // A shape-preserving residual is clean.
        let ok = (Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(8)])).residual();
        assert!(infer_shape(&ok, &[1, 8]).mismatches.is_empty());
    }

    // ── Module-level typed-composition gate (§4.5) ──────

    fn check_src(src: &str) -> Vec<NetShapeDiag> {
        let module = crate::parser::parse(&crate::lexer::lex(src)).expect("parses");
        check_module_shapes(&module)
    }

    #[test]
    fn gate_rejects_non_preserving_residual() {
        let d = check_src("net Bad { residual { layer up: Linear(256, 512); } }");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].net, "Bad");
        assert!(d[0].message.contains("residual"), "{}", d[0].message);
    }

    #[test]
    fn gate_rejects_mismatched_linear_chain() {
        let d = check_src("net Chain { layer a: Linear(256, 512); layer b: Linear(256, 128); }");
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("mismatch"), "{}", d[0].message);
    }

    #[test]
    fn gate_accepts_embedding_then_linear() {
        // Embedding(vocab, 256) → Linear(256, …) is well-typed: the inferer must
        // model the embed dim, not flag a false mismatch.
        let d = check_src(
            "net LM { layer e: Embedding(50000, 256); layer l: Linear(256, 1024); }",
        );
        assert!(d.is_empty(), "embed→linear should pass: {d:?}");
    }

    #[test]
    fn gate_accepts_a_well_typed_composition() {
        // A residual whose body returns to the input dim, in a lined-up chain.
        let d = check_src(
            "net Good { layer e: Linear(256, 256); \
             residual { layer ff1: Linear(256, 1024); layer ff2: Linear(1024, 256); } }",
        );
        assert!(d.is_empty(), "well-typed net should pass: {d:?}");
    }

    #[test]
    fn conv2d_shrinks_spatial_dims_in_shape_inference() {
        // Conv2D(2, 4, 3) on [2, 6, 6] → [4, 4, 4]
        let expr = Expr::op(
            Op::CONV2D,
            vec![Expr::int(2), Expr::int(4), Expr::int(3)],
        );
        let report = infer_shape(&expr, &[2, 6, 6]);
        assert!(report.mismatches.is_empty());
        assert_eq!(report.output_shape, vec![4, 4, 4]);
    }

    #[test]
    fn conv2d_strided_padded_shape_inference() {
        // Conv2D(in=2, out=4, k=3, bias=0, stride=2, pad=1) on [2,6,6]:
        // out = (6 + 2*1 - 3)/2 + 1 = 3 → [4, 3, 3].
        let expr = Expr::op(
            Op::CONV2D,
            vec![
                Expr::int(2),
                Expr::int(4),
                Expr::int(3),
                Expr::int(0),
                Expr::int(2),
                Expr::int(1),
            ],
        );
        let report = infer_shape(&expr, &[2, 6, 6]);
        assert!(report.mismatches.is_empty());
        assert_eq!(report.output_shape, vec![4, 3, 3]);
    }

    #[test]
    fn conv2d_same_padding_preserves_spatial() {
        // Conv2D(2, 4, 3, bias=0, stride=1, pad=1) on [2,8,8] → [4,8,8]
        // ("same" conv: out = 8 + 2 - 3 + 1 = 8).
        let expr = Expr::op(
            Op::CONV2D,
            vec![
                Expr::int(2),
                Expr::int(4),
                Expr::int(3),
                Expr::int(0),
                Expr::int(1),
                Expr::int(1),
            ],
        );
        let report = infer_shape(&expr, &[2, 8, 8]);
        assert_eq!(report.output_shape, vec![4, 8, 8]);
    }

    #[test]
    fn max_pool_halves_last_dim() {
        // MaxPool(kernel=2, stride=2) on [8] → [4]
        let expr = Expr::op(Op::MAX_POOL, vec![Expr::int(2), Expr::int(2)]);
        let report = infer_shape(&expr, &[8]);
        assert_eq!(report.output_shape, vec![4]);
    }

    #[test]
    fn unknown_ops_recorded_but_dont_clobber_shape() {
        // SPAWN is an agent op, no shape semantics. Inferer treats it as
        // shape-preserving and records it as unknown.
        let expr = Expr::op1(Op::SPAWN) >> Expr::op1(Op::RELU);
        let report = infer_shape(&expr, &[4]);
        assert_eq!(report.output_shape, vec![4]);
        assert!(report.unknown.contains(&Op::SPAWN));
    }
}
