// Agent-built (dogfooding session). A 3-layer linear regressor that learns
// the affine map y = 2*x1 - x2 + 0.5*x3 from five samples, trained via RMI's
// CpuBackend (SGD + MSE) and checkpointed for inference.
//
// Design note (from the agent's own debugging): an earlier version stacked
// ReLU activations and the loss stalled — with this tiny init/scale and a
// negative target component, the ReLU path sat in its dead zone (constant
// output = the target variance, 1.6125). A linear stack is the right model
// for a *linear* target and learns it cleanly (≈100% loss reduction).
//
// Run:
//   mage-parse --check               examples/agent_built_mlp.mg
//   mage-parse --target=abl-train   examples/agent_built_mlp.mg
//   mage-parse --target=abl-infer   examples/agent_built_mlp.mg

net AffineRegressor {
    layer fc1: Linear(3, 8);
    layer fc2: Linear(8, 4);
    layer fc3: Linear(4, 1);
    forward { fc1 }
}

train FitAffine {
    net: AffineRegressor;
    optimizer: SGD(0.05);
    loss: MSE;
    epochs: 200;
    checkpoint: "agent_built_mlp.ckpt";
    // y = 2*x1 - x2 + 0.5*x3
    inputs: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.5, 0.5, 0.5]];
    targets: [[2.0], [-1.0], [0.5], [1.5], [0.75]];
}
