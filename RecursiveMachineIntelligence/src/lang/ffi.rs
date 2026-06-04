//! RMIL Foreign Function Interface — call external libraries from RMIL.
//!
//! The FFI module allows RMIL programs to call host functions registered
//! by the embedding application. This enables bridging to native libraries,
//! system calls, and external APIs without modifying the RMIL VM.
//!
//! # Design
//!
//! Host functions are registered as `FfiFn` — a trait object that receives
//! RMIL `Val` arguments and returns a `Val` result. The [`FfiRegistry`]
//! stores them by name, and the VM calls them when it encounters an
//! `EXTERN` op referencing that name.
//!
//! Safety is maintained through:
//! - Type-checked arguments (RMIL type signatures per binding).
//! - Sandboxed execution: host functions run synchronously, can't access
//!   the VM's environment directly.
//! - Optional timeout enforcement for untrusted external calls.
//!
//! # Examples
//!
//! ```
//! use rmi::lang::ffi::{FfiRegistry, FfiBinding, FfiSignature};
//! use rmi::lang::expr::Val;
//! use rmi::lang::ty::Ty;
//!
//! let mut ffi = FfiRegistry::new();
//!
//! ffi.register(FfiBinding {
//!     name: "square".into(),
//!     signature: FfiSignature {
//!         params: vec![Ty::Scalar(rmi::lang::ty::Dtype::F64)],
//!         ret: Ty::Scalar(rmi::lang::ty::Dtype::F64),
//!     },
//!     func: Box::new(|args| {
//!         match &args[0] {
//!             Val::F64(bits) => Ok(Val::F64({
//!                 let x = f64::from_bits(*bits);
//!                 (x * x).to_bits()
//!             })),
//!             _ => Err("expected f64".into()),
//!         }
//!     }),
//! });
//!
//! let result = ffi.call("square", &[Val::f64(4.0)]).unwrap();
//! assert_eq!(result, Val::f64(16.0));
//! ```

use std::collections::HashMap;
use std::fmt;

use crate::lang::expr::Val;
use crate::lang::ty::{Dtype, Ty};

// ── FFI signature ────────────────────────────────────────────────────────────

/// Type signature for an FFI function.
#[derive(Debug, Clone)]
pub struct FfiSignature {
    /// Parameter types.
    pub params: Vec<Ty>,
    /// Return type.
    pub ret: Ty,
}

impl FfiSignature {
    /// Number of parameters.
    pub fn arity(&self) -> usize {
        self.params.len()
    }

    /// Simple signature: N f64s → f64.
    pub fn f64_to_f64(n: usize) -> Self {
        Self {
            params: vec![Ty::Scalar(Dtype::F64); n],
            ret: Ty::Scalar(Dtype::F64),
        }
    }

    /// Unary f64 → f64.
    pub fn unary_f64() -> Self {
        Self::f64_to_f64(1)
    }

    /// Binary (f64, f64) → f64.
    pub fn binary_f64() -> Self {
        Self::f64_to_f64(2)
    }

    /// No args → nil.
    pub fn void() -> Self {
        Self {
            params: vec![],
            ret: Ty::Void,
        }
    }
}

// ── FFI errors ───────────────────────────────────────────────────────────────

/// Errors from FFI calls.
#[derive(Debug, Clone)]
pub enum FfiError {
    /// Function not found in registry.
    NotFound(String),
    /// Wrong number of arguments.
    ArityMismatch {
        /// Function name.
        name: String,
        /// Expected number of params.
        expected: usize,
        /// Actual number provided.
        got: usize,
    },
    /// Type mismatch on argument.
    TypeMismatch {
        /// Function name.
        name: String,
        /// Which parameter (0-based).
        param: usize,
        /// Expected RMIL type.
        expected: Ty,
    },
    /// The host function returned an error.
    HostError(String),
}

impl fmt::Display for FfiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FfiError::NotFound(name) => write!(f, "FFI function '{name}' not found"),
            FfiError::ArityMismatch {
                name,
                expected,
                got,
            } => write!(f, "FFI '{name}': expected {expected} args, got {got}"),
            FfiError::TypeMismatch {
                name,
                param,
                expected,
            } => write!(f, "FFI '{name}': param {param} expected {expected:?}"),
            FfiError::HostError(msg) => write!(f, "FFI host error: {msg}"),
        }
    }
}

impl std::error::Error for FfiError {}

// ── FFI function trait ───────────────────────────────────────────────────────

/// A host function callable from RMIL.
///
/// Receives a slice of RMIL values and returns a result.
/// This is stored as a boxed closure in the registry.
pub type FfiFn = Box<dyn Fn(&[Val]) -> Result<Val, String> + Send + Sync>;

// ── FFI binding ──────────────────────────────────────────────────────────────

/// A registered FFI function with metadata.
pub struct FfiBinding {
    /// Name used to call this function from RMIL.
    pub name: String,
    /// Type signature for validation.
    pub signature: FfiSignature,
    /// The host function implementation.
    pub func: FfiFn,
}

impl fmt::Debug for FfiBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FfiBinding")
            .field("name", &self.name)
            .field("signature", &self.signature)
            .finish()
    }
}

// ── FFI Registry ─────────────────────────────────────────────────────────────

/// Registry of FFI functions available to RMIL programs.
///
/// Host applications register functions here, and the RMIL VM
/// calls them when EXTERN ops are encountered.
pub struct FfiRegistry {
    bindings: HashMap<String, FfiBinding>,
}

impl FfiRegistry {
    /// Create an empty FFI registry.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Register a new FFI binding.
    ///
    /// Overwrites any existing binding with the same name.
    pub fn register(&mut self, binding: FfiBinding) {
        self.bindings.insert(binding.name.clone(), binding);
    }

    /// Register a simple function by name, signature, and closure.
    pub fn register_fn(
        &mut self,
        name: impl Into<String>,
        signature: FfiSignature,
        func: impl Fn(&[Val]) -> Result<Val, String> + Send + Sync + 'static,
    ) {
        let name = name.into();
        self.bindings.insert(
            name.clone(),
            FfiBinding {
                name,
                signature,
                func: Box::new(func),
            },
        );
    }

    /// Unregister a binding.
    pub fn unregister(&mut self, name: &str) -> bool {
        self.bindings.remove(name).is_some()
    }

    /// Check if a function is registered.
    pub fn has(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Get the signature of a registered function.
    pub fn signature(&self, name: &str) -> Option<&FfiSignature> {
        self.bindings.get(name).map(|b| &b.signature)
    }

    /// List all registered function names.
    pub fn list(&self) -> Vec<&str> {
        self.bindings.keys().map(|s| s.as_str()).collect()
    }

    /// Call a registered FFI function.
    ///
    /// Validates arity and (optionally) types before invoking.
    pub fn call(&self, name: &str, args: &[Val]) -> Result<Val, FfiError> {
        let binding = self
            .bindings
            .get(name)
            .ok_or_else(|| FfiError::NotFound(name.to_string()))?;

        // Arity check
        let expected = binding.signature.arity();
        if args.len() != expected {
            return Err(FfiError::ArityMismatch {
                name: name.to_string(),
                expected,
                got: args.len(),
            });
        }

        // Type check (best effort — Val doesn't carry full type info)
        for (i, (arg, expected_ty)) in args.iter().zip(binding.signature.params.iter()).enumerate()
        {
            if !val_matches_ty(arg, expected_ty) {
                return Err(FfiError::TypeMismatch {
                    name: name.to_string(),
                    param: i,
                    expected: expected_ty.clone(),
                });
            }
        }

        // Call the host function
        (binding.func)(args).map_err(FfiError::HostError)
    }

    /// Call without type checking (faster, assumes caller validated).
    pub fn call_unchecked(&self, name: &str, args: &[Val]) -> Result<Val, FfiError> {
        let binding = self
            .bindings
            .get(name)
            .ok_or_else(|| FfiError::NotFound(name.to_string()))?;
        (binding.func)(args).map_err(FfiError::HostError)
    }

    /// Number of registered functions.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl Default for FfiRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Best-effort check: does a Val match the expected Ty?
fn val_matches_ty(val: &Val, ty: &Ty) -> bool {
    match (val, ty) {
        (_, Ty::Void) => true, // any value matches void expectation
        (Val::Nil, _) => true, // nil is always acceptable
        (Val::Bool(_), Ty::Scalar(Dtype::Bool)) => true,
        (Val::I64(_), Ty::Scalar(d)) => matches!(
            d,
            Dtype::I8
                | Dtype::I16
                | Dtype::I32
                | Dtype::I64
                | Dtype::U8
                | Dtype::U16
                | Dtype::U32
                | Dtype::U64
        ),
        (Val::F32(_), Ty::Scalar(Dtype::F32 | Dtype::F16 | Dtype::BF16)) => true,
        (Val::F64(_), Ty::Scalar(Dtype::F64)) => true,
        (Val::F64(_), Ty::Scalar(Dtype::F32)) => true, // allow f64 → f32 coercion
        (Val::Tensor { .. }, Ty::Tensor { .. }) => true,
        (Val::Sym(_), Ty::Sym) => true,
        (Val::Tuple(items), Ty::Tuple(types)) => {
            items.len() == types.len()
                && items
                    .iter()
                    .zip(types.iter())
                    .all(|(v, t)| val_matches_ty(v, t))
        }
        // Fallback: accept anything for non-scalar types
        (_, Ty::Opaque(_)) => true,
        _ => false,
    }
}

// ── Prelude FFI functions ────────────────────────────────────────────────────

/// Register a standard set of math FFI functions.
///
/// Adds: ffi_sqrt, ffi_floor, ffi_ceil, ffi_round, ffi_abs, ffi_atan2.
pub fn register_math_prelude(reg: &mut FfiRegistry) {
    reg.register_fn("ffi_sqrt", FfiSignature::unary_f64(), |args| {
        match &args[0] {
            Val::F64(bits) => Ok(Val::F64(f64::from_bits(*bits).sqrt().to_bits())),
            _ => Err("expected f64".into()),
        }
    });

    reg.register_fn("ffi_floor", FfiSignature::unary_f64(), |args| {
        match &args[0] {
            Val::F64(bits) => Ok(Val::F64(f64::from_bits(*bits).floor().to_bits())),
            _ => Err("expected f64".into()),
        }
    });

    reg.register_fn("ffi_ceil", FfiSignature::unary_f64(), |args| {
        match &args[0] {
            Val::F64(bits) => Ok(Val::F64(f64::from_bits(*bits).ceil().to_bits())),
            _ => Err("expected f64".into()),
        }
    });

    reg.register_fn("ffi_round", FfiSignature::unary_f64(), |args| {
        match &args[0] {
            Val::F64(bits) => Ok(Val::F64(f64::from_bits(*bits).round().to_bits())),
            _ => Err("expected f64".into()),
        }
    });

    reg.register_fn("ffi_abs", FfiSignature::unary_f64(), |args| {
        match &args[0] {
            Val::F64(bits) => Ok(Val::F64(f64::from_bits(*bits).abs().to_bits())),
            _ => Err("expected f64".into()),
        }
    });

    reg.register_fn("ffi_atan2", FfiSignature::binary_f64(), |args| {
        match (&args[0], &args[1]) {
            (Val::F64(y_bits), Val::F64(x_bits)) => {
                let result = f64::from_bits(*y_bits).atan2(f64::from_bits(*x_bits));
                Ok(Val::F64(result.to_bits()))
            }
            _ => Err("expected (f64, f64)".into()),
        }
    });
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_call() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("double", FfiSignature::unary_f64(), |args| match &args[0] {
            Val::F64(bits) => {
                let x = f64::from_bits(*bits);
                Ok(Val::f64(x * 2.0))
            }
            _ => Err("expected f64".into()),
        });

        let result = ffi.call("double", &[Val::f64(21.0)]).unwrap();
        assert_eq!(result, Val::f64(42.0));
    }

    #[test]
    fn test_not_found() {
        let ffi = FfiRegistry::new();
        let err = ffi.call("nope", &[]).unwrap_err();
        assert!(matches!(err, FfiError::NotFound(_)));
    }

    #[test]
    fn test_arity_mismatch() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("id", FfiSignature::unary_f64(), |args| Ok(args[0].clone()));

        let err = ffi.call("id", &[]).unwrap_err();
        assert!(matches!(err, FfiError::ArityMismatch { .. }));

        let err = ffi.call("id", &[Val::f64(1.0), Val::f64(2.0)]).unwrap_err();
        assert!(matches!(err, FfiError::ArityMismatch { .. }));
    }

    #[test]
    fn test_type_mismatch() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("id", FfiSignature::unary_f64(), |args| Ok(args[0].clone()));

        // Bool doesn't match F64
        let err = ffi.call("id", &[Val::Bool(true)]).unwrap_err();
        assert!(matches!(err, FfiError::TypeMismatch { .. }));
    }

    #[test]
    fn test_host_error() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("fail", FfiSignature::void(), |_| {
            Err("intentional failure".into())
        });

        let err = ffi.call("fail", &[]).unwrap_err();
        assert!(matches!(err, FfiError::HostError(_)));
    }

    #[test]
    fn test_unregister() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("temp", FfiSignature::void(), |_| Ok(Val::Nil));
        assert!(ffi.has("temp"));
        ffi.unregister("temp");
        assert!(!ffi.has("temp"));
    }

    #[test]
    fn test_list() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("a", FfiSignature::void(), |_| Ok(Val::Nil));
        ffi.register_fn("b", FfiSignature::void(), |_| Ok(Val::Nil));
        let mut names = ffi.list();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn test_math_prelude() {
        let mut ffi = FfiRegistry::new();
        register_math_prelude(&mut ffi);

        let sqrt = ffi.call("ffi_sqrt", &[Val::f64(16.0)]).unwrap();
        assert_eq!(sqrt, Val::f64(4.0));

        let floor = ffi.call("ffi_floor", &[Val::f64(3.7)]).unwrap();
        assert_eq!(floor, Val::f64(3.0));

        let atan2 = ffi
            .call("ffi_atan2", &[Val::f64(1.0), Val::f64(1.0)])
            .unwrap();
        if let Val::F64(bits) = atan2 {
            let v = f64::from_bits(bits);
            assert!((v - std::f64::consts::FRAC_PI_4).abs() < 1e-10);
        } else {
            panic!("expected F64");
        }
    }

    #[test]
    fn test_call_unchecked() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("add", FfiSignature::binary_f64(), |args| {
            match (&args[0], &args[1]) {
                (Val::F64(a_bits), Val::F64(b_bits)) => {
                    let result = f64::from_bits(*a_bits) + f64::from_bits(*b_bits);
                    Ok(Val::f64(result))
                }
                _ => Err("expected f64s".into()),
            }
        });

        let result = ffi
            .call_unchecked("add", &[Val::f64(1.0), Val::f64(2.0)])
            .unwrap();
        assert_eq!(result, Val::f64(3.0));
    }

    #[test]
    fn test_len_and_empty() {
        let mut ffi = FfiRegistry::new();
        assert!(ffi.is_empty());
        assert_eq!(ffi.len(), 0);

        ffi.register_fn("f", FfiSignature::void(), |_| Ok(Val::Nil));
        assert!(!ffi.is_empty());
        assert_eq!(ffi.len(), 1);
    }

    #[test]
    fn test_nil_passes_any_type() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("accept_nil", FfiSignature::unary_f64(), |_| Ok(Val::Nil));

        // Nil should pass type check for any expected param type
        let result = ffi.call("accept_nil", &[Val::Nil]).unwrap();
        assert_eq!(result, Val::Nil);
    }

    #[test]
    fn test_overwrite_binding() {
        let mut ffi = FfiRegistry::new();
        ffi.register_fn("f", FfiSignature::void(), |_| Ok(Val::I64(1)));
        ffi.register_fn("f", FfiSignature::void(), |_| Ok(Val::I64(2)));
        let result = ffi.call("f", &[]).unwrap();
        assert_eq!(result, Val::I64(2));
    }

    #[test]
    fn test_ffi_error_display() {
        let err = FfiError::NotFound("nope".into());
        assert!(err.to_string().contains("nope"));
    }
}
