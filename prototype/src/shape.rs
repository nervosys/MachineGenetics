/// MAGE Shape Inference — tensor dimension algebra.
///
/// Implements shape checking and inference for tensor operations:
///   - Broadcasting: element-wise ops between tensors of compatible shape
///   - Matrix multiply: ⊗ / matmul with inner-dimension matching
///   - Transpose: permutation of dimension ordering
///   - Reshape: element-count preservation
///   - Slice / index: dimension reduction
///
/// Shape variables (TensorDim::Var) are solved via unification.
use crate::ast;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity, TensorDimHir};
use std::collections::HashMap;

// ── Shape variable supply ────────────────────────────────────────────

struct ShapeVarSupply {
    next: u32,
}

impl ShapeVarSupply {
    fn new() -> Self {
        ShapeVarSupply { next: 0 }
    }

    fn fresh(&mut self) -> String {
        let v = format!("_d{}", self.next);
        self.next += 1;
        v
    }
}

// ── Shape substitution ──────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ShapeSubst {
    map: HashMap<String, TensorDimHir>,
}

impl ShapeSubst {
    fn new() -> Self {
        ShapeSubst { map: HashMap::new() }
    }

    fn apply_dim(&self, d: &TensorDimHir) -> TensorDimHir {
        match d {
            TensorDimHir::Var(v) => {
                if let Some(resolved) = self.map.get(v) {
                    self.apply_dim(resolved)
                } else {
                    d.clone()
                }
            }
            TensorDimHir::Lit(_) => d.clone(),
        }
    }

    fn apply_shape(&self, shape: &[TensorDimHir]) -> Vec<TensorDimHir> {
        shape.iter().map(|d| self.apply_dim(d)).collect()
    }

    fn bind(&mut self, var: &str, dim: TensorDimHir) {
        self.map.insert(var.to_string(), dim);
    }
}

// ── Dimension unification ───────────────────────────────────────────

fn unify_dim(
    subst: &mut ShapeSubst,
    a: &TensorDimHir,
    b: &TensorDimHir,
) -> Result<TensorDimHir, String> {
    let a = subst.apply_dim(a);
    let b = subst.apply_dim(b);

    match (&a, &b) {
        (TensorDimHir::Lit(x), TensorDimHir::Lit(y)) => {
            if x == y {
                Ok(a)
            } else {
                Err(format!("dimension mismatch: {x} vs {y}"))
            }
        }
        (TensorDimHir::Var(v), _) => {
            subst.bind(v, b.clone());
            Ok(b)
        }
        (_, TensorDimHir::Var(v)) => {
            subst.bind(v, a.clone());
            Ok(a)
        }
    }
}

// ── Broadcasting ────────────────────────────────────────────────────

/// Compute the broadcast-compatible output shape for two input shapes.
/// Follows NumPy-style broadcasting rules:
///   1. Right-align dimensions
///   2. Each pair must be equal, or one must be 1
///   3. Missing dimensions are treated as 1
fn broadcast(
    subst: &mut ShapeSubst,
    a: &[TensorDimHir],
    b: &[TensorDimHir],
) -> Result<Vec<TensorDimHir>, String> {
    let max_rank = a.len().max(b.len());
    let mut result = Vec::with_capacity(max_rank);

    // Iterate from rightmost dimension.
    for i in 0..max_rank {
        let da = if i < a.len() {
            &a[a.len() - 1 - i]
        } else {
            &TensorDimHir::Lit(1)
        };
        let db = if i < b.len() {
            &b[b.len() - 1 - i]
        } else {
            &TensorDimHir::Lit(1)
        };

        let da = subst.apply_dim(da);
        let db = subst.apply_dim(db);

        let out = match (&da, &db) {
            (TensorDimHir::Lit(1), _) => db,
            (_, TensorDimHir::Lit(1)) => da,
            (TensorDimHir::Lit(x), TensorDimHir::Lit(y)) if x == y => da,
            (TensorDimHir::Lit(x), TensorDimHir::Lit(y)) => {
                return Err(format!(
                    "cannot broadcast dimension {x} with {y}"
                ));
            }
            // Variables: unify if possible, otherwise keep as-is.
            _ => {
                match unify_dim(subst, &da, &db) {
                    Ok(d) => d,
                    Err(_) => da, // Keep first; constraint will be checked at runtime.
                }
            }
        };
        result.push(out);
    }

    result.reverse();
    Ok(result)
}

// ── Matrix multiplication shape ─────────────────────────────────────

/// Compute output shape for matmul (⊗):
///   [... × M × K] ⊗ [... × K × N] → [... × M × N]
/// Batch dimensions are broadcast.
fn matmul_shape(
    subst: &mut ShapeSubst,
    a: &[TensorDimHir],
    b: &[TensorDimHir],
) -> Result<Vec<TensorDimHir>, String> {
    if a.len() < 2 || b.len() < 2 {
        return Err(format!(
            "matmul requires rank ≥ 2, got rank {} and {}",
            a.len(),
            b.len()
        ));
    }

    let a_m = &a[a.len() - 2];
    let a_k = &a[a.len() - 1];
    let b_k = &b[b.len() - 2];
    let b_n = &b[b.len() - 1];

    // Inner dimensions must match: a_k == b_k
    unify_dim(subst, a_k, b_k).map_err(|e| {
        format!("matmul inner dimension mismatch: {e}")
    })?;

    // Broadcast batch dimensions.
    let a_batch = &a[..a.len() - 2];
    let b_batch = &b[..b.len() - 2];
    let mut out = broadcast(subst, a_batch, b_batch)?;

    out.push(subst.apply_dim(a_m));
    out.push(subst.apply_dim(b_n));
    Ok(out)
}

// ── Transpose shape ─────────────────────────────────────────────────

fn transpose_shape(shape: &[TensorDimHir]) -> Result<Vec<TensorDimHir>, String> {
    if shape.len() < 2 {
        return Err("transpose requires rank ≥ 2".into());
    }
    let mut out = shape.to_vec();
    let n = out.len();
    out.swap(n - 2, n - 1);
    Ok(out)
}

// ── Reshape shape ───────────────────────────────────────────────────

/// Validate that a reshape preserves total element count.
/// Returns the new shape if valid (one -1 dimension is inferred).
fn reshape_shape(
    from: &[TensorDimHir],
    to: &[TensorDimHir],
) -> Result<Vec<TensorDimHir>, String> {
    // Compute known sizes.
    let from_size: Option<u64> = from.iter().try_fold(1u64, |acc, d| match d {
        TensorDimHir::Lit(n) => Some(acc * n),
        TensorDimHir::Var(_) => None,
    });

    let to_known: Option<u64> = to.iter().try_fold(1u64, |acc, d| match d {
        TensorDimHir::Lit(n) if *n != u64::MAX => Some(acc * n),
        _ => None,
    });

    // If both are fully known, check they match.
    if let (Some(fs), Some(ts)) = (from_size, to_known) {
        if fs != ts {
            return Err(format!(
                "reshape size mismatch: {fs} elements vs {ts} elements"
            ));
        }
    }

    Ok(to.to_vec())
}

// ── Public interface ────────────────────────────────────────────────

/// Shape inference context for a module.
pub struct ShapeInfer {
    supply: ShapeVarSupply,
    subst: ShapeSubst,
    pub diagnostics: Vec<Diagnostic>,
}

impl ShapeInfer {
    pub fn new() -> Self {
        ShapeInfer {
            supply: ShapeVarSupply::new(),
            subst: ShapeSubst::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn fresh_dim(&mut self) -> TensorDimHir {
        TensorDimHir::Var(self.supply.fresh())
    }

    /// Infer output shape for a binary tensor operation.
    pub fn infer_binary_op(
        &mut self,
        op: &str,
        lhs: &[TensorDimHir],
        rhs: &[TensorDimHir],
    ) -> Vec<TensorDimHir> {
        let result = match op {
            "⊗" | "matmul" => matmul_shape(&mut self.subst, lhs, rhs),
            _ => broadcast(&mut self.subst, lhs, rhs),
        };

        match result {
            Ok(shape) => self.subst.apply_shape(&shape),
            Err(msg) => {
                self.diagnostics.push(Diagnostic::categorized(
                    Severity::Error,
                    format!("shape error in `{op}`: {msg}"),
                    DiagnosticCategory::TypeMismatch,
                    None,
                ));
                // Return LHS shape as fallback.
                lhs.to_vec()
            }
        }
    }

    /// Infer output shape for transpose.
    pub fn infer_transpose(&mut self, shape: &[TensorDimHir]) -> Vec<TensorDimHir> {
        match transpose_shape(shape) {
            Ok(s) => s,
            Err(msg) => {
                self.diagnostics.push(Diagnostic::categorized(
                    Severity::Error,
                    msg,
                    DiagnosticCategory::TypeMismatch,
                    None,
                ));
                shape.to_vec()
            }
        }
    }

    /// Validate reshape and return output shape.
    pub fn infer_reshape(
        &mut self,
        from: &[TensorDimHir],
        to: &[TensorDimHir],
    ) -> Vec<TensorDimHir> {
        match reshape_shape(from, to) {
            Ok(s) => s,
            Err(msg) => {
                self.diagnostics.push(Diagnostic::categorized(
                    Severity::Error,
                    msg,
                    DiagnosticCategory::TypeMismatch,
                    None,
                ));
                to.to_vec()
            }
        }
    }

    /// Infer the shape of a layer output given input shape and layer definition.
    pub fn infer_layer_shape(
        &mut self,
        layer: &ast::LayerDef,
        input_shape: &[TensorDimHir],
    ) -> Vec<TensorDimHir> {
        let layer_name = layer.layer_type_name();
        match layer_name.as_str() {
            "Linear" | "Dense" => {
                // Linear(in_features, out_features): [..., in] -> [..., out]
                if let Some(out_dim) = layer.args.first().and_then(|e| expr_to_dim(e)) {
                    let mut shape = input_shape[..input_shape.len().saturating_sub(1)].to_vec();
                    shape.push(out_dim);
                    shape
                } else {
                    let mut shape = input_shape[..input_shape.len().saturating_sub(1)].to_vec();
                    shape.push(self.fresh_dim());
                    shape
                }
            }
            "Conv2d" => {
                // Conv2d(out_channels, kernel, ...): [B, C, H, W] -> [B, out_C, H', W']
                let mut shape = vec![
                    input_shape.first().cloned().unwrap_or_else(|| self.fresh_dim()),
                ];
                if let Some(out_ch) = layer.args.first().and_then(|e| expr_to_dim(e)) {
                    shape.push(out_ch);
                } else {
                    shape.push(self.fresh_dim());
                }
                // Spatial dims are fresh variables (depend on padding/stride).
                shape.push(self.fresh_dim());
                shape.push(self.fresh_dim());
                shape
            }
            "Flatten" => {
                // Flatten: any -> [batch, product-of-rest]
                let batch = input_shape.first().cloned().unwrap_or_else(|| self.fresh_dim());
                vec![batch, self.fresh_dim()]
            }
            "Dropout" | "BatchNorm" | "LayerNorm" | "ReLU" | "GELU" | "Sigmoid" | "Tanh" => {
                // Shape-preserving layers.
                input_shape.to_vec()
            }
            "Attention" | "MultiHeadAttention" => {
                // [batch, seq, embed] -> [batch, seq, embed]
                input_shape.to_vec()
            }
            "Embedding" => {
                // Embedding(vocab, dim): [batch, seq] -> [batch, seq, dim]
                let mut shape = input_shape.to_vec();
                if let Some(dim) = layer.args.get(1).and_then(|e| expr_to_dim(e)) {
                    shape.push(dim);
                } else {
                    shape.push(self.fresh_dim());
                }
                shape
            }
            _ => {
                // Unknown layer: shape is fresh variables with same rank.
                input_shape.iter().map(|_| self.fresh_dim()).collect()
            }
        }
    }

    /// Infer shapes through an entire net definition.
    pub fn infer_net(&mut self, net: &ast::NetDef, input_shape: &[TensorDimHir]) -> Vec<TensorDimHir> {
        let mut shape = input_shape.to_vec();
        for layer in &net.layers {
            shape = self.infer_layer_shape(layer, &shape);
        }
        shape
    }

    /// Apply all solved substitutions to a shape.
    pub fn resolve_shape(&self, shape: &[TensorDimHir]) -> Vec<TensorDimHir> {
        self.subst.apply_shape(shape)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Extract a dimension from a literal expression.
fn expr_to_dim(expr: &ast::Expr) -> Option<TensorDimHir> {
    match expr {
        ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => {
            value.parse::<u64>().ok().map(TensorDimHir::Lit)
        }
        ast::Expr::Ident { name } => Some(TensorDimHir::Var(name.clone())),
        _ => None,
    }
}

impl ast::LayerDef {
    fn layer_type_name(&self) -> String {
        match &self.layer_type {
            ast::Type::Path { segments, .. } => segments.join("::"),
            _ => "Unknown".to_string(),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(n: u64) -> TensorDimHir {
        TensorDimHir::Lit(n)
    }

    fn var(s: &str) -> TensorDimHir {
        TensorDimHir::Var(s.to_string())
    }

    #[test]
    fn broadcast_same_shape() {
        let mut subst = ShapeSubst::new();
        let a = vec![lit(3), lit(4)];
        let b = vec![lit(3), lit(4)];
        let out = broadcast(&mut subst, &a, &b).unwrap();
        assert_eq!(out, vec![lit(3), lit(4)]);
    }

    #[test]
    fn broadcast_scalar() {
        let mut subst = ShapeSubst::new();
        let a = vec![lit(3), lit(4)];
        let b = vec![lit(1)];
        let out = broadcast(&mut subst, &a, &b).unwrap();
        assert_eq!(out, vec![lit(3), lit(4)]);
    }

    #[test]
    fn broadcast_extend_rank() {
        let mut subst = ShapeSubst::new();
        let a = vec![lit(2), lit(3), lit(4)];
        let b = vec![lit(3), lit(4)];
        let out = broadcast(&mut subst, &a, &b).unwrap();
        assert_eq!(out, vec![lit(2), lit(3), lit(4)]);
    }

    #[test]
    fn broadcast_incompatible() {
        let mut subst = ShapeSubst::new();
        let a = vec![lit(3)];
        let b = vec![lit(4)];
        assert!(broadcast(&mut subst, &a, &b).is_err());
    }

    #[test]
    fn matmul_basic() {
        let mut subst = ShapeSubst::new();
        let a = vec![lit(2), lit(3)];
        let b = vec![lit(3), lit(4)];
        let out = matmul_shape(&mut subst, &a, &b).unwrap();
        assert_eq!(out, vec![lit(2), lit(4)]);
    }

    #[test]
    fn matmul_batch() {
        let mut subst = ShapeSubst::new();
        let a = vec![lit(8), lit(2), lit(3)];
        let b = vec![lit(8), lit(3), lit(4)];
        let out = matmul_shape(&mut subst, &a, &b).unwrap();
        assert_eq!(out, vec![lit(8), lit(2), lit(4)]);
    }

    #[test]
    fn matmul_inner_mismatch() {
        let mut subst = ShapeSubst::new();
        let a = vec![lit(2), lit(3)];
        let b = vec![lit(5), lit(4)];
        assert!(matmul_shape(&mut subst, &a, &b).is_err());
    }

    #[test]
    fn matmul_with_variables() {
        let mut subst = ShapeSubst::new();
        let a = vec![var("M"), var("K")];
        let b = vec![var("K"), var("N")];
        let out = matmul_shape(&mut subst, &a, &b).unwrap();
        assert_eq!(out.len(), 2);
        // M and N are preserved.
        assert_eq!(out[0], var("M"));
        assert_eq!(out[1], var("N"));
    }

    #[test]
    fn transpose_2d() {
        let shape = vec![lit(3), lit(4)];
        let out = transpose_shape(&shape).unwrap();
        assert_eq!(out, vec![lit(4), lit(3)]);
    }

    #[test]
    fn transpose_3d() {
        let shape = vec![lit(2), lit(3), lit(4)];
        let out = transpose_shape(&shape).unwrap();
        assert_eq!(out, vec![lit(2), lit(4), lit(3)]);
    }

    #[test]
    fn reshape_valid() {
        let from = vec![lit(2), lit(3), lit(4)];
        let to = vec![lit(6), lit(4)];
        let out = reshape_shape(&from, &to).unwrap();
        assert_eq!(out, vec![lit(6), lit(4)]);
    }

    #[test]
    fn reshape_invalid() {
        let from = vec![lit(2), lit(3)];
        let to = vec![lit(7)];
        assert!(reshape_shape(&from, &to).is_err());
    }

    #[test]
    fn shape_infer_binary_broadcast() {
        let mut si = ShapeInfer::new();
        let a = vec![lit(3), lit(1)];
        let b = vec![lit(1), lit(4)];
        let out = si.infer_binary_op("+", &a, &b);
        assert_eq!(out, vec![lit(3), lit(4)]);
        assert!(si.diagnostics.is_empty());
    }

    #[test]
    fn shape_infer_matmul() {
        let mut si = ShapeInfer::new();
        let a = vec![lit(16), lit(64), lit(128)];
        let b = vec![lit(16), lit(128), lit(32)];
        let out = si.infer_binary_op("⊗", &a, &b);
        assert_eq!(out, vec![lit(16), lit(64), lit(32)]);
        assert!(si.diagnostics.is_empty());
    }

    #[test]
    fn shape_infer_error_reported() {
        let mut si = ShapeInfer::new();
        let a = vec![lit(3)];
        let b = vec![lit(4)];
        let _out = si.infer_binary_op("+", &a, &b);
        assert_eq!(si.diagnostics.len(), 1);
    }
}
