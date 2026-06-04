// framewerx::specs — agent-verifiable contracts over framework modules.
//
// Each spec block declares what an implementation MUST satisfy. The
// verify/contracts and verify/module RAP methods read these and check
// every implementation in scope.
//
// Together with the ontology entries, specs are the "reliability via
// the type system" piece - an agent that writes a module satisfying
// the corresponding spec is guaranteed to compose correctly with the
// rest of the framework.

// Module: a forward pass takes a Tensor and produces a Tensor with no
// side effects beyond parameter reads.
spec ModuleForward {
    @fx();
    @ens(|result| result.shape.len() > 0);
}

// Optimizer step: must update params and return a new step counter
// that strictly increases. No I/O or network effects allowed.
spec OptimStep {
    @fx();
    @ens(|result| result.step > 0);
}

// Loss: takes (pred, target) and returns a non-negative scalar. Pure
// computation - no logging or persistence allowed inside the loss fn.
spec LossEvaluation {
    @fx();
    @req(pred.shape == target.shape);
    @ens(|result| result >= 0.0);
}

// Hybrid prediction: the verified flag MUST be set when the kb.check
// pass succeeded; rationale is a non-empty diagnostic when verified
// is false. This is the load-bearing reliability contract.
spec HybridVerification {
    @fx();
    @ens(|result| result.verified || result.rationale.len() > 0);
}

// Training loop: a step must increase step counter by exactly 1.
// Allows the io effect for logging metrics, nothing else.
spec TrainStep {
    @fx(io);
    @ens(|result| result.step == old(state.step) + 1);
}
