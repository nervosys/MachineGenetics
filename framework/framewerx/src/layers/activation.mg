// framewerx::layers::activation — elementwise nonlinearities
//
// Each maps directly to an RMI Neural-family opcode:
//   ReLU    -> 0x0010    GELU    -> 0x0011
//   SiLU    -> 0x0012    Sigmoid -> 0x0013
//   Tanh    -> 0x0014    Mish    -> 0x0015
//   Softplus-> 0x0016    Softmax -> 0x0007
//
// Agent declares them as layer-only entries inside a net block:
//
//   net MLP {
//       layer fc1: Linear(8, 4);
//       layer act: ReLU;
//       forward { act(fc1) }
//   }
//
// No constructor arguments — all activations are parameter-free.

// Marker types for the ontology to advertise. Empty struct bodies
// match how the parser models parameterless layer kinds.
S ReLU {}
S GELU {}
S SiLU {}
S Sigmoid {}
S Tanh {}
S Mish {}
S Softplus {}

// Softmax does have an axis argument (default = -1, the last dim).
S Softmax { axis: i32 }

// SELU (Scaled Exponential Linear Unit): self-normalising activation
// from the SNN paper. Lambda = 1.0507, alpha = 1.67326 hardcoded.
S SELU {}

// ELU (Exponential Linear Unit). alpha = 1.0 default.
S ELU { alpha: f32 }

I ELU {
    +f new() -> ELU { @ELU { alpha: 1.0 } }
}

// Leaky ReLU: negative slope leaks instead of zeroing.
S LeakyReLU { negative_slope: f32 }

I LeakyReLU {
    +f new() -> LeakyReLU { @LeakyReLU { negative_slope: 0.01 } }
}

// Hard versions used in mobile/edge models (cheaper to compute).
S HardSwish {}
S HardSigmoid {}

// Swish/GLU variants common in modern LLMs.
S Swish {}
S SwiGLU {}
S GeGLU {}

