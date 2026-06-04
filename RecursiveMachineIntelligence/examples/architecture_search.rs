//! Architecture Search Example
//!
//! Demonstrates how AI agents can use RMI to collaboratively
//! design and evaluate neural network architectures.

use rmi::knowledge::{AIConceptsOntology, AIHistoryKB, ContributionCategory, AIEra, ConceptDomain};
use rmi::neural::architecture::ArchitectureBuilder;
use rmi::neural::primitives::ShapeSpec;
use std::collections::HashMap;

fn main() {
    println!("=== RMI Multi-Agent Architecture Search ===\n");

    // Build a simple MLP architecture using the ArchitectureBuilder
    println!("--- Building MLP Architecture ---\n");
    
    let mlp = ArchitectureBuilder::new("mlp_classifier")
        .input("input", ShapeSpec::from_concrete(&[32, 784]))
        .linear("hidden1", 256)
        .relu("relu1")
        .linear("output", 10)
        .build();
    
    println!("Architecture: {}", mlp.name);
    println!("Nodes: {}", mlp.node_count());
    println!("Edges: {}", mlp.edge_count());
    println!("Depth: {}", mlp.depth());
    
    // Estimate parameters
    let bindings: HashMap<String, usize> = HashMap::from([
        ("in_features".to_string(), 784),
        ("out_features".to_string(), 256),
    ]);
    let params = mlp.estimate_parameters(&bindings);
    println!("Estimated parameters: {}", params);
    
    // Get topological order
    if let Some(topo) = mlp.topological_order() {
        println!("Topological order: {} nodes", topo.len());
    }
    println!();

    // Build a transformer block
    println!("--- Building Transformer Block ---\n");
    
    let transformer = ArchitectureBuilder::new("transformer_block")
        .input("input", ShapeSpec::from_concrete(&[32, 128, 512]))
        .attention("attention", 8, 64)
        .layer_norm("norm1")
        .linear("ffn1", 2048)
        .gelu("gelu")
        .linear("ffn2", 512)
        .layer_norm("norm2")
        .build();
    
    println!("Architecture: {}", transformer.name);
    println!("Nodes: {}", transformer.node_count());
    println!("Edges: {}", transformer.edge_count());
    println!("Depth: {}", transformer.depth());
    
    let transformer_bindings: HashMap<String, usize> = HashMap::from([
        ("in_features".to_string(), 512),
        ("out_features".to_string(), 2048),
        ("heads".to_string(), 8),
        ("head_dim".to_string(), 64),
        ("batch".to_string(), 32),
        ("seq".to_string(), 128),
    ]);
    println!("Estimated parameters: {}", transformer.estimate_parameters(&transformer_bindings));
    println!("Estimated FLOPs: {}", transformer.estimate_flops(&transformer_bindings));
    println!();

    println!("=== AI History Knowledge Demo ===\n");

    // Query historical knowledge
    let history = AIHistoryKB::new();
    
    // Get papers in chronological order
    let chrono = history.chronological();
    println!("Total contributions in KB: {}", chrono.len());
    println!();

    // Find attention-related contributions
    let attention_papers = history.search_concept("attention");
    println!("Papers related to ''attention'' ({}):", attention_papers.len());
    for paper in attention_papers.iter().take(3) {
        println!(
            "  - {} ({}) by {:?}",
            paper.title, paper.year, paper.authors
        );
    }
    println!();
    
    // Find architecture papers
    let arch_papers = history.by_category(ContributionCategory::Architecture);
    println!("Architecture papers ({}):", arch_papers.len());
    for paper in arch_papers.iter().take(3) {
        println!(
            "  - {} ({})",
            paper.title, paper.year
        );
    }
    println!();
    
    // Get papers by era
    let deep_learning_era = history.by_era(AIEra::DeepLearningRevolution);
    println!("Deep Learning Revolution era papers: {}", deep_learning_era.len());
    
    if !chrono.is_empty() {
        println!("Earliest paper: {} ({})", chrono[0].title, chrono[0].year);
        println!("Latest paper: {} ({})", chrono.last().unwrap().title, chrono.last().unwrap().year);
    }

    println!("\n=== Concept Ontology Demo ===\n");

    // Query concept relationships
    let ontology = AIConceptsOntology::new();
    
    // Get concepts by domain
    let neural_concepts = ontology.by_domain(ConceptDomain::Neural);
    println!("Neural domain concepts: {}", neural_concepts.len());
    
    // Get transformer concept
    if let Some(transformer_concept) = ontology.get_by_name("transformer") {
        println!("\nConcept: {}", transformer_concept.name);
        println!("Domain: {:?}", transformer_concept.domain);
        println!("Definition: {}", transformer_concept.definition);
        if let Some(ref complexity) = transformer_concept.complexity {
            println!("Time Complexity: {}", complexity.time);
            println!("Space Complexity: {}", complexity.space);
        }
    }
    println!();
    
    // Get concepts for specific tasks
    let nlp_concepts = ontology.for_task("natural_language_processing");
    println!("NLP task concepts: {}", nlp_concepts.len());

    println!("\n=== Demo Complete ===");
}
