// Phase-5 real MLP — runs end-to-end through CpuBackend with weighted
// Linear ops and cached parameters.
//
// Run: MechGen-parse --target=rmil-compute prototype/examples/real_mlp.mg

net MLP {
    layer fc1: Linear(8, 16);
    layer act1: ReLU;
    layer fc2: Linear(16, 4);
    layer act2: Sigmoid;
    forward { fc1 }
}
