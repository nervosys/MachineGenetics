//! Interned symbols — O(1) comparison, 4-byte identity.
//!
//! [`Sym`] is a `u32` handle into a [`SymbolTable`]. Comparing two symbols
//! is a single integer comparison. Encoding a symbol costs 4 bytes.
//! This replaces string-based identifiers everywhere in RMIL.

use std::collections::HashMap;

/// An interned symbol — a u32 index into a [`SymbolTable`].
///
/// Cost: 4 bytes, O(1) equality, O(1) hash.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sym(pub u32);

impl Sym {
    /// The nil symbol (index 0, always maps to `""`).
    pub const NIL: Sym = Sym(0);
}

/// Bidirectional string↔u32 interning table.
///
/// Once a string is interned, it gets a stable [`Sym`] index that never
/// changes for the lifetime of the table. Thread-local by design — each
/// agent owns its own table; symbols cross agent boundaries via the
/// binary codec which re-interns on decode.
pub struct SymbolTable {
    to_id: HashMap<String, Sym>,
    to_str: Vec<String>,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    /// Create a new table. Slot 0 is always the nil symbol.
    pub fn new() -> Self {
        let mut t = Self {
            to_id: HashMap::new(),
            to_str: Vec::new(),
        };
        t.intern(""); // slot 0 = nil
        t
    }

    /// Intern a string, returning its stable index.
    /// If already interned, returns the existing index in O(1).
    pub fn intern(&mut self, s: &str) -> Sym {
        if let Some(&id) = self.to_id.get(s) {
            return id;
        }
        let id = Sym(self.to_str.len() as u32);
        self.to_str.push(s.to_owned());
        self.to_id.insert(s.to_owned(), id);
        id
    }

    /// Resolve a symbol back to its string. Panics on invalid index.
    pub fn resolve(&self, s: Sym) -> &str {
        &self.to_str[s.0 as usize]
    }

    /// Try to resolve; returns `None` on invalid index.
    pub fn try_resolve(&self, s: Sym) -> Option<&str> {
        self.to_str.get(s.0 as usize).map(|s| s.as_str())
    }

    /// Number of interned symbols.
    pub fn len(&self) -> usize {
        self.to_str.len()
    }

    /// Always false after construction (nil is always present).
    pub fn is_empty(&self) -> bool {
        self.to_str.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_dedup() {
        let mut t = SymbolTable::new();
        let a = t.intern("matmul");
        let b = t.intern("linear");
        let a2 = t.intern("matmul");
        assert_eq!(a, a2);
        assert_ne!(a, b);
        assert_eq!(t.resolve(a), "matmul");
        assert_eq!(t.resolve(b), "linear");
        assert_eq!(t.len(), 3); // nil + 2
    }

    #[test]
    fn nil_slot() {
        let t = SymbolTable::new();
        assert_eq!(t.resolve(Sym::NIL), "");
    }

    #[test]
    fn sym_size() {
        assert_eq!(std::mem::size_of::<Sym>(), 4);
    }
}
