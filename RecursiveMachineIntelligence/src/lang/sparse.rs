//! Sparse tensor support — CSR and COO formats for RMIL.
//!
//! Provides two standard sparse representations:
//!
//! - **COO** (Coordinate): list of (row, col, value) triples
//! - **CSR** (Compressed Sparse Row): row pointer + column indices + values
//!
//! Both formats support:
//! - Conversion to/from dense `TensorData`
//! - SpMV (sparse matrix–dense vector multiplication)
//! - Element-wise addition
//! - Transpose
//! - Density / sparsity statistics
//!
//! # Examples
//!
//! ```
//! use rmi::lang::sparse::{CooMatrix, CsrMatrix};
//!
//! // Build a 3×3 identity matrix in COO
//! let mut coo = CooMatrix::new(3, 3);
//! coo.push(0, 0, 1.0);
//! coo.push(1, 1, 1.0);
//! coo.push(2, 2, 1.0);
//! assert_eq!(coo.nnz(), 3);
//!
//! // Convert to CSR for fast row access
//! let csr = coo.to_csr();
//! assert_eq!(csr.nnz(), 3);
//!
//! // SpMV: identity * [1,2,3] = [1,2,3]
//! let y = csr.spmv(&[1.0, 2.0, 3.0]);
//! assert_eq!(y, vec![1.0, 2.0, 3.0]);
//! ```

// ── COO (Coordinate) format ─────────────────────────────────────────────────

/// Coordinate-format sparse matrix (list of triplets).
///
/// Good for incremental construction; convert to CSR for computation.
#[derive(Debug, Clone)]
pub struct CooMatrix {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Row indices.
    pub row_indices: Vec<usize>,
    /// Column indices.
    pub col_indices: Vec<usize>,
    /// Non-zero values.
    pub values: Vec<f32>,
}

impl CooMatrix {
    /// Create a new empty COO matrix.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            row_indices: Vec::new(),
            col_indices: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Create a COO matrix with pre-allocated capacity.
    pub fn with_capacity(rows: usize, cols: usize, capacity: usize) -> Self {
        Self {
            rows,
            cols,
            row_indices: Vec::with_capacity(capacity),
            col_indices: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }

    /// Add a non-zero entry.
    pub fn push(&mut self, row: usize, col: usize, value: f32) {
        debug_assert!(row < self.rows, "row {row} out of bounds ({})", self.rows);
        debug_assert!(col < self.cols, "col {col} out of bounds ({})", self.cols);
        self.row_indices.push(row);
        self.col_indices.push(col);
        self.values.push(value);
    }

    /// Number of stored non-zero entries.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Density (fraction of non-zero elements).
    pub fn density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 {
            return 0.0;
        }
        self.nnz() as f64 / total as f64
    }

    /// Sparsity (fraction of zero elements).
    pub fn sparsity(&self) -> f64 {
        1.0 - self.density()
    }

    /// Convert to dense row-major f32 vector.
    pub fn to_dense(&self) -> Vec<f32> {
        let mut dense = vec![0.0_f32; self.rows * self.cols];
        for i in 0..self.nnz() {
            let idx = self.row_indices[i] * self.cols + self.col_indices[i];
            dense[idx] += self.values[i];
        }
        dense
    }

    /// Create from a dense row-major f32 slice.
    pub fn from_dense(data: &[f32], rows: usize, cols: usize) -> Self {
        let mut coo = Self::new(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                let v = data[r * cols + c];
                if v != 0.0 {
                    coo.push(r, c, v);
                }
            }
        }
        coo
    }

    /// Convert to CSR format.
    pub fn to_csr(&self) -> CsrMatrix {
        // Sort triplets by row, then column
        let mut indices: Vec<usize> = (0..self.nnz()).collect();
        indices.sort_by(|&a, &b| {
            self.row_indices[a]
                .cmp(&self.row_indices[b])
                .then(self.col_indices[a].cmp(&self.col_indices[b]))
        });

        let mut row_ptr = vec![0usize; self.rows + 1];
        let mut col_indices = Vec::with_capacity(self.nnz());
        let mut values = Vec::with_capacity(self.nnz());

        for &i in &indices {
            col_indices.push(self.col_indices[i]);
            values.push(self.values[i]);
            row_ptr[self.row_indices[i] + 1] += 1;
        }

        // Cumulative sum
        for r in 0..self.rows {
            row_ptr[r + 1] += row_ptr[r];
        }

        CsrMatrix {
            rows: self.rows,
            cols: self.cols,
            row_ptr,
            col_indices,
            values,
        }
    }

    /// Transpose (swap rows and columns).
    pub fn transpose(&self) -> Self {
        Self {
            rows: self.cols,
            cols: self.rows,
            row_indices: self.col_indices.clone(),
            col_indices: self.row_indices.clone(),
            values: self.values.clone(),
        }
    }

    /// Element-wise add two COO matrices (same shape).
    pub fn add(&self, other: &CooMatrix) -> CooMatrix {
        assert_eq!(self.rows, other.rows);
        assert_eq!(self.cols, other.cols);
        let mut result = self.clone();
        result.row_indices.extend_from_slice(&other.row_indices);
        result.col_indices.extend_from_slice(&other.col_indices);
        result.values.extend_from_slice(&other.values);
        result
    }

    /// Scale all values by a constant.
    pub fn scale(&mut self, factor: f32) {
        for v in &mut self.values {
            *v *= factor;
        }
    }
}

// ── CSR (Compressed Sparse Row) format ───────────────────────────────────────

/// Compressed Sparse Row matrix.
///
/// Efficient for row-based access and SpMV.
#[derive(Debug, Clone)]
pub struct CsrMatrix {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Row pointer array (length = rows + 1).
    pub row_ptr: Vec<usize>,
    /// Column indices for non-zeros.
    pub col_indices: Vec<usize>,
    /// Non-zero values.
    pub values: Vec<f32>,
}

impl CsrMatrix {
    /// Create an empty CSR matrix.
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            row_ptr: vec![0; rows + 1],
            col_indices: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Number of stored non-zero entries.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Density.
    pub fn density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 {
            return 0.0;
        }
        self.nnz() as f64 / total as f64
    }

    /// Sparsity.
    pub fn sparsity(&self) -> f64 {
        1.0 - self.density()
    }

    /// Number of non-zeros in a specific row.
    pub fn row_nnz(&self, row: usize) -> usize {
        self.row_ptr[row + 1] - self.row_ptr[row]
    }

    /// Get the column indices and values for a row.
    pub fn row_entries(&self, row: usize) -> (&[usize], &[f32]) {
        let start = self.row_ptr[row];
        let end = self.row_ptr[row + 1];
        (&self.col_indices[start..end], &self.values[start..end])
    }

    /// Convert to dense row-major vector.
    pub fn to_dense(&self) -> Vec<f32> {
        let mut dense = vec![0.0_f32; self.rows * self.cols];
        for r in 0..self.rows {
            let (cols, vals) = self.row_entries(r);
            for (&c, &v) in cols.iter().zip(vals.iter()) {
                dense[r * self.cols + c] = v;
            }
        }
        dense
    }

    /// Create from dense data.
    pub fn from_dense(data: &[f32], rows: usize, cols: usize) -> Self {
        CooMatrix::from_dense(data, rows, cols).to_csr()
    }

    /// Sparse matrix × dense vector multiplication.
    ///
    /// `y = A * x` where A is this matrix.
    pub fn spmv(&self, x: &[f32]) -> Vec<f32> {
        assert_eq!(x.len(), self.cols, "vector length must match columns");
        let mut y = vec![0.0_f32; self.rows];
        for (r, y_r) in y.iter_mut().enumerate() {
            let (cols, vals) = self.row_entries(r);
            let mut sum = 0.0_f32;
            for (&c, &v) in cols.iter().zip(vals.iter()) {
                sum += v * x[c];
            }
            *y_r = sum;
        }
        y
    }

    /// Sparse matrix × dense matrix multiplication (A * B).
    ///
    /// B is stored row-major with shape (self.cols × b_cols).
    pub fn spmm(&self, b: &[f32], b_cols: usize) -> Vec<f32> {
        assert_eq!(
            b.len(),
            self.cols * b_cols,
            "B must have shape (cols × b_cols)"
        );
        let mut c = vec![0.0_f32; self.rows * b_cols];
        for r in 0..self.rows {
            let (a_cols, a_vals) = self.row_entries(r);
            for (&ac, &av) in a_cols.iter().zip(a_vals.iter()) {
                for j in 0..b_cols {
                    c[r * b_cols + j] += av * b[ac * b_cols + j];
                }
            }
        }
        c
    }

    /// Convert to COO format.
    pub fn to_coo(&self) -> CooMatrix {
        let mut coo = CooMatrix::with_capacity(self.rows, self.cols, self.nnz());
        for r in 0..self.rows {
            let (cols, vals) = self.row_entries(r);
            for (&c, &v) in cols.iter().zip(vals.iter()) {
                coo.push(r, c, v);
            }
        }
        coo
    }

    /// Transpose to CSR (by converting through COO).
    pub fn transpose(&self) -> CsrMatrix {
        self.to_coo().transpose().to_csr()
    }

    /// Memory size in bytes (approximate).
    pub fn memory_bytes(&self) -> usize {
        self.row_ptr.len() * size_of::<usize>()
            + self.col_indices.len() * size_of::<usize>()
            + self.values.len() * size_of::<f32>()
    }

    /// Memory savings vs dense storage.
    pub fn memory_ratio(&self) -> f64 {
        let dense_bytes = self.rows * self.cols * size_of::<f32>();
        if dense_bytes == 0 {
            return 1.0;
        }
        self.memory_bytes() as f64 / dense_bytes as f64
    }
}

use std::mem::size_of;

// ── Identity / diagonal constructors ─────────────────────────────────────────

/// Create a sparse identity matrix of size n×n.
pub fn sparse_identity(n: usize) -> CsrMatrix {
    let mut row_ptr = Vec::with_capacity(n + 1);
    let mut col_indices = Vec::with_capacity(n);
    let mut values = Vec::with_capacity(n);

    for i in 0..n {
        row_ptr.push(i);
        col_indices.push(i);
        values.push(1.0);
    }
    row_ptr.push(n);

    CsrMatrix {
        rows: n,
        cols: n,
        row_ptr,
        col_indices,
        values,
    }
}

/// Create a sparse diagonal matrix from a vector.
pub fn sparse_diagonal(diag: &[f32]) -> CsrMatrix {
    let n = diag.len();
    let mut row_ptr = Vec::with_capacity(n + 1);
    let mut col_indices = Vec::new();
    let mut values = Vec::new();

    let mut ptr = 0;
    for (i, &v) in diag.iter().enumerate() {
        row_ptr.push(ptr);
        if v != 0.0 {
            col_indices.push(i);
            values.push(v);
            ptr += 1;
        }
    }
    row_ptr.push(ptr);

    CsrMatrix {
        rows: n,
        cols: n,
        row_ptr,
        col_indices,
        values,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── COO ──────────────────────────────────────────────────────────────

    #[test]
    fn coo_empty() {
        let coo = CooMatrix::new(3, 3);
        assert_eq!(coo.nnz(), 0);
        assert_eq!(coo.density(), 0.0);
        assert_eq!(coo.sparsity(), 1.0);
    }

    #[test]
    fn coo_push_and_nnz() {
        let mut coo = CooMatrix::new(3, 3);
        coo.push(0, 0, 1.0);
        coo.push(1, 2, 2.0);
        assert_eq!(coo.nnz(), 2);
    }

    #[test]
    fn coo_to_dense() {
        let mut coo = CooMatrix::new(2, 3);
        coo.push(0, 0, 1.0);
        coo.push(0, 2, 3.0);
        coo.push(1, 1, 2.0);
        let dense = coo.to_dense();
        assert_eq!(dense, vec![1.0, 0.0, 3.0, 0.0, 2.0, 0.0]);
    }

    #[test]
    fn coo_from_dense() {
        let data = vec![1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0];
        let coo = CooMatrix::from_dense(&data, 3, 3);
        assert_eq!(coo.nnz(), 4); // 1.0, 2.0, 3.0, 4.0
        let restored = coo.to_dense();
        assert_eq!(restored, data);
    }

    #[test]
    fn coo_density() {
        let mut coo = CooMatrix::new(4, 4);
        coo.push(0, 0, 1.0);
        coo.push(1, 1, 1.0);
        assert!((coo.density() - 2.0 / 16.0).abs() < 1e-10);
    }

    #[test]
    fn coo_transpose() {
        let mut coo = CooMatrix::new(2, 3);
        coo.push(0, 1, 5.0);
        coo.push(1, 2, 7.0);
        let t = coo.transpose();
        assert_eq!(t.rows, 3);
        assert_eq!(t.cols, 2);
        assert_eq!(t.row_indices, vec![1, 2]);
        assert_eq!(t.col_indices, vec![0, 1]);
    }

    #[test]
    fn coo_add() {
        let mut a = CooMatrix::new(2, 2);
        a.push(0, 0, 1.0);
        let mut b = CooMatrix::new(2, 2);
        b.push(1, 1, 2.0);
        let c = a.add(&b);
        assert_eq!(c.nnz(), 2);
        let dense = c.to_dense();
        assert_eq!(dense, vec![1.0, 0.0, 0.0, 2.0]);
    }

    #[test]
    fn coo_scale() {
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 0, 2.0);
        coo.push(1, 1, 3.0);
        coo.scale(0.5);
        assert_eq!(coo.values, vec![1.0, 1.5]);
    }

    // ── CSR ──────────────────────────────────────────────────────────────

    #[test]
    fn csr_from_coo() {
        let mut coo = CooMatrix::new(3, 3);
        coo.push(0, 0, 1.0);
        coo.push(1, 1, 2.0);
        coo.push(2, 2, 3.0);
        let csr = coo.to_csr();
        assert_eq!(csr.nnz(), 3);
        assert_eq!(csr.row_ptr, vec![0, 1, 2, 3]);
    }

    #[test]
    fn csr_row_entries() {
        let mut coo = CooMatrix::new(2, 3);
        coo.push(0, 0, 1.0);
        coo.push(0, 2, 3.0);
        coo.push(1, 1, 2.0);
        let csr = coo.to_csr();

        let (cols, vals) = csr.row_entries(0);
        assert_eq!(cols, &[0, 2]);
        assert_eq!(vals, &[1.0, 3.0]);

        let (cols, vals) = csr.row_entries(1);
        assert_eq!(cols, &[1]);
        assert_eq!(vals, &[2.0]);
    }

    #[test]
    fn csr_to_dense() {
        let mut coo = CooMatrix::new(2, 2);
        coo.push(0, 0, 1.0);
        coo.push(0, 1, 2.0);
        coo.push(1, 0, 3.0);
        coo.push(1, 1, 4.0);
        let csr = coo.to_csr();
        assert_eq!(csr.to_dense(), vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn csr_from_dense() {
        let data = vec![1.0, 0.0, 0.0, 2.0];
        let csr = CsrMatrix::from_dense(&data, 2, 2);
        assert_eq!(csr.nnz(), 2);
        assert_eq!(csr.to_dense(), data);
    }

    // ── SpMV ─────────────────────────────────────────────────────────────

    #[test]
    fn spmv_identity() {
        let id = sparse_identity(3);
        let x = vec![1.0, 2.0, 3.0];
        let y = id.spmv(&x);
        assert_eq!(y, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn spmv_general() {
        // [[1, 2], [3, 4]] * [1, 1] = [3, 7]
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let csr = CsrMatrix::from_dense(&data, 2, 2);
        let y = csr.spmv(&[1.0, 1.0]);
        assert_eq!(y, vec![3.0, 7.0]);
    }

    #[test]
    fn spmv_sparse() {
        // [[1, 0], [0, 2]] * [3, 4] = [3, 8]
        let data = vec![1.0, 0.0, 0.0, 2.0];
        let csr = CsrMatrix::from_dense(&data, 2, 2);
        let y = csr.spmv(&[3.0, 4.0]);
        assert_eq!(y, vec![3.0, 8.0]);
    }

    // ── SpMM ─────────────────────────────────────────────────────────────

    #[test]
    fn spmm_identity() {
        let id = sparse_identity(2);
        let b = vec![1.0, 2.0, 3.0, 4.0]; // 2×2
        let c = id.spmm(&b, 2);
        assert_eq!(c, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn spmm_general() {
        // [[1, 0], [0, 2]] * [[1, 2], [3, 4]] = [[1, 2], [6, 8]]
        let a = CsrMatrix::from_dense(&[1.0, 0.0, 0.0, 2.0], 2, 2);
        let b = vec![1.0, 2.0, 3.0, 4.0];
        let c = a.spmm(&b, 2);
        assert_eq!(c, vec![1.0, 2.0, 6.0, 8.0]);
    }

    // ── Transpose ────────────────────────────────────────────────────────

    #[test]
    fn csr_transpose() {
        let data = vec![1.0, 2.0, 0.0, 3.0];
        let csr = CsrMatrix::from_dense(&data, 2, 2);
        let t = csr.transpose();
        assert_eq!(t.to_dense(), vec![1.0, 0.0, 2.0, 3.0]);
    }

    // ── Identity / diagonal constructors ─────────────────────────────────

    #[test]
    fn sparse_identity_test() {
        let id = sparse_identity(4);
        assert_eq!(id.nnz(), 4);
        assert_eq!(id.rows, 4);
        assert_eq!(id.cols, 4);
        let dense = id.to_dense();
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert_eq!(dense[i * 4 + j], expected);
            }
        }
    }

    #[test]
    fn sparse_diagonal_test() {
        let d = sparse_diagonal(&[2.0, 0.0, 3.0]);
        assert_eq!(d.nnz(), 2); // zero is skipped
        assert_eq!(
            d.to_dense(),
            vec![2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0]
        );
    }

    // ── Memory stats ─────────────────────────────────────────────────────

    #[test]
    fn memory_ratio() {
        // 100×100 identity: 100 nnz vs 10000 dense elements
        let id = sparse_identity(100);
        assert!(id.memory_ratio() < 0.5); // should be much less than dense
    }

    #[test]
    fn memory_bytes() {
        let id = sparse_identity(3);
        assert!(id.memory_bytes() > 0);
    }

    // ── Round-trip COO ↔ CSR ─────────────────────────────────────────────

    #[test]
    fn coo_csr_roundtrip() {
        let mut coo = CooMatrix::new(3, 3);
        coo.push(0, 1, 5.0);
        coo.push(2, 0, 7.0);
        coo.push(1, 2, 3.0);

        let csr = coo.to_csr();
        let coo2 = csr.to_coo();
        assert_eq!(coo2.nnz(), 3);

        // Dense representations should match
        assert_eq!(coo.to_dense(), coo2.to_dense());
    }

    #[test]
    fn csr_empty() {
        let csr = CsrMatrix::new(3, 4);
        assert_eq!(csr.nnz(), 0);
        assert_eq!(csr.rows, 3);
        assert_eq!(csr.cols, 4);
        assert_eq!(csr.density(), 0.0);
    }

    #[test]
    fn row_nnz() {
        let mut coo = CooMatrix::new(3, 3);
        coo.push(0, 0, 1.0);
        coo.push(0, 1, 2.0);
        coo.push(2, 2, 3.0);
        let csr = coo.to_csr();
        assert_eq!(csr.row_nnz(0), 2);
        assert_eq!(csr.row_nnz(1), 0);
        assert_eq!(csr.row_nnz(2), 1);
    }

    #[test]
    fn coo_with_capacity() {
        let coo = CooMatrix::with_capacity(10, 10, 50);
        assert_eq!(coo.nnz(), 0);
        assert_eq!(coo.rows, 10);
    }
}
