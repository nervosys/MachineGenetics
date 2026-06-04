// framewerx::optim::adam — Adam optimizer
//
// Maps to RMI optimizer constructor (rmi::compute::OptimState::adam).
// Agents declare the optimizer alongside the model and the training
// loop wires them via the TrainState struct.

S Adam {
    lr: f32,
    beta1: f32,
    beta2: f32,
    eps: f32,
}

I Adam {
    +f new(lr: f32) -> Adam {
        @Adam { lr: lr, beta1: 0.9, beta2: 0.999, eps: 0.00000001 }
    }

    +f with_betas(lr: f32, beta1: f32, beta2: f32) -> Adam {
        @Adam { lr: lr, beta1: beta1, beta2: beta2, eps: 0.00000001 }
    }
}
