//! Neurosymbolic Reasoning Example
//!
//! Demonstrates hybrid neural-symbolic reasoning where agents combine
//! pattern recognition with logical inference.

use rmi::neurosymbolic::{
    ConstraintSolver, HybridReasoner, ReasoningMode,
    SoftConstraint, SymbolEmbedder, EmbeddingConfig,
};
use rmi::neurosymbolic::constraint::ConstraintFormula;
use rmi::neurosymbolic::hybrid::HybridConfig;
use rmi::symbolic::{InferenceEngine, KnowledgeBase, Predicate, Term};
use rmi::symbolic::inference::InferenceConfig;

fn main() {
    println!("=== RMI Neurosymbolic Reasoning ===\n");

    // Part 1: Symbolic Knowledge Base
    println!("--- Part 1: Symbolic Reasoning ---\n");

    let mut kb = KnowledgeBase::new();

    // Define facts about neural architectures
    kb.add_fact("architecture", vec![Term::constant("transformer")]);
    kb.add_fact("architecture", vec![Term::constant("cnn")]);
    kb.add_fact("architecture", vec![Term::constant("rnn")]);

    // Define properties
    kb.add_fact("has_attention", vec![Term::constant("transformer")]);
    kb.add_fact("translation_equivariant", vec![Term::constant("cnn")]);
    kb.add_fact("sequential", vec![Term::constant("rnn")]);
    kb.add_fact("sequential", vec![Term::constant("transformer")]);

    // Add rules
    kb.add_rule(
        Predicate::new("good_for_long_range", vec![Term::var("X")]),
        vec![Predicate::new("has_attention", vec![Term::var("X")])],
    );
    kb.add_rule(
        Predicate::new("good_for_images", vec![Term::var("X")]),
        vec![Predicate::new("translation_equivariant", vec![Term::var("X")])],
    );
    kb.add_rule(
        Predicate::new("processes_sequences", vec![Term::var("X")]),
        vec![Predicate::new("sequential", vec![Term::var("X")])],
    );

    let facts_count = kb.all().iter().filter(|c| c.is_fact()).count();
    let rules_count = kb.all().iter().filter(|c| c.is_rule()).count();
    println!("Knowledge base contains {} facts and {} rules", facts_count, rules_count);

    let engine = InferenceEngine::new(InferenceConfig::default());
    let derived = engine.forward_chain(&mut kb);

    println!("\nDerived facts via forward chaining:");
    for (i, fact) in derived.iter().enumerate().take(5) {
        println!("  {}. {}", i + 1, fact);
    }
    println!();

    // Part 2: Differentiable Constraints
    println!("--- Part 2: Differentiable Constraints ---\n");

    let latency_constraint = SoftConstraint::new(
        "latency",
        ConstraintFormula::LessThan(
            Box::new(ConstraintFormula::Variable("latency".to_string())),
            Box::new(ConstraintFormula::Constant(10.0)),
        ),
    );

    let memory_constraint = SoftConstraint::new(
        "memory",
        ConstraintFormula::LessThan(
            Box::new(ConstraintFormula::Variable("memory".to_string())),
            Box::new(ConstraintFormula::Constant(512.0)),
        ),
    );

    let accuracy_constraint = SoftConstraint::new(
        "accuracy",
        ConstraintFormula::GreaterThan(
            Box::new(ConstraintFormula::Variable("accuracy".to_string())),
            Box::new(ConstraintFormula::Constant(0.9)),
        ),
    );

    let mut solver = ConstraintSolver::new();
    solver.add_constraint(latency_constraint);
    solver.add_constraint(memory_constraint);
    solver.add_constraint(accuracy_constraint);

    let configs = vec![
        ("Config A", vec![("latency", 5.0), ("memory", 256.0), ("accuracy", 0.95)]),
        ("Config B", vec![("latency", 15.0), ("memory", 256.0), ("accuracy", 0.92)]),
        ("Config C", vec![("latency", 5.0), ("memory", 768.0), ("accuracy", 0.98)]),
        ("Config D", vec![("latency", 12.0), ("memory", 600.0), ("accuracy", 0.85)]),
    ];

    for (name, vars) in &configs {
        for (var, val) in vars {
            solver.set_initial(var, *val);
        }
        let violation = solver.total_violation();
        println!(
            "{}: latency={:.1}ms, memory={:.0}MB, accuracy={:.2}",
            name,
            solver.get(vars[0].0).unwrap_or(0.0),
            solver.get(vars[1].0).unwrap_or(0.0),
            solver.get(vars[2].0).unwrap_or(0.0)
        );
        println!("  Total violation: {:.3}\n", violation);
    }

    // Part 3: Hybrid Reasoning
    println!("--- Part 3: Hybrid Neurosymbolic Reasoning ---\n");

    let config = HybridConfig::default();
    let _reasoner = HybridReasoner::new(config);

    println!("Hybrid reasoner modes:");
    for mode in [ReasoningMode::Neural, ReasoningMode::Symbolic, ReasoningMode::Hybrid, ReasoningMode::Adaptive] {
        println!("  {:?}: {}", mode, match mode {
            ReasoningMode::Neural => "Pattern matching, similarity search",
            ReasoningMode::Symbolic => "Logical deduction, rule application",
            ReasoningMode::Hybrid => "Complex queries requiring both",
            ReasoningMode::Adaptive => "Unknown query types",
        });
    }
    println!();

    // Part 4: Symbol Embeddings
    println!("--- Part 4: Symbol Embeddings ---\n");

    let config = EmbeddingConfig { embedding_dim: 64, ..Default::default() };
    let mut embedder = SymbolEmbedder::new(config);

    let terms = [
        Term::constant("transformer"),
        Term::constant("attention"),
        Term::constant("convolution"),
        Term::constant("gradient"),
    ];

    println!("Embedding terms into 64-dimensional space:");
    for term in &terms {
        let embedding = embedder.embed_term(term);
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!("  {} -> [{:.3}, {:.3}, ...] (norm: {:.3})", term, embedding[0], embedding[1], norm);
    }

    println!("\nSimilarity between terms:");
    for i in 0..terms.len() {
        let emb_i = embedder.embed_term(&terms[i]);
        for j in (i + 1)..terms.len() {
            let emb_j = embedder.embed_term(&terms[j]);
            let sim = SymbolEmbedder::similarity(&emb_i, &emb_j);
            println!("  {} <-> {}: {:.3}", terms[i], terms[j], sim);
        }
    }

    println!("\n=== Demo Complete ===");
}
