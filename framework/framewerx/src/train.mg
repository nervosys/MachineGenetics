// framewerx::train — TrainState and training loop
//
// Mirrors FLAX's TrainState: bundles model parameters, optimizer
// state, and step counter into one struct that the agent passes
// through update steps. Computation actually happens via the RMI
// `train` block which the bridge already routes to autograd +
// CpuBackend.

S TrainState {
    step: u64,
    params: ParamStore,
    optim: OptimState,
}

I TrainState {
    +f new(params: ParamStore, optim: OptimState) -> TrainState {
        @TrainState { step: 0, params: params, optim: optim }
    }
}

// Standard supervised-learning step: forward, loss, backward, update.
// Lowers to a `train` block via the RMIL bridge.
//
//   train classifier {
//       loss: cross_entropy;
//       optim: Adam(0.001);
//       epochs: 10;
//       batch: 64;
//   }
