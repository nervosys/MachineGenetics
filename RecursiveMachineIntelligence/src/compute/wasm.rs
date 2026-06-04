//! WASM-compatible compute backend.
//!
//! Provides a `wasm32`-friendly subset of the compute API using pure Rust
//! (no `ndarray` threading, no `tokio`, no file I/O). When compiled to
//! `wasm32-unknown-unknown` with `--features wasm`, this module provides
//! JavaScript interop via `wasm-bindgen`.
//!
//! # Feature gate
//!
//! ```toml
//! [dependencies]
//! rmi = { version = "1", features = ["wasm"] }
//! ```
//!
//! # Example (JavaScript)
//!
//! ```js
//! import init, { WasmTensor } from './rmi_wasm.js';
//! await init();
//! const t = WasmTensor.zeros(new Uint32Array([3, 4]));
//! console.log(t.shape());
//! ```

use wasm_bindgen::prelude::*;

/// A tensor stored in linear WASM memory.
///
/// This is a lightweight tensor type designed for use from JavaScript.
/// All data lives in the WASM linear memory and is accessible from JS.
#[wasm_bindgen]
pub struct WasmTensor {
    data: Vec<f32>,
    shape: Vec<usize>,
}

#[wasm_bindgen]
impl WasmTensor {
    /// Create a tensor filled with zeros.
    #[wasm_bindgen]
    pub fn zeros(shape: &[u32]) -> WasmTensor {
        let shape: Vec<usize> = shape.iter().map(|&s| s as usize).collect();
        let numel: usize = shape.iter().product();
        WasmTensor {
            data: vec![0.0; numel],
            shape,
        }
    }

    /// Create a tensor filled with ones.
    #[wasm_bindgen]
    pub fn ones(shape: &[u32]) -> WasmTensor {
        let shape: Vec<usize> = shape.iter().map(|&s| s as usize).collect();
        let numel: usize = shape.iter().product();
        WasmTensor {
            data: vec![1.0; numel],
            shape,
        }
    }

    /// Create a tensor from f32 data.
    #[wasm_bindgen(js_name = "fromData")]
    pub fn from_data(data: &[f32], shape: &[u32]) -> WasmTensor {
        let shape: Vec<usize> = shape.iter().map(|&s| s as usize).collect();
        WasmTensor {
            data: data.to_vec(),
            shape,
        }
    }

    /// Get the shape as a JS-friendly array.
    #[wasm_bindgen]
    pub fn shape(&self) -> Vec<u32> {
        self.shape.iter().map(|&s| s as u32).collect()
    }

    /// Get number of elements.
    #[wasm_bindgen]
    pub fn numel(&self) -> u32 {
        self.shape.iter().product::<usize>() as u32
    }

    /// Get the data as a JS-friendly float array.
    #[wasm_bindgen]
    pub fn data(&self) -> Vec<f32> {
        self.data.clone()
    }

    /// Element-wise addition.
    #[wasm_bindgen]
    pub fn add(&self, other: &WasmTensor) -> WasmTensor {
        let data: Vec<f32> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a + b)
            .collect();
        WasmTensor {
            data,
            shape: self.shape.clone(),
        }
    }

    /// Element-wise multiplication.
    #[wasm_bindgen]
    pub fn mul(&self, other: &WasmTensor) -> WasmTensor {
        let data: Vec<f32> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a * b)
            .collect();
        WasmTensor {
            data,
            shape: self.shape.clone(),
        }
    }

    /// Scalar multiplication.
    #[wasm_bindgen]
    pub fn scale(&self, scalar: f32) -> WasmTensor {
        let data: Vec<f32> = self.data.iter().map(|x| x * scalar).collect();
        WasmTensor {
            data,
            shape: self.shape.clone(),
        }
    }

    /// ReLU activation.
    #[wasm_bindgen]
    pub fn relu(&self) -> WasmTensor {
        let data: Vec<f32> = self.data.iter().map(|x| x.max(0.0)).collect();
        WasmTensor {
            data,
            shape: self.shape.clone(),
        }
    }

    /// Sigmoid activation.
    #[wasm_bindgen]
    pub fn sigmoid(&self) -> WasmTensor {
        let data: Vec<f32> = self.data.iter().map(|x| 1.0 / (1.0 + (-x).exp())).collect();
        WasmTensor {
            data,
            shape: self.shape.clone(),
        }
    }

    /// Sum all elements.
    #[wasm_bindgen]
    pub fn sum(&self) -> f32 {
        self.data.iter().sum()
    }

    /// Mean of all elements.
    #[wasm_bindgen]
    pub fn mean(&self) -> f32 {
        self.sum() / self.data.len() as f32
    }

    /// Matrix multiplication (2D only).
    #[wasm_bindgen]
    pub fn matmul(&self, other: &WasmTensor) -> WasmTensor {
        assert!(self.shape.len() == 2 && other.shape.len() == 2);
        let m = self.shape[0];
        let k = self.shape[1];
        assert_eq!(k, other.shape[0]);
        let n = other.shape[1];

        let mut out = vec![0.0f32; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut s = 0.0f32;
                for p in 0..k {
                    s += self.data[i * k + p] * other.data[p * n + j];
                }
                out[i * n + j] = s;
            }
        }
        WasmTensor {
            data: out,
            shape: vec![m, n],
        }
    }

    /// Reshape the tensor (returns a new tensor).
    #[wasm_bindgen]
    pub fn reshape(&self, new_shape: &[u32]) -> WasmTensor {
        let new_shape: Vec<usize> = new_shape.iter().map(|&s| s as usize).collect();
        let new_numel: usize = new_shape.iter().product();
        assert_eq!(new_numel, self.data.len());
        WasmTensor {
            data: self.data.clone(),
            shape: new_shape,
        }
    }
}

/// Log a message to the browser console.
#[wasm_bindgen]
pub fn rmi_log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}

/// Get the RMI version string.
#[wasm_bindgen]
pub fn rmi_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
