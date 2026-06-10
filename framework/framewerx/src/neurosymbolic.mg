// framewerx::neurosymbolic — bridge between net (neural) and kb (symbolic).
//
// This is the reliability-via-ontology piece. A pure neural model can
// hallucinate; a pure symbolic model can't generalise. The Hybrid
// module composes both:
//
//   1. Neural branch produces a candidate output (e.g. classification).
//   2. KB branch checks the candidate against declared rules.
//   3. If the KB rejects, the system falls back to a default OR
//      requests refinement from the agent backend.
//
// Both branches lower to Agentic Binary Language: the neural side via existing op-family
// dispatch, the symbolic side via SKB->RMI ontology adapter
// (`rmi_ontology_adapter.rs`).

S Hybrid {
    neural: @Module,
    knowledge: @KnowledgeBase,
}

// Reliability contract: every Hybrid produces an output annotated
// with its symbolic-validation result. The agent can branch on the
// `verified` flag.
S HybridOutput {
    value: Tensor,
    verified: bool,
    rationale: s,
}

I Hybrid {
    +f new(neural: @Module, knowledge: @KnowledgeBase) -> Hybrid {
        @Hybrid { neural: neural, knowledge: knowledge }
    }

    +f predict(&self, x: Tensor) -> HybridOutput {
        v candidate = self.neural.forward(x);
        v verified = self.knowledge.check(candidate);
        @HybridOutput {
            value: candidate,
            verified: verified,
            rationale: "ok",
        }
    }
}

// Example use:
//
//   kb DomainRules {
//       fact valid_class(0);
//       fact valid_class(1);
//       fact valid_class(2);
//       rule prediction_ok(c) { valid_class(c) }
//   }
//
//   net Classifier {
//       layer fc: Linear(8, 4);
//       layer head: Linear(4, 3);
//       forward { head(fc) }
//   }
//
//   v hybrid = Hybrid.new(@Classifier, @DomainRules);
//   v result = hybrid.predict(input);
//   ? result.verified { use(result.value) } : { request_refinement() }
