// framewerx::optim::sgd — stochastic gradient descent

S SGD {
    lr: f32,
    momentum: f32,
    weight_decay: f32,
}

I SGD {
    +f new(lr: f32) -> SGD {
        @SGD { lr: lr, momentum: 0.0, weight_decay: 0.0 }
    }

    +f with_momentum(lr: f32, momentum: f32) -> SGD {
        @SGD { lr: lr, momentum: momentum, weight_decay: 0.0 }
    }
}
