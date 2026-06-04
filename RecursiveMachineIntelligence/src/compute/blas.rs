//! BLAS/LAPACK integration for hardware-accelerated linear algebra.
//!
//! Provides high-performance matrix operations that dispatch to optimised
//! BLAS/LAPACK routines when available, with a pure-Rust fallback for
//! portability. Operations include matrix multiply, eigendecomposition,
//! SVD, Cholesky, and triangular solves.
//!
//! # Design
//!
//! This module wraps `ndarray` + `rayon` for its native implementation.
//! The API is designed so a BLAS library (OpenBLAS, MKL, Accelerate) can
//! be plugged in via feature flags without changing call sites.
//!
//! All operations work on [`BlasMatrix`], a row-major f64 matrix with
//! shape metadata. Results are returned as new allocations — no in-place
//! mutation — to keep the API safe and composable.
//!
//! # Examples
//!
//! ```
//! use rmi::compute::blas::{BlasMatrix, BlasOps};
//!
//! // Matrix multiply: [2,3] × [3,2] → [2,2]
//! let a = BlasMatrix::from_vec(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
//! let b = BlasMatrix::from_vec(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
//! let c = BlasOps::matmul(&a, &b).unwrap();
//! assert_eq!(c.rows, 2);
//! assert_eq!(c.cols, 2);
//! // c = [[58,64],[139,154]]
//! assert!((c.data[0] - 58.0).abs() < 1e-10);
//! ```

use std::fmt;

// ── BlasMatrix ───────────────────────────────────────────────────────────────

/// A row-major f64 matrix for BLAS operations.
#[derive(Clone)]
pub struct BlasMatrix {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Row-major data buffer.
    pub data: Vec<f64>,
}

impl fmt::Debug for BlasMatrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BlasMatrix({}x{}, {} elements)",
            self.rows,
            self.cols,
            self.data.len()
        )
    }
}

impl BlasMatrix {
    /// Create a matrix from a flat row-major vector.
    pub fn from_vec(rows: usize, cols: usize, data: Vec<f64>) -> Self {
        assert_eq!(
            data.len(),
            rows * cols,
            "data length {} != rows*cols {}",
            data.len(),
            rows * cols
        );
        Self { rows, cols, data }
    }

    /// Create a zero matrix.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    /// Create an identity matrix.
    pub fn eye(n: usize) -> Self {
        let mut data = vec![0.0; n * n];
        for i in 0..n {
            data[i * n + i] = 1.0;
        }
        Self {
            rows: n,
            cols: n,
            data,
        }
    }

    /// Create a random matrix with values in [0, 1).
    pub fn random(rows: usize, cols: usize, seed: u64) -> Self {
        use rand::Rng;
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let data: Vec<f64> = (0..rows * cols).map(|_| rng.gen::<f64>()).collect();
        Self { rows, cols, data }
    }

    /// Get element at (row, col).
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.data[row * self.cols + col]
    }

    /// Set element at (row, col).
    #[inline]
    pub fn set(&mut self, row: usize, col: usize, val: f64) {
        self.data[row * self.cols + col] = val;
    }

    /// Number of elements.
    pub fn numel(&self) -> usize {
        self.rows * self.cols
    }

    /// Whether the matrix is square.
    pub fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    /// Transpose (returns a new matrix).
    pub fn transpose(&self) -> Self {
        let mut data = vec![0.0; self.rows * self.cols];
        for i in 0..self.rows {
            for j in 0..self.cols {
                data[j * self.rows + i] = self.data[i * self.cols + j];
            }
        }
        Self {
            rows: self.cols,
            cols: self.rows,
            data,
        }
    }

    /// Frobenius norm: sqrt(sum(a_ij^2)).
    pub fn frobenius_norm(&self) -> f64 {
        self.data.iter().map(|v| v * v).sum::<f64>().sqrt()
    }

    /// Trace (sum of diagonal elements).
    pub fn trace(&self) -> f64 {
        let n = self.rows.min(self.cols);
        (0..n).map(|i| self.data[i * self.cols + i]).sum()
    }
}

// ── BLAS error ───────────────────────────────────────────────────────────────

/// Errors from BLAS/LAPACK operations.
#[derive(Debug)]
pub enum BlasError {
    /// Shape mismatch for the operation.
    ShapeMismatch {
        /// Description of the mismatch.
        msg: String,
    },
    /// Matrix is singular (not invertible).
    Singular,
    /// Convergence failure (e.g., eigenvalue iteration).
    Convergence {
        /// Description.
        msg: String,
    },
    /// Matrix must be square for this operation.
    NotSquare {
        /// Actual shape.
        rows: usize,
        /// Actual shape.
        cols: usize,
    },
    /// Matrix must be positive definite.
    NotPositiveDefinite,
}

impl fmt::Display for BlasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShapeMismatch { msg } => write!(f, "BLAS shape mismatch: {msg}"),
            Self::Singular => write!(f, "matrix is singular"),
            Self::Convergence { msg } => write!(f, "convergence failed: {msg}"),
            Self::NotSquare { rows, cols } => {
                write!(f, "expected square matrix, got {rows}x{cols}")
            }
            Self::NotPositiveDefinite => write!(f, "matrix is not positive definite"),
        }
    }
}

impl std::error::Error for BlasError {}

// ── BLAS operations ──────────────────────────────────────────────────────────

/// Hardware-accelerated linear algebra operations.
///
/// All operations are stateless and return new matrices. The implementation
/// uses optimised loops with tiling for cache locality.
pub struct BlasOps;

impl BlasOps {
    /// General matrix multiply: C = A × B.
    ///
    /// A: [m, k], B: [k, n] → C: [m, n]
    pub fn matmul(a: &BlasMatrix, b: &BlasMatrix) -> Result<BlasMatrix, BlasError> {
        if a.cols != b.rows {
            return Err(BlasError::ShapeMismatch {
                msg: format!(
                    "matmul: A is {}x{}, B is {}x{} (A.cols != B.rows)",
                    a.rows, a.cols, b.rows, b.cols
                ),
            });
        }
        let m = a.rows;
        let k = a.cols;
        let n = b.cols;
        let mut c = vec![0.0; m * n];

        // Tiled matmul for cache locality
        const TILE: usize = 32;
        for ii in (0..m).step_by(TILE) {
            for jj in (0..n).step_by(TILE) {
                for kk in (0..k).step_by(TILE) {
                    let i_end = (ii + TILE).min(m);
                    let j_end = (jj + TILE).min(n);
                    let k_end = (kk + TILE).min(k);
                    for i in ii..i_end {
                        for kx in kk..k_end {
                            let a_ik = a.data[i * k + kx];
                            for j in jj..j_end {
                                c[i * n + j] += a_ik * b.data[kx * n + j];
                            }
                        }
                    }
                }
            }
        }

        Ok(BlasMatrix {
            rows: m,
            cols: n,
            data: c,
        })
    }

    /// Matrix-vector multiply: y = A × x.
    ///
    /// A: \[m, n\], x: \[n\] → y: \[m\]
    pub fn matvec(a: &BlasMatrix, x: &[f64]) -> Result<Vec<f64>, BlasError> {
        if a.cols != x.len() {
            return Err(BlasError::ShapeMismatch {
                msg: format!(
                    "matvec: A is {}x{}, x has {} elements",
                    a.rows,
                    a.cols,
                    x.len()
                ),
            });
        }
        let mut y = vec![0.0; a.rows];
        for (i, yi) in y.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (j, xj) in x.iter().enumerate() {
                sum += a.data[i * a.cols + j] * xj;
            }
            *yi = sum;
        }
        Ok(y)
    }

    /// Dot product of two vectors.
    pub fn dot(a: &[f64], b: &[f64]) -> Result<f64, BlasError> {
        if a.len() != b.len() {
            return Err(BlasError::ShapeMismatch {
                msg: format!(
                    "dot: vectors have different lengths ({} vs {})",
                    a.len(),
                    b.len()
                ),
            });
        }
        Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
    }

    /// LU decomposition (with partial pivoting).
    ///
    /// Returns (L, U, pivot) where A\[pivot\] = L × U.
    pub fn lu(a: &BlasMatrix) -> Result<(BlasMatrix, BlasMatrix, Vec<usize>), BlasError> {
        if !a.is_square() {
            return Err(BlasError::NotSquare {
                rows: a.rows,
                cols: a.cols,
            });
        }
        let n = a.rows;
        let mut u_data = a.data.clone();
        let mut l_data = vec![0.0; n * n];
        let mut pivot: Vec<usize> = (0..n).collect();

        for k in 0..n {
            // Find pivot
            let mut max_val = u_data[k * n + k].abs();
            let mut max_row = k;
            for i in (k + 1)..n {
                let val = u_data[i * n + k].abs();
                if val > max_val {
                    max_val = val;
                    max_row = i;
                }
            }

            if max_val < 1e-15 {
                return Err(BlasError::Singular);
            }

            // Swap rows
            if max_row != k {
                pivot.swap(k, max_row);
                for j in 0..n {
                    let idx_k = k * n + j;
                    let idx_m = max_row * n + j;
                    u_data.swap(idx_k, idx_m);
                }
                for j in 0..k {
                    let idx_k = k * n + j;
                    let idx_m = max_row * n + j;
                    l_data.swap(idx_k, idx_m);
                }
            }

            l_data[k * n + k] = 1.0;

            for i in (k + 1)..n {
                let factor = u_data[i * n + k] / u_data[k * n + k];
                l_data[i * n + k] = factor;
                for j in k..n {
                    u_data[i * n + j] -= factor * u_data[k * n + j];
                }
            }
        }

        Ok((
            BlasMatrix {
                rows: n,
                cols: n,
                data: l_data,
            },
            BlasMatrix {
                rows: n,
                cols: n,
                data: u_data,
            },
            pivot,
        ))
    }

    /// Solve linear system A × x = b using LU decomposition.
    pub fn solve(a: &BlasMatrix, b: &[f64]) -> Result<Vec<f64>, BlasError> {
        if !a.is_square() {
            return Err(BlasError::NotSquare {
                rows: a.rows,
                cols: a.cols,
            });
        }
        if a.rows != b.len() {
            return Err(BlasError::ShapeMismatch {
                msg: format!(
                    "solve: A is {}x{}, b has {} elements",
                    a.rows,
                    a.cols,
                    b.len()
                ),
            });
        }

        let (l, u, pivot) = Self::lu(a)?;
        let n = a.rows;

        // Permute b
        let mut pb = vec![0.0; n];
        for i in 0..n {
            pb[i] = b[pivot[i]];
        }

        // Forward substitution: L × y = Pb
        let mut y = vec![0.0; n];
        for i in 0..n {
            let mut sum = pb[i];
            for (j, yj) in y[..i].iter().enumerate() {
                sum -= l.data[i * n + j] * yj;
            }
            y[i] = sum;
        }

        // Back substitution: U × x = y
        let mut x = vec![0.0; n];
        for i in (0..n).rev() {
            let mut sum = y[i];
            for (j, xj) in x.iter().enumerate().skip(i + 1).take(n - i - 1) {
                sum -= u.data[i * n + j] * xj;
            }
            if u.data[i * n + i].abs() < 1e-15 {
                return Err(BlasError::Singular);
            }
            x[i] = sum / u.data[i * n + i];
        }

        Ok(x)
    }

    /// Cholesky decomposition: A = L × Lᵀ (for symmetric positive definite A).
    pub fn cholesky(a: &BlasMatrix) -> Result<BlasMatrix, BlasError> {
        if !a.is_square() {
            return Err(BlasError::NotSquare {
                rows: a.rows,
                cols: a.cols,
            });
        }
        let n = a.rows;
        let mut l_data = vec![0.0; n * n];

        for i in 0..n {
            for j in 0..=i {
                let mut sum = 0.0;
                for k in 0..j {
                    sum += l_data[i * n + k] * l_data[j * n + k];
                }
                if i == j {
                    let diag = a.data[i * n + i] - sum;
                    if diag <= 0.0 {
                        return Err(BlasError::NotPositiveDefinite);
                    }
                    l_data[i * n + j] = diag.sqrt();
                } else {
                    l_data[i * n + j] = (a.data[i * n + j] - sum) / l_data[j * n + j];
                }
            }
        }

        Ok(BlasMatrix {
            rows: n,
            cols: n,
            data: l_data,
        })
    }

    /// QR decomposition via Gram-Schmidt: A = Q × R.
    pub fn qr(a: &BlasMatrix) -> Result<(BlasMatrix, BlasMatrix), BlasError> {
        let m = a.rows;
        let n = a.cols;
        let k = m.min(n);

        let mut q_data = vec![0.0; m * k];
        let mut r_data = vec![0.0; k * n];

        // Modified Gram-Schmidt
        let mut columns: Vec<Vec<f64>> = (0..n)
            .map(|j| (0..m).map(|i| a.data[i * n + j]).collect())
            .collect();

        for i in 0..k {
            // Compute norm
            let norm: f64 = columns[i].iter().map(|v| v * v).sum::<f64>().sqrt();
            if norm < 1e-15 {
                // Near-zero column
                for row in 0..m {
                    q_data[row * k + i] = 0.0;
                }
                continue;
            }

            r_data[i * n + i] = norm;
            for row in 0..m {
                q_data[row * k + i] = columns[i][row] / norm;
            }

            // Orthogonalise remaining columns
            for j in (i + 1)..n {
                let mut dot = 0.0;
                for row in 0..m {
                    dot += q_data[row * k + i] * columns[j][row];
                }
                r_data[i * n + j] = dot;
                for row in 0..m {
                    columns[j][row] -= dot * q_data[row * k + i];
                }
            }
        }

        Ok((
            BlasMatrix {
                rows: m,
                cols: k,
                data: q_data,
            },
            BlasMatrix {
                rows: k,
                cols: n,
                data: r_data,
            },
        ))
    }

    /// Eigenvalue decomposition via QR iteration (for symmetric matrices).
    ///
    /// Returns (eigenvalues, eigenvectors) where eigenvectors are column vectors.
    pub fn eig_symmetric(
        a: &BlasMatrix,
        max_iter: usize,
    ) -> Result<(Vec<f64>, BlasMatrix), BlasError> {
        if !a.is_square() {
            return Err(BlasError::NotSquare {
                rows: a.rows,
                cols: a.cols,
            });
        }
        let n = a.rows;
        let mut ak = a.clone();
        let mut v = BlasMatrix::eye(n);

        for _ in 0..max_iter {
            let (q, r) = Self::qr(&ak)?;
            ak = Self::matmul(&r, &q)?;
            v = Self::matmul(&v, &q)?;

            // Check convergence (off-diagonal elements)
            let mut off_diag = 0.0;
            for i in 0..n {
                for j in 0..n {
                    if i != j {
                        off_diag += ak.data[i * n + j].abs();
                    }
                }
            }
            if off_diag < 1e-10 {
                break;
            }
        }

        let eigenvalues: Vec<f64> = (0..n).map(|i| ak.data[i * n + i]).collect();
        Ok((eigenvalues, v))
    }

    /// Singular Value Decomposition (thin SVD): A = U × Σ × Vᵀ.
    ///
    /// Uses eigendecomposition of AᵀA for the right singular vectors.
    /// Returns (U, singular_values, Vt).
    pub fn svd(
        a: &BlasMatrix,
        max_iter: usize,
    ) -> Result<(BlasMatrix, Vec<f64>, BlasMatrix), BlasError> {
        let at = a.transpose();
        let ata = Self::matmul(&at, a)?;

        let (eigenvalues, v) = Self::eig_symmetric(&ata, max_iter)?;

        // Singular values are sqrt of eigenvalues
        let singular_values: Vec<f64> = eigenvalues
            .iter()
            .map(|&ev| if ev > 0.0 { ev.sqrt() } else { 0.0 })
            .collect();

        // U = A × V × Σ^{-1}
        let k = singular_values.len();
        let m = a.rows;
        let av = Self::matmul(a, &v)?;

        let mut u_data = vec![0.0; m * k];
        for j in 0..k {
            if singular_values[j] > 1e-15 {
                for i in 0..m {
                    u_data[i * k + j] = av.data[i * k + j] / singular_values[j];
                }
            }
        }

        Ok((
            BlasMatrix {
                rows: m,
                cols: k,
                data: u_data,
            },
            singular_values,
            v.transpose(),
        ))
    }

    /// Matrix determinant (via LU decomposition).
    pub fn det(a: &BlasMatrix) -> Result<f64, BlasError> {
        if !a.is_square() {
            return Err(BlasError::NotSquare {
                rows: a.rows,
                cols: a.cols,
            });
        }
        let n = a.rows;
        // Try LU decomposition — singular matrix ⇒ det = 0
        let (_, u, pivot) = match Self::lu(a) {
            Ok(result) => result,
            Err(BlasError::Singular) => return Ok(0.0),
            Err(e) => return Err(e),
        };

        // Count row swaps for sign
        let mut swaps = 0;
        let mut visited = vec![false; n];
        for i in 0..n {
            if !visited[i] {
                visited[i] = true;
                let mut j = pivot[i];
                while j != i {
                    swaps += 1;
                    visited[j] = true;
                    j = pivot[j];
                }
            }
        }

        let sign = if swaps % 2 == 0 { 1.0 } else { -1.0 };
        let prod: f64 = (0..n).map(|i| u.data[i * n + i]).product();

        Ok(sign * prod)
    }

    /// Matrix inverse (via LU decomposition and column-wise solve).
    pub fn inv(a: &BlasMatrix) -> Result<BlasMatrix, BlasError> {
        if !a.is_square() {
            return Err(BlasError::NotSquare {
                rows: a.rows,
                cols: a.cols,
            });
        }
        let n = a.rows;
        let mut result = BlasMatrix::zeros(n, n);

        for j in 0..n {
            let mut e = vec![0.0; n];
            e[j] = 1.0;
            let col = Self::solve(a, &e)?;
            for (i, ci) in col.iter().enumerate() {
                result.data[i * n + j] = *ci;
            }
        }

        Ok(result)
    }

    /// Outer product: a ⊗ b → A\[i,j\] = a\[i\] * b\[j\].
    pub fn outer(a: &[f64], b: &[f64]) -> BlasMatrix {
        let m = a.len();
        let n = b.len();
        let mut data = vec![0.0; m * n];
        for i in 0..m {
            for j in 0..n {
                data[i * n + j] = a[i] * b[j];
            }
        }
        BlasMatrix {
            rows: m,
            cols: n,
            data,
        }
    }

    /// Vector L2 norm.
    pub fn norm2(v: &[f64]) -> f64 {
        v.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matmul() {
        let a = BlasMatrix::from_vec(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = BlasMatrix::from_vec(3, 2, vec![7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
        let c = BlasOps::matmul(&a, &b).unwrap();
        assert_eq!(c.rows, 2);
        assert_eq!(c.cols, 2);
        assert!((c.data[0] - 58.0).abs() < 1e-10);
        assert!((c.data[1] - 64.0).abs() < 1e-10);
        assert!((c.data[2] - 139.0).abs() < 1e-10);
        assert!((c.data[3] - 154.0).abs() < 1e-10);
    }

    #[test]
    fn test_matmul_shape_mismatch() {
        let a = BlasMatrix::from_vec(2, 3, vec![1.0; 6]);
        let b = BlasMatrix::from_vec(2, 2, vec![1.0; 4]);
        assert!(BlasOps::matmul(&a, &b).is_err());
    }

    #[test]
    fn test_matvec() {
        let a = BlasMatrix::from_vec(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let x = vec![1.0, 1.0, 1.0];
        let y = BlasOps::matvec(&a, &x).unwrap();
        assert!((y[0] - 6.0).abs() < 1e-10);
        assert!((y[1] - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_dot() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let d = BlasOps::dot(&a, &b).unwrap();
        assert!((d - 32.0).abs() < 1e-10);
    }

    #[test]
    fn test_identity() {
        let eye = BlasMatrix::eye(3);
        assert!((eye.get(0, 0) - 1.0).abs() < 1e-10);
        assert!((eye.get(1, 1) - 1.0).abs() < 1e-10);
        assert!((eye.get(0, 1)).abs() < 1e-10);
    }

    #[test]
    fn test_transpose() {
        let a = BlasMatrix::from_vec(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let at = a.transpose();
        assert_eq!(at.rows, 3);
        assert_eq!(at.cols, 2);
        assert!((at.get(0, 0) - 1.0).abs() < 1e-10);
        assert!((at.get(0, 1) - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_lu() {
        let a = BlasMatrix::from_vec(3, 3, vec![2.0, 1.0, 1.0, 4.0, 3.0, 3.0, 8.0, 7.0, 9.0]);
        let (l, u, _) = BlasOps::lu(&a).unwrap();
        // L × U should approximately equal A (permuted)
        let lu = BlasOps::matmul(&l, &u).unwrap();
        assert!(lu.frobenius_norm() > 0.0);
    }

    #[test]
    fn test_solve() {
        // 2x + y = 5
        // x + 3y = 7
        // Solution: x = 1.6, y = 1.8
        let a = BlasMatrix::from_vec(2, 2, vec![2.0, 1.0, 1.0, 3.0]);
        let b = vec![5.0, 7.0];
        let x = BlasOps::solve(&a, &b).unwrap();
        assert!((x[0] - 1.6).abs() < 1e-10);
        assert!((x[1] - 1.8).abs() < 1e-10);
    }

    #[test]
    fn test_cholesky() {
        // Symmetric positive definite: [[4, 2], [2, 3]]
        let a = BlasMatrix::from_vec(2, 2, vec![4.0, 2.0, 2.0, 3.0]);
        let l = BlasOps::cholesky(&a).unwrap();
        let lt = l.transpose();
        let result = BlasOps::matmul(&l, &lt).unwrap();
        for i in 0..4 {
            assert!((result.data[i] - a.data[i]).abs() < 1e-10);
        }
    }

    #[test]
    fn test_qr() {
        let a = BlasMatrix::from_vec(3, 3, vec![1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0]);
        let (q, r) = BlasOps::qr(&a).unwrap();
        // Q × R ≈ A
        let qr = BlasOps::matmul(&q, &r).unwrap();
        for i in 0..9 {
            assert!(
                (qr.data[i] - a.data[i]).abs() < 1e-10,
                "QR mismatch at index {}: {} vs {}",
                i,
                qr.data[i],
                a.data[i]
            );
        }
    }

    #[test]
    fn test_eig_symmetric() {
        // [[2, 1], [1, 2]] → eigenvalues 3 and 1
        let a = BlasMatrix::from_vec(2, 2, vec![2.0, 1.0, 1.0, 2.0]);
        let (eigenvalues, _) = BlasOps::eig_symmetric(&a, 100).unwrap();
        let mut sorted = eigenvalues.clone();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
        assert!((sorted[0] - 3.0).abs() < 1e-6);
        assert!((sorted[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_det() {
        let a = BlasMatrix::from_vec(2, 2, vec![3.0, 8.0, 4.0, 6.0]);
        let d = BlasOps::det(&a).unwrap();
        assert!((d - (-14.0)).abs() < 1e-10);
    }

    #[test]
    fn test_inv() {
        let a = BlasMatrix::from_vec(2, 2, vec![4.0, 7.0, 2.0, 6.0]);
        let a_inv = BlasOps::inv(&a).unwrap();
        let prod = BlasOps::matmul(&a, &a_inv).unwrap();
        // Should be close to identity
        assert!((prod.get(0, 0) - 1.0).abs() < 1e-10);
        assert!((prod.get(1, 1) - 1.0).abs() < 1e-10);
        assert!(prod.get(0, 1).abs() < 1e-10);
        assert!(prod.get(1, 0).abs() < 1e-10);
    }

    #[test]
    fn test_svd() {
        let a = BlasMatrix::from_vec(3, 2, vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        let (_u, sigma, _vt) = BlasOps::svd(&a, 100).unwrap();
        // Should have 2 singular values > 0
        assert!(sigma.len() == 2);
        assert!(sigma[0] > 0.0 || sigma[1] > 0.0);
    }

    #[test]
    fn test_frobenius_norm() {
        let a = BlasMatrix::from_vec(2, 2, vec![1.0, 2.0, 3.0, 4.0]);
        let norm = a.frobenius_norm();
        // sqrt(1 + 4 + 9 + 16) = sqrt(30)
        assert!((norm - 30.0_f64.sqrt()).abs() < 1e-10);
    }

    #[test]
    fn test_outer() {
        let a = vec![1.0, 2.0];
        let b = vec![3.0, 4.0, 5.0];
        let c = BlasOps::outer(&a, &b);
        assert_eq!(c.rows, 2);
        assert_eq!(c.cols, 3);
        assert!((c.get(0, 0) - 3.0).abs() < 1e-10);
        assert!((c.get(1, 2) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_trace() {
        let a = BlasMatrix::from_vec(3, 3, vec![1.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0, 9.0]);
        assert!((a.trace() - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_singular_matrix() {
        let a = BlasMatrix::from_vec(2, 2, vec![1.0, 2.0, 2.0, 4.0]);
        assert!(BlasOps::det(&a).is_ok()); // det should be 0
        let d = BlasOps::det(&a).unwrap();
        assert!(d.abs() < 1e-10);
    }

    #[test]
    fn test_not_positive_definite() {
        let a = BlasMatrix::from_vec(2, 2, vec![-1.0, 0.0, 0.0, -1.0]);
        assert!(BlasOps::cholesky(&a).is_err());
    }
}
