use serde::{Deserialize, Serialize};

/// An effect declaration exported by a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDecl {
    /// Effect name (e.g., "io", "net", "Db")
    pub name: String,
    /// Effect methods
    pub methods: Vec<EffectMethod>,
    /// Whether this is a built-in effect
    pub builtin: bool,
}

/// A method within an effect declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectMethod {
    /// Method name
    pub name: String,
    /// Parameter types (simplified representation)
    pub params: Vec<String>,
    /// Return type (simplified representation)
    pub return_type: String,
}

impl EffectDecl {
    pub fn new(name: impl Into<String>) -> Self {
        EffectDecl {
            name: name.into(),
            methods: Vec::new(),
            builtin: false,
        }
    }

    pub fn builtin(name: impl Into<String>) -> Self {
        EffectDecl {
            name: name.into(),
            methods: Vec::new(),
            builtin: true,
        }
    }

    pub fn with_method(mut self, method: EffectMethod) -> Self {
        self.methods.push(method);
        self
    }
}

impl EffectMethod {
    pub fn new(
        name: impl Into<String>,
        params: Vec<String>,
        return_type: impl Into<String>,
    ) -> Self {
        EffectMethod {
            name: name.into(),
            params,
            return_type: return_type.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_declaration() {
        let effect = EffectDecl::new("Db")
            .with_method(EffectMethod::new(
                "query",
                vec!["&s".to_string()],
                "R[Rows, DbError]".to_string(),
            ))
            .with_method(EffectMethod::new(
                "execute",
                vec!["&s".to_string()],
                "R[u64, DbError]".to_string(),
            ));
        assert_eq!(effect.name, "Db");
        assert_eq!(effect.methods.len(), 2);
        assert!(!effect.builtin);
    }

    #[test]
    fn test_builtin_effect() {
        let io = EffectDecl::builtin("io");
        assert!(io.builtin);
        assert_eq!(io.name, "io");
    }
}
