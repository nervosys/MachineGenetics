//! Symbol Embedding
//!
//! Converts symbolic representations (terms, predicates, formulas)
//! into continuous vector spaces for neural processing.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::symbolic::logic::{Term, Predicate, Formula, Clause};

/// Configuration for embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Dimension of embeddings
    pub embedding_dim: usize,
    
    /// Maximum vocabulary size
    pub max_vocab: usize,
    
    /// Whether to use positional encodings for structure
    pub use_positional: bool,
    
    /// Aggregation method for composite terms
    pub aggregation: AggregationMethod,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 128,
            max_vocab: 10000,
            use_positional: true,
            aggregation: AggregationMethod::Mean,
        }
    }
}

/// Methods for aggregating multiple embeddings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationMethod {
    /// Average embeddings
    Mean,
    
    /// Sum embeddings
    Sum,
    
    /// Max pooling
    Max,
    
    /// Concatenate and project
    Concat,
    
    /// Self-attention aggregation
    Attention,
}

/// Embeds symbolic structures into vector spaces
pub struct SymbolEmbedder {
    /// Configuration
    config: EmbeddingConfig,
    
    /// Symbol to index mapping
    symbol_to_idx: HashMap<String, usize>,
    
    /// Embedding matrix [vocab_size, embedding_dim]
    embeddings: Vec<f32>,
    
    /// Next available index
    next_idx: usize,
}

impl SymbolEmbedder {
    /// Create a new symbol embedder
    pub fn new(config: EmbeddingConfig) -> Self {
        Self {
            embeddings: vec![0.0; config.max_vocab * config.embedding_dim],
            symbol_to_idx: HashMap::new(),
            next_idx: 0,
            config,
        }
    }
    
    /// Create with default configuration
    pub fn default_embedder() -> Self {
        Self::new(EmbeddingConfig::default())
    }
    
    /// Get or create an index for a symbol
    fn get_or_create_idx(&mut self, symbol: &str) -> usize {
        if let Some(&idx) = self.symbol_to_idx.get(symbol) {
            return idx;
        }
        
        if self.next_idx >= self.config.max_vocab {
            // Use hash for overflow
            let hash = symbol.bytes().fold(0usize, |acc, b| acc.wrapping_add(b as usize));
            return hash % self.config.max_vocab;
        }
        
        let idx = self.next_idx;
        self.symbol_to_idx.insert(symbol.to_string(), idx);
        
        // Initialize with random embedding (deterministic based on symbol)
        self.initialize_embedding(idx, symbol);
        
        self.next_idx += 1;
        idx
    }
    
    /// Initialize embedding for a symbol
    fn initialize_embedding(&mut self, idx: usize, symbol: &str) {
        let dim = self.config.embedding_dim;
        let offset = idx * dim;
        
        // Deterministic "random" initialization based on symbol hash
        let mut seed: u64 = symbol.bytes().fold(42u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });
        
        let scale = (1.0 / dim as f32).sqrt();
        
        for i in 0..dim {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u = (seed >> 33) as f32 / (1u64 << 31) as f32;
            self.embeddings[offset + i] = (u * 2.0 - 1.0) * scale;
        }
    }
    
    /// Get the embedding for a symbol
    pub fn get_embedding(&mut self, symbol: &str) -> Vec<f32> {
        let idx = self.get_or_create_idx(symbol);
        let dim = self.config.embedding_dim;
        let offset = idx * dim;
        self.embeddings[offset..offset + dim].to_vec()
    }
    
    /// Embed a term
    pub fn embed_term(&mut self, term: &Term) -> Vec<f32> {
        match term {
            Term::Variable(name) => {
                // Variables get a special prefix
                let symbol = format!("VAR:{}", name);
                self.get_embedding(&symbol)
            }
            
            Term::Constant(name) => {
                let symbol = format!("CONST:{}", name);
                self.get_embedding(&symbol)
            }
            
            Term::Integer(n) => {
                // Encode integers with position-based embedding
                let symbol = format!("INT:{}", n);
                self.get_embedding(&symbol)
            }
            
            Term::Float(bits) => {
                let f = f64::from_bits(*bits);
                let symbol = format!("FLOAT:{:.6}", f);
                self.get_embedding(&symbol)
            }
            
            Term::Function { name, args } => {
                // Embed function symbol
                let func_symbol = format!("FUNC:{}", name);
                let mut func_emb = self.get_embedding(&func_symbol);
                
                // Embed and aggregate arguments
                if !args.is_empty() {
                    let arg_embeddings: Vec<Vec<f32>> = args.iter()
                        .map(|a| self.embed_term(a))
                        .collect();
                    
                    let args_agg = self.aggregate(&arg_embeddings);
                    
                    // Combine function and arguments
                    for (i, v) in args_agg.iter().enumerate() {
                        func_emb[i] = (func_emb[i] + v) / 2.0;
                    }
                }
                
                func_emb
            }
            
            Term::List(terms) => {
                if terms.is_empty() {
                    self.get_embedding("LIST:EMPTY")
                } else {
                    let embeddings: Vec<Vec<f32>> = terms.iter()
                        .map(|t| self.embed_term(t))
                        .collect();
                    self.aggregate(&embeddings)
                }
            }
            
            Term::Cons(head, tail) => {
                let head_emb = self.embed_term(head);
                let tail_emb = self.embed_term(tail);
                
                let cons_emb = self.get_embedding("CONS");
                
                // Combine: cons + head + tail
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = (cons_emb[i] + head_emb[i] + tail_emb[i]) / 3.0;
                }
                result
            }
        }
    }
    
    /// Embed a predicate
    pub fn embed_predicate(&mut self, pred: &Predicate) -> Vec<f32> {
        let pred_symbol = format!("PRED:{}", pred.name);
        let mut pred_emb = self.get_embedding(&pred_symbol);
        
        if !pred.args.is_empty() {
            let arg_embeddings: Vec<Vec<f32>> = pred.args.iter()
                .enumerate()
                .map(|(i, a)| {
                    let mut emb = self.embed_term(a);
                    if self.config.use_positional {
                        self.add_positional(&mut emb, i);
                    }
                    emb
                })
                .collect();
            
            let args_agg = self.aggregate(&arg_embeddings);
            
            // Combine predicate and arguments
            for (i, v) in args_agg.iter().enumerate() {
                pred_emb[i] = (pred_emb[i] + v) / 2.0;
            }
        }
        
        pred_emb
    }
    
    /// Embed a formula
    pub fn embed_formula(&mut self, formula: &Formula) -> Vec<f32> {
        match formula {
            Formula::Atom(pred) => self.embed_predicate(pred),
            
            Formula::Not(inner) => {
                let inner_emb = self.embed_formula(inner);
                let not_emb = self.get_embedding("OP:NOT");
                
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = not_emb[i] - inner_emb[i]; // Negation as difference
                }
                result
            }
            
            Formula::And(formulas) => {
                let op_emb = self.get_embedding("OP:AND");
                let embeddings: Vec<Vec<f32>> = formulas.iter()
                    .map(|f| self.embed_formula(f))
                    .collect();
                let agg = self.aggregate(&embeddings);
                
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = (op_emb[i] + agg[i]) / 2.0;
                }
                result
            }
            
            Formula::Or(formulas) => {
                let op_emb = self.get_embedding("OP:OR");
                let embeddings: Vec<Vec<f32>> = formulas.iter()
                    .map(|f| self.embed_formula(f))
                    .collect();
                let agg = self.aggregate(&embeddings);
                
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = (op_emb[i] + agg[i]) / 2.0;
                }
                result
            }
            
            Formula::Implies(a, b) => {
                let op_emb = self.get_embedding("OP:IMPLIES");
                let a_emb = self.embed_formula(a);
                let b_emb = self.embed_formula(b);
                
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = (op_emb[i] + a_emb[i] + b_emb[i]) / 3.0;
                }
                result
            }
            
            Formula::ForAll { variables, formula } => {
                let op_emb = self.get_embedding("OP:FORALL");
                let inner_emb = self.embed_formula(formula);
                
                // Embed bound variables
                let var_embeddings: Vec<Vec<f32>> = variables.iter()
                    .map(|v| self.get_embedding(&format!("BOUND:{}", v)))
                    .collect();
                let vars_agg = if var_embeddings.is_empty() {
                    vec![0.0; self.config.embedding_dim]
                } else {
                    self.aggregate(&var_embeddings)
                };
                
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = (op_emb[i] + inner_emb[i] + vars_agg[i]) / 3.0;
                }
                result
            }
            
            Formula::Exists { variables, formula } => {
                let op_emb = self.get_embedding("OP:EXISTS");
                let inner_emb = self.embed_formula(formula);
                
                let var_embeddings: Vec<Vec<f32>> = variables.iter()
                    .map(|v| self.get_embedding(&format!("BOUND:{}", v)))
                    .collect();
                let vars_agg = if var_embeddings.is_empty() {
                    vec![0.0; self.config.embedding_dim]
                } else {
                    self.aggregate(&var_embeddings)
                };
                
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = (op_emb[i] + inner_emb[i] + vars_agg[i]) / 3.0;
                }
                result
            }
            
            Formula::Equals(t1, t2) => {
                let op_emb = self.get_embedding("OP:EQUALS");
                let t1_emb = self.embed_term(t1);
                let t2_emb = self.embed_term(t2);
                
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = (op_emb[i] + t1_emb[i] + t2_emb[i]) / 3.0;
                }
                result
            }
            
            Formula::True => self.get_embedding("CONST:TRUE"),
            Formula::False => self.get_embedding("CONST:FALSE"),
            
            Formula::Iff(a, b) => {
                let op_emb = self.get_embedding("OP:IFF");
                let a_emb = self.embed_formula(a);
                let b_emb = self.embed_formula(b);
                
                let mut result = vec![0.0; self.config.embedding_dim];
                for i in 0..self.config.embedding_dim {
                    result[i] = (op_emb[i] + a_emb[i] + b_emb[i]) / 3.0;
                }
                result
            }
        }
    }
    
    /// Embed a clause
    pub fn embed_clause(&mut self, clause: &Clause) -> Vec<f32> {
        let mut embeddings = Vec::new();
        
        if let Some(ref head) = clause.head {
            let head_emb = self.embed_predicate(head);
            embeddings.push(head_emb);
        }
        
        for pred in &clause.body {
            let pred_emb = self.embed_predicate(pred);
            embeddings.push(pred_emb);
        }
        
        if embeddings.is_empty() {
            return vec![0.0; self.config.embedding_dim];
        }
        
        // Add clause type encoding
        let clause_type = if clause.is_fact() {
            self.get_embedding("CLAUSE:FACT")
        } else if clause.is_rule() {
            self.get_embedding("CLAUSE:RULE")
        } else {
            self.get_embedding("CLAUSE:GOAL")
        };
        embeddings.push(clause_type);
        
        self.aggregate(&embeddings)
    }
    
    /// Aggregate multiple embeddings
    fn aggregate(&self, embeddings: &[Vec<f32>]) -> Vec<f32> {
        if embeddings.is_empty() {
            return vec![0.0; self.config.embedding_dim];
        }
        
        let dim = self.config.embedding_dim;
        let n = embeddings.len() as f32;
        
        match self.config.aggregation {
            AggregationMethod::Mean => {
                let mut result = vec![0.0; dim];
                for emb in embeddings {
                    for (i, v) in emb.iter().enumerate() {
                        result[i] += v;
                    }
                }
                for v in &mut result {
                    *v /= n;
                }
                result
            }
            
            AggregationMethod::Sum => {
                let mut result = vec![0.0; dim];
                for emb in embeddings {
                    for (i, v) in emb.iter().enumerate() {
                        result[i] += v;
                    }
                }
                result
            }
            
            AggregationMethod::Max => {
                let mut result = vec![f32::NEG_INFINITY; dim];
                for emb in embeddings {
                    for (i, v) in emb.iter().enumerate() {
                        result[i] = result[i].max(*v);
                    }
                }
                result
            }
            
            AggregationMethod::Concat | AggregationMethod::Attention => {
                // Fallback to mean for now
                let mut result = vec![0.0; dim];
                for emb in embeddings {
                    for (i, v) in emb.iter().enumerate() {
                        result[i] += v;
                    }
                }
                for v in &mut result {
                    *v /= n;
                }
                result
            }
        }
    }
    
    /// Add positional encoding to an embedding
    fn add_positional(&self, emb: &mut [f32], position: usize) {
        let dim = self.config.embedding_dim;
        
        #[allow(clippy::needless_range_loop)]
        for i in 0..dim {
            let angle = position as f32 / 10000f32.powf(2.0 * (i / 2) as f32 / dim as f32);
            if i % 2 == 0 {
                emb[i] += angle.sin();
            } else {
                emb[i] += angle.cos();
            }
        }
    }
    
    /// Compute similarity between two embeddings
    pub fn similarity(emb1: &[f32], emb2: &[f32]) -> f32 {
        assert_eq!(emb1.len(), emb2.len());
        
        let dot: f32 = emb1.iter().zip(emb2.iter()).map(|(a, b)| a * b).sum();
        let norm1: f32 = emb1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm2: f32 = emb2.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm1 < 1e-10 || norm2 < 1e-10 {
            return 0.0;
        }
        
        dot / (norm1 * norm2)
    }
    
    /// Get vocabulary size
    pub fn vocab_size(&self) -> usize {
        self.next_idx
    }
}

impl Default for SymbolEmbedder {
    fn default() -> Self {
        Self::default_embedder()
    }
}

/// Convenience function to embed a term
pub fn embed_term(term: &Term, embedder: &mut SymbolEmbedder) -> Vec<f32> {
    embedder.embed_term(term)
}

/// Convenience function to embed a predicate
pub fn embed_predicate(pred: &Predicate, embedder: &mut SymbolEmbedder) -> Vec<f32> {
    embedder.embed_predicate(pred)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_term_embedding() {
        let mut embedder = SymbolEmbedder::default_embedder();
        
        let term = Term::constant("alice");
        let emb = embedder.embed_term(&term);
        
        assert_eq!(emb.len(), 128);
        
        // Same term should give same embedding
        let emb2 = embedder.embed_term(&term);
        assert_eq!(emb, emb2);
    }
    
    #[test]
    fn test_different_terms() {
        let mut embedder = SymbolEmbedder::default_embedder();
        
        let t1 = Term::constant("alice");
        let t2 = Term::constant("bob");
        
        let e1 = embedder.embed_term(&t1);
        let e2 = embedder.embed_term(&t2);
        
        // Different terms should give different embeddings
        assert_ne!(e1, e2);
    }
    
    #[test]
    fn test_function_embedding() {
        let mut embedder = SymbolEmbedder::default_embedder();
        
        let term = Term::func("parent", vec![
            Term::constant("alice"),
            Term::constant("bob"),
        ]);
        
        let emb = embedder.embed_term(&term);
        assert_eq!(emb.len(), 128);
    }
    
    #[test]
    fn test_predicate_embedding() {
        let mut embedder = SymbolEmbedder::default_embedder();
        
        let pred = Predicate::new("loves", vec![
            Term::constant("romeo"),
            Term::constant("juliet"),
        ]);
        
        let emb = embedder.embed_predicate(&pred);
        assert_eq!(emb.len(), 128);
    }
    
    #[test]
    fn test_similarity() {
        let mut embedder = SymbolEmbedder::default_embedder();
        
        let t1 = Term::constant("cat");
        let t2 = Term::constant("cat");
        let t3 = Term::constant("dog");
        
        let e1 = embedder.embed_term(&t1);
        let e2 = embedder.embed_term(&t2);
        let e3 = embedder.embed_term(&t3);
        
        // Same term should have similarity 1
        let sim12 = SymbolEmbedder::similarity(&e1, &e2);
        assert!((sim12 - 1.0).abs() < 0.001);
        
        // Different terms should have similarity < 1
        let sim13 = SymbolEmbedder::similarity(&e1, &e3);
        assert!(sim13 < 0.99);
    }
    
    #[test]
    fn test_formula_embedding() {
        let mut embedder = SymbolEmbedder::default_embedder();
        
        let formula = Formula::and(vec![
            Formula::Atom(Predicate::new("human", vec![Term::var("X")])),
            Formula::Atom(Predicate::new("mortal", vec![Term::var("X")])),
        ]);
        
        let emb = embedder.embed_formula(&formula);
        assert_eq!(emb.len(), 128);
    }

    #[test]
    fn test_vocab_size() {
        let mut embedder = SymbolEmbedder::default_embedder();
        assert_eq!(embedder.vocab_size(), 0);
        embedder.get_embedding("hello");
        assert_eq!(embedder.vocab_size(), 1);
        embedder.get_embedding("world");
        assert_eq!(embedder.vocab_size(), 2);
        // Same symbol shouldn't increase vocab
        embedder.get_embedding("hello");
        assert_eq!(embedder.vocab_size(), 2);
    }

    #[test]
    fn test_embed_integer_term() {
        let mut embedder = SymbolEmbedder::default_embedder();
        let term = Term::Integer(42);
        let emb = embedder.embed_term(&term);
        assert_eq!(emb.len(), 128); // default dim
    }

    #[test]
    fn test_embed_list_term() {
        let mut embedder = SymbolEmbedder::default_embedder();
        let list = Term::List(vec![
            Term::Constant("a".to_string()),
            Term::Constant("b".to_string()),
        ]);
        let emb = embedder.embed_term(&list);
        assert_eq!(emb.len(), 128);
    }

    #[test]
    fn test_embed_clause() {
        let mut embedder = SymbolEmbedder::default_embedder();
        let clause = Clause::fact(Predicate::new(
            "parent",
            vec![Term::Constant("alice".to_string()), Term::Constant("bob".to_string())],
        ));
        let emb = embedder.embed_clause(&clause);
        assert_eq!(emb.len(), 128);
    }

}
