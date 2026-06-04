//! Structural type system for RMIL.
//!
//! Types are **structural**, not nominal. Two types are equal iff their
//! structure is identical. This lets agents reason about compatibility
//! without a name registry.
//!
//! Type encoding is maximally compact:
//! - Scalars: 2 bytes (tag + dtype)
//! - Tensors: 2 + 1 + N×4 bytes (tag + dtype + ndim + dims)
//! - Functions: 2 + arg_types + ret_types
//!
//! The [`Dtype`] enum covers every hardware-relevant numeric type.
//! [`DYN`] marks dimensions whose size is determined at runtime.

use std::fmt;

// ── Scalar data types ────────────────────────────────────────────────────────

/// Scalar data type — matches hardware primitives exactly.
///
/// The discriminant is the wire format (1 byte).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dtype {
    /// 1-bit boolean.
    Bool = 0,
    /// Signed 8-bit integer.
    I8 = 1,
    /// Signed 16-bit integer.
    I16 = 2,
    /// Signed 32-bit integer.
    I32 = 3,
    /// Signed 64-bit integer.
    I64 = 4,
    /// Unsigned 8-bit integer.
    U8 = 5,
    /// Unsigned 16-bit integer.
    U16 = 6,
    /// Unsigned 32-bit integer.
    U32 = 7,
    /// Unsigned 64-bit integer.
    U64 = 8,
    /// IEEE 754 half-precision float.
    F16 = 9,
    /// Brain float 16.
    BF16 = 10,
    /// IEEE 754 single-precision float.
    F32 = 11,
    /// IEEE 754 double-precision float.
    F64 = 12,
}

impl Dtype {
    /// Size in bytes.
    pub const fn size(self) -> usize {
        match self {
            Self::Bool | Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 | Self::F16 | Self::BF16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }

    /// Whether this is a floating-point type (supports gradients).
    pub const fn is_float(self) -> bool {
        matches!(self, Self::F16 | Self::BF16 | Self::F32 | Self::F64)
    }

    /// Decode from a single byte. Returns `None` on invalid discriminant.
    pub fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Bool),
            1 => Some(Self::I8),
            2 => Some(Self::I16),
            3 => Some(Self::I32),
            4 => Some(Self::I64),
            5 => Some(Self::U8),
            6 => Some(Self::U16),
            7 => Some(Self::U32),
            8 => Some(Self::U64),
            9 => Some(Self::F16),
            10 => Some(Self::BF16),
            11 => Some(Self::F32),
            12 => Some(Self::F64),
            _ => None,
        }
    }
}

impl fmt::Display for Dtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Bool => "b1",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F16 => "f16",
            Self::BF16 => "bf16",
            Self::F32 => "f32",
            Self::F64 => "f64",
        };
        f.write_str(s)
    }
}

// ── Shape ────────────────────────────────────────────────────────────────────

/// Tensor shape — a list of dimension sizes.
pub type Shape = Vec<usize>;

/// Sentinel value for a dynamic (runtime-determined) dimension.
pub const DYN: usize = usize::MAX;

// ── Structural Type ──────────────────────────────────────────────────────────

/// Structural type of every RMIL expression.
///
/// Types compose algebraically:
/// - `Tuple(A, B)` = product A × B
/// - `Union(A, B)` = coproduct A + B
/// - `Fn([A], [B])` = morphism A → B
///
/// An AI agent can check type compatibility via structural equality
/// and compose operations by matching output types to input types.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    /// Unit / void — zero information.
    Void,
    /// Scalar value of a specific dtype.
    Scalar(Dtype),
    /// N-dimensional tensor: dtype + shape.
    Tensor(Dtype, Shape),
    /// Interned symbol reference.
    Sym,
    /// Function: input types → output types.
    Fn(Vec<Ty>, Vec<Ty>),
    /// Product type (tuple): A × B × C × ...
    Tuple(Vec<Ty>),
    /// Sum type (tagged union): A | B | C | ...
    Union(Vec<Ty>),
    /// Type variable (for parametric polymorphism / inference).
    Var(u32),
    /// Opaque agent-defined type (payload is a type ID).
    Opaque(u32),
}

impl Ty {
    // ── Constructors ─────────────────────────────────────────────────────

    /// `f32` scalar.
    pub fn f32() -> Self {
        Self::Scalar(Dtype::F32)
    }
    /// `f64` scalar.
    pub fn f64() -> Self {
        Self::Scalar(Dtype::F64)
    }
    /// `i64` scalar.
    pub fn i64() -> Self {
        Self::Scalar(Dtype::I64)
    }
    /// Boolean scalar.
    pub fn bool() -> Self {
        Self::Scalar(Dtype::Bool)
    }
    /// Typed tensor with concrete shape.
    pub fn tensor(dtype: Dtype, shape: &[usize]) -> Self {
        Self::Tensor(dtype, shape.to_vec())
    }
    /// F32 tensor.
    pub fn f32t(shape: &[usize]) -> Self {
        Self::tensor(Dtype::F32, shape)
    }
    /// Morphism from inputs to outputs.
    pub fn morphism(inputs: Vec<Ty>, outputs: Vec<Ty>) -> Self {
        Self::Fn(inputs, outputs)
    }

    // ── Queries ──────────────────────────────────────────────────────────

    /// Whether this type supports gradient computation.
    pub fn is_differentiable(&self) -> bool {
        match self {
            Self::Scalar(d) | Self::Tensor(d, _) => d.is_float(),
            Self::Tuple(ts) => ts.iter().all(|t| t.is_differentiable()),
            _ => false,
        }
    }

    /// Number of scalar elements (None if any dimension is dynamic).
    pub fn numel(&self) -> Option<usize> {
        match self {
            Self::Scalar(_) => Some(1),
            Self::Tensor(_, s) => {
                if s.contains(&DYN) {
                    None
                } else {
                    Some(s.iter().product())
                }
            }
            _ => None,
        }
    }

    /// Byte size (None if dynamic or non-data type).
    pub fn size_bytes(&self) -> Option<usize> {
        match self {
            Self::Void => Some(0),
            Self::Scalar(d) => Some(d.size()),
            Self::Tensor(d, _) => self.numel().map(|n| n * d.size()),
            Self::Sym => Some(4),
            _ => None,
        }
    }

    /// Number of dimensions (None for non-tensor types).
    pub fn ndim(&self) -> Option<usize> {
        match self {
            Self::Tensor(_, s) => Some(s.len()),
            Self::Scalar(_) => Some(0),
            _ => None,
        }
    }

    /// Whether output type of `self` matches input type of `other`,
    /// enabling sequential composition `self >> other`.
    pub fn composes_with(&self, other: &Ty) -> bool {
        match (self, other) {
            (Ty::Fn(_, out), Ty::Fn(inp, _)) => out == inp,
            _ => self == other,
        }
    }

    /// Wire-format tag byte.
    pub(crate) fn tag(&self) -> u8 {
        match self {
            Self::Void => 0,
            Self::Scalar(_) => 1,
            Self::Tensor(_, _) => 2,
            Self::Sym => 3,
            Self::Fn(_, _) => 4,
            Self::Tuple(_) => 5,
            Self::Union(_) => 6,
            Self::Var(_) => 7,
            Self::Opaque(_) => 8,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Void => f.write_str("∅"),
            Self::Scalar(d) => write!(f, "{d}"),
            Self::Tensor(d, s) => {
                write!(f, "{d}[")?;
                for (i, dim) in s.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    if *dim == DYN {
                        f.write_str("?")?;
                    } else {
                        write!(f, "{dim}")?;
                    }
                }
                f.write_str("]")
            }
            Self::Sym => f.write_str("sym"),
            Self::Fn(inp, out) => {
                f.write_str("(")?;
                for (i, t) in inp.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{t}")?;
                }
                f.write_str("→")?;
                for (i, t) in out.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{t}")?;
                }
                f.write_str(")")
            }
            Self::Tuple(ts) => {
                f.write_str("(")?;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        f.write_str("×")?;
                    }
                    write!(f, "{t}")?;
                }
                f.write_str(")")
            }
            Self::Union(ts) => {
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        f.write_str("|")?;
                    }
                    write!(f, "{t}")?;
                }
                Ok(())
            }
            Self::Var(n) => write!(f, "τ{n}"),
            Self::Opaque(n) => write!(f, "⊥{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtype_sizes() {
        assert_eq!(Dtype::Bool.size(), 1);
        assert_eq!(Dtype::F16.size(), 2);
        assert_eq!(Dtype::F32.size(), 4);
        assert_eq!(Dtype::F64.size(), 8);
    }

    #[test]
    fn dtype_roundtrip() {
        for i in 0..=12u8 {
            let d = Dtype::from_u8(i).unwrap();
            assert_eq!(d as u8, i);
        }
        assert!(Dtype::from_u8(13).is_none());
    }

    #[test]
    fn tensor_numel() {
        let t = Ty::f32t(&[2, 3, 4]);
        assert_eq!(t.numel(), Some(24));
        assert_eq!(t.size_bytes(), Some(96));
        assert_eq!(t.ndim(), Some(3));
    }

    #[test]
    fn dynamic_dims() {
        let t = Ty::tensor(Dtype::F32, &[DYN, 768]);
        assert_eq!(t.numel(), None);
        assert_eq!(t.size_bytes(), None);
    }

    #[test]
    fn differentiable() {
        assert!(Ty::f32().is_differentiable());
        assert!(!Ty::i64().is_differentiable());
        assert!(Ty::f32t(&[3, 4]).is_differentiable());
    }

    #[test]
    fn type_display() {
        assert_eq!(format!("{}", Ty::Void), "∅");
        assert_eq!(format!("{}", Ty::f32()), "f32");
        assert_eq!(format!("{}", Ty::f32t(&[2, DYN, 768])), "f32[2,?,768]");
        assert_eq!(
            format!("{}", Ty::morphism(vec![Ty::f32()], vec![Ty::f64()])),
            "(f32→f64)"
        );
    }

    #[test]
    fn composition_check() {
        let a = Ty::morphism(vec![Ty::f32()], vec![Ty::f64()]);
        let b = Ty::morphism(vec![Ty::f64()], vec![Ty::bool()]);
        let c = Ty::morphism(vec![Ty::i64()], vec![Ty::bool()]);
        assert!(a.composes_with(&b)); // f32→f64 >> f64→bool ✓
        assert!(!a.composes_with(&c)); // f32→f64 >> i64→bool ✗
    }

    #[test]
    fn type_size() {
        // Ty is an enum with Vec variants; check it's within bounds
        assert!(std::mem::size_of::<Ty>() <= 80);
    }
}
