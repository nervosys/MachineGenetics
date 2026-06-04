// framewerx::neurosymbolic::verification — provable safety / soundness.
//
// Output-verifying layers that catch unsafe / hallucinated predictions
// before they propagate. Complements src/neurosymbolic.mg (Hybrid).

// Neural-network verifier (sound over-approximation).
S NeuralVerifier {
    method: s,
    epsilon: f32,
}

I NeuralVerifier {
    +f crown() -> NeuralVerifier { @NeuralVerifier { method: "CROWN", epsilon: 0.01 } }
    +f ibp() -> NeuralVerifier { @NeuralVerifier { method: "IBP", epsilon: 0.01 } }
    +f marabou() -> NeuralVerifier { @NeuralVerifier { method: "Marabou", epsilon: 0.001 } }
}

// Certified robustness via randomized smoothing.
S RandomizedSmoothing { sigma: f32, num_samples: usize, confidence: f32 }

// Constraint layer: project the network output into a feasible polytope.
S ConstraintProjection {
    num_constraints: usize,
    projection_method: s,
}

// Conformal prediction wrapper.
S ConformalPredictor {
    alpha: f32,
    calibration_size: usize,
    score_function: s,
}

I ConformalPredictor {
    +f new(alpha: f32) -> ConformalPredictor {
        @ConformalPredictor {
            alpha: alpha,
            calibration_size: 1000,
            score_function: "softmax_residual",
        }
    }
}

// Abstention / refusal: confidence-thresholded gate that returns
// "I do not know" when neural confidence is too low for symbolic guard.
S AbstentionGate {
    threshold: f32,
    on_refuse: s,
}

I AbstentionGate {
    +f new(threshold: f32) -> AbstentionGate {
        @AbstentionGate { threshold: threshold, on_refuse: "abstain" }
    }
}

// Constitutional guard: pre-trained rule set that vetoes outputs.
S ConstitutionalGuard {
    rules: [s]~,
    veto_strength: f32,
}
