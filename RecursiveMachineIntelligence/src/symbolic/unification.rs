//! Unification Algorithm
//!
//! Implements Robinson's unification algorithm for first-order terms,
//! enabling AI agents to perform pattern matching and variable binding.

use super::logic::Term;
use std::collections::HashMap;

/// A substitution mapping variables to terms
pub type Substitution = HashMap<String, Term>;

/// Compose two substitutions: apply s2 to s1, then combine
pub fn compose(s1: &Substitution, s2: &Substitution) -> Substitution {
    let mut result: Substitution = s1
        .iter()
        .map(|(k, v)| (k.clone(), v.apply_substitution(s2)))
        .collect();

    for (k, v) in s2 {
        if !result.contains_key(k) {
            result.insert(k.clone(), v.clone());
        }
    }

    result
}

/// Check if variable occurs in term (occurs check)
/// Prevents infinite substitutions like X = f(X)
pub fn occurs_check(var: &str, term: &Term) -> bool {
    match term {
        Term::Variable(name) => name == var,
        Term::Function { args, .. } => args.iter().any(|t| occurs_check(var, t)),
        Term::List(terms) => terms.iter().any(|t| occurs_check(var, t)),
        Term::Cons(head, tail) => occurs_check(var, head) || occurs_check(var, tail),
        _ => false,
    }
}

/// Unification error types for term matching
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnificationError {
    /// Terms have different function symbols
    SymbolMismatch {
        /// Expected symbol name
        expected: String,
        /// Actual symbol found
        found: String,
    },

    /// Terms have different arities
    ArityMismatch {
        /// Expected arity
        expected: usize,
        /// Actual arity found
        found: usize,
    },

    /// Occurs check failed (would create infinite term)
    OccursCheck {
        /// Variable that would be recursive
        variable: String,
        /// Term containing the variable
        term: String,
    },

    /// Cannot unify different constants
    ConstantMismatch {
        /// First constant
        a: String,
        /// Second constant
        b: String,
    },

    /// Cannot unify different types
    TypeMismatch,
}

impl std::fmt::Display for UnificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnificationError::SymbolMismatch { expected, found } => {
                write!(f, "Symbol mismatch: expected {}, found {}", expected, found)
            }
            UnificationError::ArityMismatch { expected, found } => {
                write!(f, "Arity mismatch: expected {}, found {}", expected, found)
            }
            UnificationError::OccursCheck { variable, term } => {
                write!(f, "Occurs check failed: {} occurs in {}", variable, term)
            }
            UnificationError::ConstantMismatch { a, b } => {
                write!(f, "Cannot unify constants: {} and {}", a, b)
            }
            UnificationError::TypeMismatch => {
                write!(f, "Type mismatch")
            }
        }
    }
}

impl std::error::Error for UnificationError {}

/// Result type for unification
pub type UnifyResult = Result<Substitution, UnificationError>;

/// Unify two terms, returning the most general unifier (MGU)
pub fn unify(t1: &Term, t2: &Term) -> UnifyResult {
    unify_with_substitution(t1, t2, Substitution::new())
}

/// Unify two terms with an existing substitution
fn unify_with_substitution(t1: &Term, t2: &Term, subst: Substitution) -> UnifyResult {
    let t1 = t1.apply_substitution(&subst);
    let t2 = t2.apply_substitution(&subst);

    match (&t1, &t2) {
        // Same terms unify trivially
        (a, b) if a == b => Ok(subst),

        // Variable bindings
        (Term::Variable(name), other) => {
            if occurs_check(name, other) {
                Err(UnificationError::OccursCheck {
                    variable: name.clone(),
                    term: format!("{}", other),
                })
            } else {
                let mut new_subst = subst.clone();
                new_subst.insert(name.clone(), other.clone());
                Ok(new_subst)
            }
        }

        (other, Term::Variable(name)) => {
            if occurs_check(name, other) {
                Err(UnificationError::OccursCheck {
                    variable: name.clone(),
                    term: format!("{}", other),
                })
            } else {
                let mut new_subst = subst.clone();
                new_subst.insert(name.clone(), other.clone());
                Ok(new_subst)
            }
        }

        // Constants
        (Term::Constant(a), Term::Constant(b)) => {
            if a == b {
                Ok(subst)
            } else {
                Err(UnificationError::ConstantMismatch {
                    a: a.clone(),
                    b: b.clone(),
                })
            }
        }

        // Integers
        (Term::Integer(a), Term::Integer(b)) => {
            if a == b {
                Ok(subst)
            } else {
                Err(UnificationError::ConstantMismatch {
                    a: a.to_string(),
                    b: b.to_string(),
                })
            }
        }

        // Floats
        (Term::Float(a), Term::Float(b)) => {
            if a == b {
                Ok(subst)
            } else {
                Err(UnificationError::ConstantMismatch {
                    a: f64::from_bits(*a).to_string(),
                    b: f64::from_bits(*b).to_string(),
                })
            }
        }

        // Function terms
        (Term::Function { name: n1, args: a1 }, Term::Function { name: n2, args: a2 }) => {
            if n1 != n2 {
                return Err(UnificationError::SymbolMismatch {
                    expected: n1.clone(),
                    found: n2.clone(),
                });
            }

            if a1.len() != a2.len() {
                return Err(UnificationError::ArityMismatch {
                    expected: a1.len(),
                    found: a2.len(),
                });
            }

            unify_args(a1, a2, subst)
        }

        // Lists
        (Term::List(l1), Term::List(l2)) => {
            if l1.len() != l2.len() {
                return Err(UnificationError::ArityMismatch {
                    expected: l1.len(),
                    found: l2.len(),
                });
            }

            unify_args(l1, l2, subst)
        }

        // Cons cells
        (Term::Cons(h1, t1), Term::Cons(h2, t2)) => {
            let subst = unify_with_substitution(h1, h2, subst)?;
            unify_with_substitution(t1, t2, subst)
        }

        // List with Cons (destructuring)
        (Term::List(items), Term::Cons(head, tail))
        | (Term::Cons(head, tail), Term::List(items)) => {
            if items.is_empty() {
                return Err(UnificationError::ArityMismatch {
                    expected: 1,
                    found: 0,
                });
            }

            let subst = unify_with_substitution(&items[0], head, subst)?;
            let rest = Term::List(items[1..].to_vec());
            unify_with_substitution(&rest, tail, subst)
        }

        // Type mismatch
        _ => Err(UnificationError::TypeMismatch),
    }
}

/// Unify lists of arguments
fn unify_args(args1: &[Term], args2: &[Term], mut subst: Substitution) -> UnifyResult {
    for (a1, a2) in args1.iter().zip(args2.iter()) {
        subst = unify_with_substitution(a1, a2, subst)?;
    }
    Ok(subst)
}

/// Check if two terms are unifiable (without computing the substitution)
pub fn unifiable(t1: &Term, t2: &Term) -> bool {
    unify(t1, t2).is_ok()
}

/// Find all unifiers for a term against a set of candidate terms
pub fn find_unifiers(query: &Term, candidates: &[Term]) -> Vec<(usize, Substitution)> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(i, t)| unify(query, t).ok().map(|s| (i, s)))
        .collect()
}

/// Specialized unification for predicates
pub mod predicate_unify {
    use super::*;
    use crate::symbolic::logic::Predicate;

    /// Unify two predicates
    pub fn unify_predicates(p1: &Predicate, p2: &Predicate) -> UnifyResult {
        if p1.name != p2.name {
            return Err(UnificationError::SymbolMismatch {
                expected: p1.name.clone(),
                found: p2.name.clone(),
            });
        }

        if p1.arity() != p2.arity() {
            return Err(UnificationError::ArityMismatch {
                expected: p1.arity(),
                found: p2.arity(),
            });
        }

        unify_args(&p1.args, &p2.args, Substitution::new())
    }

    /// Check if two predicates are unifiable
    pub fn predicates_unifiable(p1: &Predicate, p2: &Predicate) -> bool {
        unify_predicates(p1, p2).is_ok()
    }
}

/// Anti-unification (finding the least general generalization)
pub fn anti_unify(t1: &Term, t2: &Term) -> (Term, HashMap<(Term, Term), String>) {
    let mut counter = 0;
    let mut mappings = HashMap::new();

    fn anti_unify_inner(
        t1: &Term,
        t2: &Term,
        counter: &mut usize,
        mappings: &mut HashMap<(Term, Term), String>,
    ) -> Term {
        if t1 == t2 {
            return t1.clone();
        }

        match (t1, t2) {
            (Term::Function { name: n1, args: a1 }, Term::Function { name: n2, args: a2 })
                if n1 == n2 && a1.len() == a2.len() =>
            {
                let new_args: Vec<Term> = a1
                    .iter()
                    .zip(a2.iter())
                    .map(|(x, y)| anti_unify_inner(x, y, counter, mappings))
                    .collect();
                Term::Function {
                    name: n1.clone(),
                    args: new_args,
                }
            }

            (Term::List(l1), Term::List(l2)) if l1.len() == l2.len() => {
                let new_items: Vec<Term> = l1
                    .iter()
                    .zip(l2.iter())
                    .map(|(x, y)| anti_unify_inner(x, y, counter, mappings))
                    .collect();
                Term::List(new_items)
            }

            _ => {
                let key = (t1.clone(), t2.clone());
                if let Some(var_name) = mappings.get(&key) {
                    Term::Variable(var_name.clone())
                } else {
                    let var_name = format!("G{}", counter);
                    *counter += 1;
                    mappings.insert(key, var_name.clone());
                    Term::Variable(var_name)
                }
            }
        }
    }

    let result = anti_unify_inner(t1, t2, &mut counter, &mut mappings);
    (result, mappings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unify_variables() {
        let x = Term::var("X");
        let a = Term::constant("a");

        let result = unify(&x, &a).unwrap();
        assert_eq!(result.get("X"), Some(&a));
    }

    #[test]
    fn test_unify_functions() {
        let t1 = Term::func("f", vec![Term::var("X"), Term::constant("a")]);
        let t2 = Term::func("f", vec![Term::constant("b"), Term::var("Y")]);

        let result = unify(&t1, &t2).unwrap();
        assert_eq!(result.get("X"), Some(&Term::constant("b")));
        assert_eq!(result.get("Y"), Some(&Term::constant("a")));
    }

    #[test]
    fn test_unify_nested() {
        let t1 = Term::func(
            "f",
            vec![Term::func("g", vec![Term::var("X")]), Term::var("Y")],
        );
        let t2 = Term::func(
            "f",
            vec![Term::var("Z"), Term::func("h", vec![Term::constant("a")])],
        );

        let result = unify(&t1, &t2).unwrap();

        // Z should unify with g(X)
        assert!(result.contains_key("Z"));
        // Y should unify with h(a)
        assert_eq!(
            result.get("Y"),
            Some(&Term::func("h", vec![Term::constant("a")]))
        );
    }

    #[test]
    fn test_unify_symbol_mismatch() {
        let t1 = Term::func("f", vec![Term::constant("a")]);
        let t2 = Term::func("g", vec![Term::constant("a")]);

        let result = unify(&t1, &t2);
        assert!(matches!(
            result,
            Err(UnificationError::SymbolMismatch { .. })
        ));
    }

    #[test]
    fn test_occurs_check() {
        let x = Term::var("X");
        let f_x = Term::func("f", vec![Term::var("X")]);

        // X = f(X) should fail due to occurs check
        let result = unify(&x, &f_x);
        assert!(matches!(result, Err(UnificationError::OccursCheck { .. })));
    }

    #[test]
    fn test_unify_lists() {
        let l1 = Term::list(vec![Term::var("X"), Term::constant("b")]);
        let l2 = Term::list(vec![Term::constant("a"), Term::var("Y")]);

        let result = unify(&l1, &l2).unwrap();
        assert_eq!(result.get("X"), Some(&Term::constant("a")));
        assert_eq!(result.get("Y"), Some(&Term::constant("b")));
    }

    #[test]
    fn test_compose_substitutions() {
        let mut s1 = Substitution::new();
        s1.insert("X".to_string(), Term::var("Y"));

        let mut s2 = Substitution::new();
        s2.insert("Y".to_string(), Term::constant("a"));

        let composed = compose(&s1, &s2);

        // X should map to a (Y was substituted with a)
        assert_eq!(composed.get("X"), Some(&Term::constant("a")));
        // Y should map to a
        assert_eq!(composed.get("Y"), Some(&Term::constant("a")));
    }

    #[test]
    fn test_anti_unify() {
        let t1 = Term::func("f", vec![Term::constant("a"), Term::constant("b")]);
        let t2 = Term::func("f", vec![Term::constant("c"), Term::constant("b")]);

        let (lgg, _) = anti_unify(&t1, &t2);

        // LGG should be f(G0, b) where G0 is a fresh variable
        match lgg {
            Term::Function { name, args } => {
                assert_eq!(name, "f");
                assert!(args[0].is_variable());
                assert_eq!(args[1], Term::constant("b"));
            }
            _ => panic!("Expected function term"),
        }
    }

    #[test]
    fn test_predicate_unification() {
        use crate::symbolic::logic::Predicate;

        let p1 = Predicate::new("parent", vec![Term::var("X"), Term::constant("bob")]);
        let p2 = Predicate::new("parent", vec![Term::constant("alice"), Term::var("Y")]);

        let result = predicate_unify::unify_predicates(&p1, &p2).unwrap();
        assert_eq!(result.get("X"), Some(&Term::constant("alice")));
        assert_eq!(result.get("Y"), Some(&Term::constant("bob")));
    }
}
