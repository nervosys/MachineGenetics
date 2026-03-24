//! # std::tensor — Tensor Algebra
//!
//! First-class tensor types with compile-time shape checking,
//! automatic hardware dispatch, and built-in autograd.

// ---------------------------------------------------------------------------
// Tensor type
// ---------------------------------------------------------------------------

/// A multi-dimensional array with compile-time shape checking.
/// Hardware dispatch is automatic: NPU → GPU → CPU SIMD → scalar.
pub struct Tensor<T, const SHAPE: [usize]> {
    data: *mut T,
    shape: [usize],
    strides: [usize],
    device: Device,
}

/// A learnable parameter — a tensor tracked for autograd.
pub struct Param<T, const SHAPE: [usize]> {
    tensor: Tensor<T, SHAPE>,
    grad: Option<Tensor<T, SHAPE>>,
    requires_grad: bool,
}

/// Hardware device for tensor storage.
pub enum Device {
    Cpu,
    Gpu(usize),
    Npu(usize),
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

impl<T, const S: [usize]> Tensor<T, S> {
    /// Create a tensor filled with zeros.
    pub fn zeros() -> Tensor<T, S>;

    /// Create a tensor filled with ones.
    pub fn ones() -> Tensor<T, S>;

    /// Create a tensor with random normal values.
    pub fn randn() -> Tensor<T, S> / rng;

    /// Create a tensor with random uniform values in [0, 1).
    pub fn rand() -> Tensor<T, S> / rng;

    /// Create a tensor from a slice.
    pub fn from_slice(data: &[T]) -> Tensor<T, S>;

    /// Create an identity matrix (2D only).
    pub fn eye() -> Tensor<T, S>;

    /// Create a tensor filled with a value.
    pub fn full(value: T) -> Tensor<T, S>;
}

impl<T, const S: [usize]> Param<T, S> {
    /// Create a learnable parameter initialized with random normal values.
    pub fn randn() -> Param<T, S> / rng;

    /// Create from an existing tensor.
    pub fn from_tensor(tensor: Tensor<T, S>) -> Param<T, S>;

    /// Access the underlying tensor.
    pub fn tensor(&self) -> &Tensor<T, S>;

    /// Access the gradient (if computed).
    pub fn grad(&self) -> Option<&Tensor<T, S>>;

    /// Zero the gradient.
    pub fn zero_grad(&mut self);
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

impl<T, const S: [usize]> Tensor<T, S> {
    /// Reshape to a new shape (total elements must match).
    pub fn reshape<const S2: [usize]>(&self) -> Tensor<T, S2>;

    /// Flatten to a 1D tensor.
    pub fn flatten(&self) -> Tensor<T, [_]>;

    /// Transpose (swap last two dimensions).
    pub fn T(&self) -> Tensor<T, _>;

    /// Sum all elements.
    pub fn sum(&self) -> Tensor<T, []>;

    /// Mean of all elements.
    pub fn mean(&self) -> Tensor<T, []>;

    /// Mean along an axis.
    pub fn mean_axis(&self, axis: usize) -> Tensor<T, _>;

    /// Sum along an axis.
    pub fn sum_axis(&self, axis: usize) -> Tensor<T, _>;

    /// Maximum element.
    pub fn max(&self) -> T;

    /// Minimum element.
    pub fn min(&self) -> T;

    /// Argmax along an axis.
    pub fn argmax(&self, axis: usize) -> Tensor<i64, _>;

    /// Slice the tensor.
    pub fn slice(&self, ranges: &[Range<usize>]) -> Tensor<T, _>;

    /// Get a single scalar value (0D tensor).
    pub fn item(&self) -> T;

    /// Move tensor to a device.
    pub fn to(&self, device: Device) -> Tensor<T, S> / gpu;

    /// Number of dimensions.
    pub fn ndim(&self) -> usize;

    /// Total number of elements.
    pub fn numel(&self) -> usize;
}

// ---------------------------------------------------------------------------
// Tensor operators (overloaded)
// ---------------------------------------------------------------------------

// A + B: element-wise addition (shapes must match or broadcast)
// A - B: element-wise subtraction
// A * B: element-wise multiplication (use .* in explicit form)
// A / B: element-wise division
// A @ B: matrix multiplication (MATMUL operator)
//   [M, K] @ [K, N] -> [M, N]
// A.T: transpose

// ---------------------------------------------------------------------------
// Autograd
// ---------------------------------------------------------------------------

/// Compute gradients of `loss` with respect to `params`.
/// The loss must be a scalar (0D tensor).
/// All operations in the computation graph must be differentiable.
pub fn grad<T>(
    loss: Tensor<T, []>,
    params: &[&Param<T, _>],
) -> Vec<Tensor<T, _>>;

// ---------------------------------------------------------------------------
// Concatenation and stacking
// ---------------------------------------------------------------------------

/// Concatenate tensors along an axis.
pub fn cat<T, const S: [usize]>(tensors: &[Tensor<T, S>], axis: usize) -> Tensor<T, _>;

/// Stack tensors along a new axis.
pub fn stack<T, const S: [usize]>(tensors: &[Tensor<T, S>]) -> Tensor<T, _>;

// ---------------------------------------------------------------------------
// SIMD types (CPU acceleration)
// ---------------------------------------------------------------------------

/// 128-bit SIMD: 4 × f32
pub struct f32x4;

/// 256-bit SIMD: 8 × f32
pub struct f32x8;

/// 256-bit SIMD: 4 × f64
pub struct f64x4;

/// 512-bit SIMD: 16 × f32 (AVX-512)
pub struct f32x16;
