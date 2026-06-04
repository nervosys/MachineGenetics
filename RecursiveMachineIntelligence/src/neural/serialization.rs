//! Model Serialization Module
//!
//! Provides convenient model save/load using the safetensors-compatible
//! `TensorStorage` format from the core storage module. Supports saving
//! and restoring layer parameters, training history, and model metadata.
//!
//! # Example
//!
//! ```no_run
//! use rmi::neural::serialization::{ModelSerializer, ModelMetadata};
//! use rmi::neural::{Linear, Trainer, TrainerConfig, MSELoss, SGD};
//! use rmi::neural::layers::Layer;
//! use rmi::neural::loss::Loss;
//! use rmi::neural::optim::Optimizer;
//!
//! // Build a model
//! let layers: Vec<Box<dyn Layer>> = vec![
//!     Box::new(Linear::new(4, 8)),
//!     Box::new(Linear::new(8, 2)),
//! ];
//!
//! // Save
//! ModelSerializer::save_layers("model.rmi", &layers, None).unwrap();
//!
//! // Load back
//! let (params, meta) = ModelSerializer::load("model.rmi").unwrap();
//! assert_eq!(params.len(), 4); // 2 weights + 2 biases
//! ```

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::RmiError;

use super::layers::Layer;
use super::training::TrainingHistory;

// ============================================================================
// Model metadata
// ============================================================================

/// Metadata stored alongside model weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Model name/identifier.
    pub name: String,
    /// Framework version that created the file.
    pub version: String,
    /// Layer names in order.
    pub layer_names: Vec<String>,
    /// Number of parameters per layer.
    pub layer_param_counts: Vec<usize>,
    /// Total number of parameters.
    pub total_parameters: usize,
    /// Optional training history.
    pub training_history: Option<SerializableHistory>,
    /// Optional custom key-value metadata.
    pub custom: HashMap<String, String>,
}

/// Serializable training history (mirrors [`TrainingHistory`] but with serde).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableHistory {
    /// Per-epoch training losses.
    pub losses: Vec<f32>,
    /// Per-epoch validation losses.
    pub val_losses: Vec<f32>,
    /// Per-epoch learning rates.
    pub learning_rates: Vec<f32>,
}

impl From<&TrainingHistory> for SerializableHistory {
    fn from(h: &TrainingHistory) -> Self {
        Self {
            losses: h.losses.clone(),
            val_losses: h.val_losses.clone(),
            learning_rates: h.learning_rates.clone(),
        }
    }
}

impl From<&SerializableHistory> for TrainingHistory {
    fn from(h: &SerializableHistory) -> Self {
        Self {
            losses: h.losses.clone(),
            val_losses: h.val_losses.clone(),
            learning_rates: h.learning_rates.clone(),
        }
    }
}

// ============================================================================
// Named parameter
// ============================================================================

/// A named parameter tensor (name, shape, data).
#[derive(Debug, Clone)]
pub struct NamedParameter {
    /// Parameter name (e.g. `"layer_0.weight"`).
    pub name: String,
    /// Tensor shape.
    pub shape: Vec<usize>,
    /// Flattened f32 data.
    pub data: Vec<f32>,
}

// ============================================================================
// File format
// ============================================================================
//
// The `.rmi` model format:
//   [8 bytes] magic: b"RMIMODEL"
//   [4 bytes] metadata_len (u32 LE)
//   [metadata_len bytes] JSON-encoded ModelMetadata
//   [4 bytes] num_tensors (u32 LE)
//   For each tensor:
//       [4 bytes] name_len (u32 LE)
//       [name_len bytes] name (UTF-8)
//       [4 bytes] ndims (u32 LE)
//       [ndims * 8 bytes] shape (u64 LE each)
//       [8 bytes] data_len_bytes (u64 LE)
//       [data_len_bytes] f32 LE data
//

const MAGIC: &[u8; 8] = b"RMIMODEL";

// ============================================================================
// ModelSerializer
// ============================================================================

/// High-level model save/load.
pub struct ModelSerializer;

impl ModelSerializer {
    /// Save layer parameters to a `.rmi` file.
    ///
    /// Optionally includes training history and custom metadata.
    pub fn save_layers(
        path: impl AsRef<Path>,
        layers: &[Box<dyn Layer>],
        history: Option<&TrainingHistory>,
    ) -> Result<()> {
        let params = Self::extract_params(layers);
        let meta = ModelMetadata {
            name: String::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            layer_names: layers.iter().map(|l| l.name().to_string()).collect(),
            layer_param_counts: layers.iter().map(|l| l.num_parameters()).collect(),
            total_parameters: layers.iter().map(|l| l.num_parameters()).sum(),
            training_history: history.map(SerializableHistory::from),
            custom: HashMap::new(),
        };

        Self::save_raw(path, &params, &meta)
    }

    /// Save named parameters and metadata to a `.rmi` file.
    pub fn save_raw(
        path: impl AsRef<Path>,
        params: &[NamedParameter],
        metadata: &ModelMetadata,
    ) -> Result<()> {
        let mut file = File::create(path.as_ref())?;

        // Magic
        file.write_all(MAGIC)?;

        // Metadata
        let meta_json =
            serde_json::to_vec(metadata).map_err(|e| RmiError::Serialization(e.to_string()))?;
        file.write_all(&(meta_json.len() as u32).to_le_bytes())?;
        file.write_all(&meta_json)?;

        // Tensors
        file.write_all(&(params.len() as u32).to_le_bytes())?;
        for param in params {
            // name
            let name_bytes = param.name.as_bytes();
            file.write_all(&(name_bytes.len() as u32).to_le_bytes())?;
            file.write_all(name_bytes)?;

            // shape
            file.write_all(&(param.shape.len() as u32).to_le_bytes())?;
            for &dim in &param.shape {
                file.write_all(&(dim as u64).to_le_bytes())?;
            }

            // data
            let data_bytes: Vec<u8> = param.data.iter().flat_map(|f| f.to_le_bytes()).collect();
            file.write_all(&(data_bytes.len() as u64).to_le_bytes())?;
            file.write_all(&data_bytes)?;
        }

        Ok(())
    }

    /// Load parameters and metadata from a `.rmi` file.
    pub fn load(path: impl AsRef<Path>) -> Result<(Vec<NamedParameter>, ModelMetadata)> {
        let data = fs::read(path.as_ref())?;
        let mut pos = 0;

        // Magic
        if data.len() < 8 || &data[0..8] != MAGIC {
            return Err(RmiError::Serialization("Invalid RMI model magic".into()));
        }
        pos += 8;

        // Metadata
        if data.len() < pos + 4 {
            return Err(RmiError::Serialization("Truncated metadata length".into()));
        }
        let meta_len =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if data.len() < pos + meta_len {
            return Err(RmiError::Serialization("Truncated metadata".into()));
        }
        let metadata: ModelMetadata = serde_json::from_slice(&data[pos..pos + meta_len])
            .map_err(|e| RmiError::Serialization(e.to_string()))?;
        pos += meta_len;

        // Num tensors
        if data.len() < pos + 4 {
            return Err(RmiError::Serialization("Truncated tensor count".into()));
        }
        let num_tensors =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        let mut params = Vec::with_capacity(num_tensors);
        for _ in 0..num_tensors {
            // name
            if data.len() < pos + 4 {
                return Err(RmiError::Serialization("Truncated name length".into()));
            }
            let name_len =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                    as usize;
            pos += 4;
            let name = String::from_utf8_lossy(&data[pos..pos + name_len]).to_string();
            pos += name_len;

            // shape
            if data.len() < pos + 4 {
                return Err(RmiError::Serialization("Truncated ndims".into()));
            }
            let ndims = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                as usize;
            pos += 4;
            let mut shape = Vec::with_capacity(ndims);
            for _ in 0..ndims {
                let dim = u64::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                    data[pos + 4],
                    data[pos + 5],
                    data[pos + 6],
                    data[pos + 7],
                ]) as usize;
                pos += 8;
                shape.push(dim);
            }

            // data
            let data_len = u64::from_le_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
                data[pos + 4],
                data[pos + 5],
                data[pos + 6],
                data[pos + 7],
            ]) as usize;
            pos += 8;
            let values: Vec<f32> = data[pos..pos + data_len]
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            pos += data_len;

            params.push(NamedParameter {
                name,
                shape,
                data: values,
            });
        }

        Ok((params, metadata))
    }

    /// Load parameters from file and apply them to existing layers.
    ///
    /// Parameters are matched by index order: the first layer's weight
    /// is matched with `"layer_0.weight"`, its bias with `"layer_0.bias"`, etc.
    pub fn load_into(
        path: impl AsRef<Path>,
        layers: &mut [Box<dyn Layer>],
    ) -> Result<ModelMetadata> {
        let (params, meta) = Self::load(path)?;
        let param_map: HashMap<String, &NamedParameter> =
            params.iter().map(|p| (p.name.clone(), p)).collect();

        for (i, layer) in layers.iter_mut().enumerate() {
            for (param_idx, param) in layer.parameters_mut().iter_mut().enumerate() {
                let key = format!("layer_{}.param_{}", i, param_idx);
                if let Some(saved) = param_map.get(&key) {
                    if param.data.len() == saved.data.len() {
                        param.data.clone_from(&saved.data);
                    }
                }
            }
        }

        Ok(meta)
    }

    /// Extract named parameters from layers.
    fn extract_params(layers: &[Box<dyn Layer>]) -> Vec<NamedParameter> {
        let mut params = Vec::new();
        for (i, layer) in layers.iter().enumerate() {
            for (j, param) in layer.parameters().iter().enumerate() {
                params.push(NamedParameter {
                    name: format!("layer_{}.param_{}", i, j),
                    shape: param.shape.clone(),
                    data: param.data.clone(),
                });
            }
        }
        params
    }
}

/// Convert a model's parameters to a JSON-compatible dict for inspection.
pub fn params_to_json(layers: &[Box<dyn Layer>]) -> serde_json::Value {
    let params = ModelSerializer::extract_params(layers);
    let mut map = serde_json::Map::new();
    for p in &params {
        map.insert(
            p.name.clone(),
            serde_json::json!({
                "shape": p.shape,
                "numel": p.data.len(),
                "mean": p.data.iter().sum::<f32>() / p.data.len().max(1) as f32,
                "min": p.data.iter().copied().reduce(f32::min),
                "max": p.data.iter().copied().reduce(f32::max),
            }),
        );
    }
    serde_json::Value::Object(map)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::Linear;
    use tempfile::NamedTempFile;

    fn make_layers() -> Vec<Box<dyn Layer>> {
        vec![Box::new(Linear::new(4, 8)), Box::new(Linear::new(8, 2))]
    }

    #[test]
    fn test_save_load_roundtrip() {
        let layers = make_layers();
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        ModelSerializer::save_layers(path, &layers, None).unwrap();

        let (params, meta) = ModelSerializer::load(path).unwrap();

        // 2 layers: Linear(4,8) => weight(4*8) + bias(8), Linear(8,2) => weight(8*2) + bias(2)
        assert_eq!(params.len(), 4);
        assert_eq!(meta.layer_names.len(), 2);
        assert_eq!(meta.total_parameters, 4 * 8 + 8 + 8 * 2 + 2);

        // Verify data matches
        let original = ModelSerializer::extract_params(&layers);
        for (orig, loaded) in original.iter().zip(params.iter()) {
            assert_eq!(orig.name, loaded.name);
            assert_eq!(orig.shape, loaded.shape);
            assert_eq!(orig.data.len(), loaded.data.len());
            for (a, b) in orig.data.iter().zip(loaded.data.iter()) {
                assert!((a - b).abs() < 1e-7, "data mismatch: {} vs {}", a, b);
            }
        }
    }

    #[test]
    fn test_save_with_history() {
        let layers = make_layers();
        let history = TrainingHistory {
            losses: vec![1.0, 0.5, 0.3],
            val_losses: vec![1.1, 0.6, 0.35],
            learning_rates: vec![0.01, 0.01, 0.005],
        };

        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        ModelSerializer::save_layers(path, &layers, Some(&history)).unwrap();
        let (_, meta) = ModelSerializer::load(path).unwrap();

        let h = meta.training_history.unwrap();
        assert_eq!(h.losses, vec![1.0, 0.5, 0.3]);
        assert_eq!(h.val_losses, vec![1.1, 0.6, 0.35]);
    }

    #[test]
    fn test_load_into_layers() {
        let layers = make_layers();
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();

        ModelSerializer::save_layers(path, &layers, None).unwrap();

        // Create fresh layers (different random weights)
        let mut new_layers = make_layers();
        ModelSerializer::load_into(path, &mut new_layers).unwrap();

        // After loading, parameters should match the saved ones
        let original_params = ModelSerializer::extract_params(&layers);
        let loaded_params = ModelSerializer::extract_params(&new_layers);

        for (orig, loaded) in original_params.iter().zip(loaded_params.iter()) {
            assert_eq!(orig.data.len(), loaded.data.len());
            for (a, b) in orig.data.iter().zip(loaded.data.iter()) {
                assert!((a - b).abs() < 1e-7);
            }
        }
    }

    #[test]
    fn test_invalid_magic() {
        let tmp = NamedTempFile::new().unwrap();
        fs::write(tmp.path(), b"BADMAGIC").unwrap();
        assert!(ModelSerializer::load(tmp.path()).is_err());
    }

    #[test]
    fn test_params_to_json() {
        let layers = make_layers();
        let json = params_to_json(&layers);
        assert!(json.is_object());
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("layer_0.param_0"));
        assert!(obj.contains_key("layer_1.param_1"));
    }

    #[test]
    fn test_named_parameter_format() {
        let layers = make_layers();
        let params = ModelSerializer::extract_params(&layers);
        assert_eq!(params[0].name, "layer_0.param_0");
        assert_eq!(params[1].name, "layer_0.param_1");
        assert_eq!(params[2].name, "layer_1.param_0");
        assert_eq!(params[3].name, "layer_1.param_1");
    }

    #[test]
    fn test_empty_model() {
        let layers: Vec<Box<dyn Layer>> = vec![];
        let tmp = NamedTempFile::new().unwrap();

        ModelSerializer::save_layers(tmp.path(), &layers, None).unwrap();
        let (params, meta) = ModelSerializer::load(tmp.path()).unwrap();
        assert!(params.is_empty());
        assert_eq!(meta.total_parameters, 0);
    }

    #[test]
    fn test_metadata_version() {
        let layers = make_layers();
        let tmp = NamedTempFile::new().unwrap();

        ModelSerializer::save_layers(tmp.path(), &layers, None).unwrap();
        let (_, meta) = ModelSerializer::load(tmp.path()).unwrap();
        assert_eq!(meta.version, env!("CARGO_PKG_VERSION"));
    }
}
