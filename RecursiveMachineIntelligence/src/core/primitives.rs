//! Primitive System - Machine-Native Building Blocks
//!
//! Primitives are the atomic units of computation and reasoning that AI agents
//! can compose, analyze, and optimize. Unlike human-oriented APIs, primitives
//! expose their full algebraic structure for machine reasoning.
//!
//! Each primitive carries:
//! - Formal type signature
//! - Algebraic properties (associativity, commutativity, etc.)
//! - Computational complexity bounds
//! - Differentiability information
//! - Hardware affinity hints

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{RmiError, Result};

/// Classification of primitives by their computational nature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum PrimitiveType {
    // Tensor operations
    /// Create tensors (zeros, ones, random)
    TensorCreate = 0x0100,
    /// Transform tensors (reshape, transpose, slice)
    TensorTransform = 0x0101,
    /// Arithmetic operations (add, mul, matmul)
    TensorArithmetic = 0x0102,
    /// Reduction operations (sum, mean, max)
    TensorReduce = 0x0103,
    /// Comparison operations (eq, lt, gt)
    TensorCompare = 0x0104,

    // Neural operations
    /// Linear transformations (linear, conv, attention)
    NeuralLinear = 0x0200,
    /// Non-linear activations (relu, gelu, softmax)
    NeuralNonlinear = 0x0201,
    /// Normalization (batchnorm, layernorm)
    NeuralNorm = 0x0202,
    /// Pooling (maxpool, avgpool)
    NeuralPool = 0x0203,
    /// Regularization (dropout, droppath)
    NeuralDropout = 0x0204,

    // Symbolic operations
    /// Logical operations (and, or, not, implies)
    SymbolicLogic = 0x0300,
    /// Quantifiers (forall, exists)
    SymbolicQuantifier = 0x0301,
    /// Unification and substitution
    SymbolicUnify = 0x0302,
    /// Inference rules (modus_ponens, resolution)
    SymbolicInference = 0x0303,

    // Hybrid operations
    /// Symbol-vector conversion
    HybridEmbed = 0x0400,
    /// Differentiable constraints
    HybridConstrain = 0x0401,
    /// Neural queries over knowledge
    HybridQuery = 0x0402,

    // Control flow
    /// Branching (if, switch)
    ControlBranch = 0x0500,
    /// Loops (for, while, scan)
    ControlLoop = 0x0501,
    /// Parallelism (pmap, vmap)
    ControlParallel = 0x0502,
}

/// Algebraic properties that enable optimization and reasoning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AlgebraicProperty {
    /// (a ∘ b) ∘ c = a ∘ (b ∘ c)
    Associative = 0x01,
    /// a ∘ b = b ∘ a
    Commutative = 0x02,
    /// a ∘ a = a
    Idempotent = 0x03,
    /// a ∘ a = 0
    Nilpotent = 0x04,
    /// f(f(x)) = x
    Involutory = 0x05,
    /// a ∘ (b + c) = (a ∘ b) + (a ∘ c)
    DistributiveLeft = 0x06,
    /// (a + b) ∘ c = (a ∘ c) + (b ∘ c)
    DistributiveRight = 0x07,
    /// ∃e: a ∘ e = e ∘ a = a
    IdentityExists = 0x08,
    /// ∃a⁻¹: a ∘ a⁻¹ = e
    InverseExists = 0x09,
    /// x ≤ y → f(x) ≤ f(y)
    Monotonic = 0x0A,
    /// |f(x) - f(y)| ≤ K|x - y|
    Lipschitz = 0x0B,
    /// Continuous in the topological sense
    Continuous = 0x0C,
    /// Has gradient
    Differentiable = 0x0D,
    /// Has hessian
    TwiceDifferentiable = 0x0E,
    /// f(ax + by) = af(x) + bf(y)
    Linear = 0x0F,
    /// f(λx + (1-λ)y) ≤ λf(x) + (1-λ)f(y)
    Convex = 0x10,
}

/// Formal type signature for a primitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeSignature {
    /// Input types as type strings
    pub input_types: Vec<String>,
    /// Output type as string
    pub output_type: String,
    /// Type constraints (e.g., "T: Numeric")
    pub type_constraints: Vec<String>,
}

impl TypeSignature {
    /// Create a new type signature.
    pub fn new(inputs: Vec<&str>, output: &str) -> Self {
        Self {
            input_types: inputs.into_iter().map(String::from).collect(),
            output_type: output.to_string(),
            type_constraints: Vec::new(),
        }
    }

    /// Add a type constraint.
    pub fn with_constraint(mut self, constraint: &str) -> Self {
        self.type_constraints.push(constraint.to_string());
        self
    }

    /// Serialize to binary for inter-agent communication.
    pub fn to_binary(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).unwrap_or_default()
    }

    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(data).map_err(|e| RmiError::Serialization(e.to_string()))
    }
}

impl std::fmt::Display for TypeSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inputs = self.input_types.join(", ");
        write!(f, "({}) -> {}", inputs, self.output_type)?;
        if !self.type_constraints.is_empty() {
            write!(f, " where {}", self.type_constraints.join(", "))?;
        }
        Ok(())
    }
}

/// Computational complexity specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityBound {
    /// Time complexity (e.g., "O(n^2)")
    pub time_complexity: String,
    /// Space complexity
    pub space_complexity: String,
    /// Parallelism potential (e.g., "O(n)" for fully parallel)
    pub parallelism: String,
    /// FLOPs per element for cost modeling
    pub flops_per_element: f64,
    /// Bytes per element
    pub bytes_per_element: f64,
}

impl Default for ComplexityBound {
    fn default() -> Self {
        Self {
            time_complexity: "O(n)".to_string(),
            space_complexity: "O(1)".to_string(),
            parallelism: "O(n)".to_string(),
            flops_per_element: 1.0,
            bytes_per_element: 4.0,
        }
    }
}

/// Hardware efficiency hints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAffinity {
    /// CPU efficiency (0-1)
    pub cpu_efficiency: f32,
    /// CUDA efficiency (0-1)
    pub cuda_efficiency: f32,
    /// Minimum batch size for GPU efficiency
    pub min_batch_for_gpu: u32,
    /// Memory bandwidth bound
    pub memory_bound: bool,
    /// Compute bound
    pub compute_bound: bool,
    /// Has fused kernel available
    pub has_fused_kernel: bool,
    /// Supports mixed precision
    pub supports_mixed_precision: bool,
}

impl Default for HardwareAffinity {
    fn default() -> Self {
        Self {
            cpu_efficiency: 1.0,
            cuda_efficiency: 1.0,
            min_batch_for_gpu: 32,
            memory_bound: false,
            compute_bound: true,
            has_fused_kernel: false,
            supports_mixed_precision: true,
        }
    }
}

/// Information about differentiability for automatic differentiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientInfo {
    /// Is this operation differentiable?
    pub is_differentiable: bool,
    /// Has custom VJP implementation
    pub has_custom_vjp: bool,
    /// Has custom JVP implementation
    pub has_custom_jvp: bool,
    /// Symbolic gradient formula
    pub gradient_formula: Option<String>,
    /// Hessian structure ("diagonal", "block_diagonal", "full")
    pub hessian_structure: Option<String>,
    /// Is twice differentiable?
    pub is_twice_differentiable: bool,
}

impl Default for GradientInfo {
    fn default() -> Self {
        Self {
            is_differentiable: true,
            has_custom_vjp: false,
            has_custom_jvp: false,
            gradient_formula: None,
            hessian_structure: None,
            is_twice_differentiable: true,
        }
    }
}

/// Unique identifier for a primitive.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrimitiveId {
    /// Namespace (e.g., "air.neural")
    pub namespace: String,
    /// Local name
    pub name: String,
    /// Version
    pub version: u32,
}

impl PrimitiveId {
    /// Create a new primitive ID.
    #[inline]
    pub fn new(namespace: &str, name: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            name: name.to_string(),
            version: 1,
        }
    }

    /// Get the full URI.
    #[inline]
    pub fn uri(&self) -> String {
        format!(
            "air://{}/{}@v{}",
            self.namespace, self.name, self.version
        )
    }
}

impl std::fmt::Display for PrimitiveId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}@v{}", self.namespace, self.name, self.version)
    }
}

/// Base trait for all primitives in air.
///
/// Primitives are designed to be introspectable by AI agents:
/// - Full algebraic specification
/// - Complexity bounds
/// - Hardware hints
/// - Gradient information
pub trait Primitive: Send + Sync {
    /// Unique identifier for this primitive.
    fn id(&self) -> &PrimitiveId;

    /// Classification of this primitive.
    fn primitive_type(&self) -> PrimitiveType;

    /// Formal type signature.
    fn type_signature(&self) -> &TypeSignature;

    /// Algebraic properties this operation satisfies.
    fn algebraic_properties(&self) -> &HashSet<AlgebraicProperty>;

    /// Computational complexity specification.
    fn complexity(&self) -> &ComplexityBound;

    /// Hardware efficiency hints.
    fn hardware_affinity(&self) -> &HardwareAffinity {
        &DEFAULT_HARDWARE_AFFINITY
    }

    /// Differentiability information.
    fn gradient_info(&self) -> &GradientInfo {
        &DEFAULT_GRADIENT_INFO
    }

    /// Machine-readable semantic description.
    fn semantic_description(&self) -> &str {
        ""
    }

    /// Content-addressable hash for deduplication.
    fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id().uri().as_bytes());
        hasher.update([self.primitive_type() as u8]);
        hasher.update(self.type_signature().to_binary());
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// Serialize primitive specification to binary format.
    fn to_binary(&self) -> Vec<u8> {
        let spec = PrimitiveSpec {
            id: self.id().clone(),
            primitive_type: self.primitive_type(),
            type_signature: self.type_signature().clone(),
            algebraic_properties: self.algebraic_properties().iter().copied().collect(),
            complexity: self.complexity().clone(),
            hardware_affinity: self.hardware_affinity().clone(),
            gradient_info: self.gradient_info().clone(),
            semantic_description: self.semantic_description().to_string(),
        };

        // Compress with LZ4
        let packed = rmp_serde::to_vec(&spec).unwrap_or_default();
        lz4_flex::compress_prepend_size(&packed)
    }
}

// Static defaults to avoid allocation
lazy_static::lazy_static! {
    /// Default hardware affinity
    pub static ref DEFAULT_HARDWARE_AFFINITY: HardwareAffinity = HardwareAffinity::default();
    /// Default gradient info
    pub static ref DEFAULT_GRADIENT_INFO: GradientInfo = GradientInfo::default();
}

/// Serializable primitive specification for persistence and transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimitiveSpec {
    /// Unique primitive identifier
    pub id: PrimitiveId,
    /// Type classification of the primitive
    pub primitive_type: PrimitiveType,
    /// Type signature for inputs and outputs
    pub type_signature: TypeSignature,
    /// Algebraic properties for optimization
    pub algebraic_properties: Vec<AlgebraicProperty>,
    /// Computational complexity bounds
    pub complexity: ComplexityBound,
    /// Hardware execution preferences
    pub hardware_affinity: HardwareAffinity,
    /// Gradient computation information
    pub gradient_info: GradientInfo,
    /// Human-readable semantic description
    pub semantic_description: String,
}

impl PrimitiveSpec {
    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        let decompressed = lz4_flex::decompress_size_prepended(data)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;
        rmp_serde::from_slice(&decompressed)
            .map_err(|e| RmiError::Serialization(e.to_string()))
    }
}

/// Global registry of all primitives.
///
/// Agents can query the registry to discover available primitives,
/// search by properties, and compose new operations.
pub struct PrimitiveRegistry {
    primitives: RwLock<HashMap<PrimitiveId, Arc<dyn Primitive>>>,
    by_type: RwLock<HashMap<PrimitiveType, HashSet<PrimitiveId>>>,
    by_property: RwLock<HashMap<AlgebraicProperty, HashSet<PrimitiveId>>>,
}

impl PrimitiveRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            primitives: RwLock::new(HashMap::new()),
            by_type: RwLock::new(HashMap::new()),
            by_property: RwLock::new(HashMap::new()),
        }
    }

    /// Register a primitive in the global registry.
    pub fn register(&self, primitive: Arc<dyn Primitive>) {
        let id = primitive.id().clone();
        let ptype = primitive.primitive_type();
        let properties = primitive.algebraic_properties().clone();

        // Insert primitive
        self.primitives
            .write()
            .unwrap()
            .insert(id.clone(), primitive);

        // Index by type
        self.by_type
            .write()
            .unwrap()
            .entry(ptype)
            .or_default()
            .insert(id.clone());

        // Index by properties
        let mut by_prop = self.by_property.write().unwrap();
        for prop in properties {
            by_prop
                .entry(prop)
                .or_default()
                .insert(id.clone());
        }
    }

    /// Batch-register primitives (amortizes lock acquisition over N primitives).
    pub fn register_batch(&self, primitives: Vec<Arc<dyn Primitive>>) {
        let mut prim_map = self.primitives.write().unwrap();
        let mut type_map = self.by_type.write().unwrap();
        let mut prop_map = self.by_property.write().unwrap();

        for primitive in primitives {
            let id = primitive.id().clone();
            let ptype = primitive.primitive_type();
            let properties = primitive.algebraic_properties().clone();

            prim_map.insert(id.clone(), primitive);
            type_map
                .entry(ptype)
                .or_default()
                .insert(id.clone());
            for prop in properties {
                prop_map
                    .entry(prop)
                    .or_default()
                    .insert(id.clone());
            }
        }
    }

    /// Retrieve a primitive by ID.
    #[inline]
    pub fn get(&self, id: &PrimitiveId) -> Option<Arc<dyn Primitive>> {
        self.primitives.read().unwrap().get(id).cloned()
    }

    /// Find all primitives of a given type.
    pub fn query_by_type(&self, ptype: PrimitiveType) -> Vec<Arc<dyn Primitive>> {
        let by_type = self.by_type.read().unwrap();
        let primitives = self.primitives.read().unwrap();

        by_type
            .get(&ptype)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| primitives.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find primitives with specific algebraic properties.
    pub fn query_by_properties(
        &self,
        required: &HashSet<AlgebraicProperty>,
        forbidden: Option<&HashSet<AlgebraicProperty>>,
    ) -> Vec<Arc<dyn Primitive>> {
        let by_prop = self.by_property.read().unwrap();
        let primitives = self.primitives.read().unwrap();

        // Find candidates with all required properties
        let mut candidates: Option<HashSet<PrimitiveId>> = None;

        for prop in required {
            if let Some(ids) = by_prop.get(prop) {
                candidates = Some(match candidates {
                    Some(c) => c.intersection(ids).cloned().collect(),
                    None => ids.clone(),
                });
            } else {
                return Vec::new(); // Required property not found
            }
        }

        let mut result_ids = candidates.unwrap_or_else(|| primitives.keys().cloned().collect());

        // Remove forbidden
        if let Some(forbidden) = forbidden {
            for prop in forbidden {
                if let Some(ids) = by_prop.get(prop) {
                    result_ids = result_ids.difference(ids).cloned().collect();
                }
            }
        }

        result_ids
            .iter()
            .filter_map(|id| primitives.get(id).cloned())
            .collect()
    }

    /// Export the entire primitive catalog as compressed binary.
    pub fn export_catalog(&self) -> Vec<u8> {
        let primitives = self.primitives.read().unwrap();

        let specs: Vec<PrimitiveSpec> = primitives
            .values()
            .map(|p| PrimitiveSpec {
                id: p.id().clone(),
                primitive_type: p.primitive_type(),
                type_signature: p.type_signature().clone(),
                algebraic_properties: p.algebraic_properties().iter().copied().collect(),
                complexity: p.complexity().clone(),
                hardware_affinity: p.hardware_affinity().clone(),
                gradient_info: p.gradient_info().clone(),
                semantic_description: p.semantic_description().to_string(),
            })
            .collect();

        let packed = rmp_serde::to_vec(&specs).unwrap_or_default();
        lz4_flex::compress_prepend_size(&packed)
    }

    /// Get the number of registered primitives.
    #[inline]
    pub fn len(&self) -> usize {
        self.primitives.read().unwrap().len()
    }

    /// Check if the registry is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.primitives.read().unwrap().is_empty()
    }
}

impl Default for PrimitiveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global registry instance.
static GLOBAL_REGISTRY: std::sync::OnceLock<PrimitiveRegistry> = std::sync::OnceLock::new();

/// Get the global primitive registry.
pub fn global_registry() -> &'static PrimitiveRegistry {
    GLOBAL_REGISTRY.get_or_init(PrimitiveRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_signature() {
        let sig = TypeSignature::new(
            vec!["Tensor[f32, N, M]", "Tensor[f32, M, K]"],
            "Tensor[f32, N, K]",
        )
        .with_constraint("N > 0")
        .with_constraint("M > 0");

        assert_eq!(sig.input_types.len(), 2);
        assert_eq!(sig.type_constraints.len(), 2);

        let binary = sig.to_binary();
        let restored = TypeSignature::from_binary(&binary).unwrap();
        assert_eq!(sig, restored);
    }

    #[test]
    fn test_primitive_id() {
        let id = PrimitiveId::new("air.neural", "matmul");
        assert_eq!(id.uri(), "air://air.neural/matmul@v1");
    }

    #[test]
    fn test_type_signature_display() {
        let sig = TypeSignature::new(
            vec!["Tensor[f32, N, M]", "Tensor[f32, M, K]"],
            "Tensor[f32, N, K]",
        )
        .with_constraint("N > 0");

        let display = format!("{}", sig);
        assert!(display.contains("Tensor[f32, N, M]"));
        assert!(display.contains("where"));
    }

    #[test]
    fn test_type_signature_binary_roundtrip() {
        let sig = TypeSignature::new(vec!["f32", "f32"], "f32")
            .with_constraint("positive");

        let binary = sig.to_binary();
        let restored = TypeSignature::from_binary(&binary).unwrap();
        assert_eq!(sig, restored);
    }

    #[test]
    fn test_primitive_id_display() {
        let id = PrimitiveId::new("air.tensor", "add");
        let display = format!("{}", id);
        assert!(display.contains("air.tensor"));
        assert!(display.contains("add"));
    }

    #[test]
    fn test_primitive_id_versioning() {
        let id = PrimitiveId::new("air.neural", "conv2d");
        assert_eq!(id.version, 1);
        assert_eq!(id.uri(), "air://air.neural/conv2d@v1");
    }

    #[test]
    fn test_primitive_registry_empty() {
        let reg = PrimitiveRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.get(&PrimitiveId::new("ns", "name")).is_none());
    }

    #[test]
    fn test_primitive_registry_query_empty_type() {
        let reg = PrimitiveRegistry::new();
        let result = reg.query_by_type(PrimitiveType::NeuralLinear);
        assert!(result.is_empty());
    }

    #[test]
    fn test_global_registry() {
        let reg = global_registry();
        // Just verify it's accessible and initialized
        let _len = reg.len(); // verify accessible
    }

    #[test]
    fn test_complexity_bound_defaults() {
        let cb = ComplexityBound::default();
        assert_eq!(cb.time_complexity, "O(n)");
        assert_eq!(cb.space_complexity, "O(1)");
        assert_eq!(cb.flops_per_element, 1.0);
    }

    #[test]
    fn test_hardware_affinity_defaults() {
        let ha = HardwareAffinity::default();
        assert_eq!(ha.cpu_efficiency, 1.0);
        assert_eq!(ha.cuda_efficiency, 1.0);
        assert!(ha.supports_mixed_precision);
    }

    #[test]
    fn test_gradient_info_defaults() {
        let gi = GradientInfo::default();
        assert!(gi.is_differentiable);
        assert!(gi.is_twice_differentiable);
        assert!(!gi.has_custom_vjp);
    }
}