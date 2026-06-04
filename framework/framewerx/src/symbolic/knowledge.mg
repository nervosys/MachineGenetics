// framewerx::symbolic::knowledge — KB / KG / triple stores / SPARQL-like.

// Knowledge Graph: subject-predicate-object triples.
S KnowledgeGraph { num_entities: usize, num_relations: usize, num_triples: usize }

// Triple store with index by S, P, O.
S TripleStore { backend: s, indexes: [s]~ }
I TripleStore {
    +f new(backend: s) -> TripleStore {
        @TripleStore { backend: backend, indexes: ["SPO", "POS", "OSP"] }
    }
}

// SPARQL-like query engine.
S SPARQLEngine { features: [s]~ }

// Knowledge graph embedding models (TransE, DistMult, RotatE, ComplEx).
S TransE { dim: usize, margin: f32 }
S DistMult { dim: usize }
S RotatE { dim: usize }
S ComplEx { dim: usize }

// Reasoning rules in N3 / OWL.
S N3Reasoner { rule_set: s }
S OWLReasoner { profile: s }

I OWLReasoner {
    +f el() -> OWLReasoner { @OWLReasoner { profile: "EL" } }
    +f rl() -> OWLReasoner { @OWLReasoner { profile: "RL" } }
    +f ql() -> OWLReasoner { @OWLReasoner { profile: "QL" } }
}

// Conceptual schema: frames and semantic networks.
S Frame { name: s, slots: [s]~, parents: [s]~ }
S SemanticNetwork { nodes: [s]~, edges: [s]~ }

// Ontology alignment / matching.
S OntologyMatcher { strategy: s, threshold: f32 }
