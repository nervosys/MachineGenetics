// framewerx::module — base Module abstractions
//
// A `Module` is anything that has a `forward` mapping a tensor to a
// tensor. Framewerx-MG modules are declared with the `net` keyword
// (which the parser already routes to Agentic Binary Language); this file declares the
// composition glue (Sequential, Residual, Branch) that wraps net
// definitions into reusable shapes.

T Module {
    f forward(&self, x: Tensor) -> Tensor;
}

// Sequential composition: apply a list of modules left-to-right.
// `forward { layers[0..N] }` semantics: agent declares an ordered list,
// each layer's output feeds the next. Lowers to a Control::Seq op
// chain through the RMI bridge.
S Sequential {
    layers: [@Module]~,
}

// Residual: y = x + f(x). Common shape in transformers; declared as
// a struct so the ontology can advertise it as a composition pattern.
S Residual {
    inner: @Module,
}

// Parallel branch: apply N modules to the same input, return a tuple.
// Used in multi-head attention and mixture-of-experts.
S Branch {
    branches: [@Module]~,
}

// Empty marker types for the layer surface names that the RMI bridge
// already understands. Listed here so agents can `use framewerx.{...}`
// and have the names resolved.
S Linear { in_features: usize, out_features: usize }
S Conv2D { in_channels: usize, out_channels: usize, kernel: usize }
S Attention { dim: usize, heads: usize }
S Embed { vocab: usize, dim: usize }
S Dropout { rate: f32 }
S Softmax { axis: i32 }
S ReLU {}
S GELU {}
S SiLU {}
S Sigmoid {}
S Tanh {}
S LayerNorm { dim: usize }
S RMSNorm { dim: usize }
S BatchNorm { dim: usize }
S MaxPool { kernel: usize }
S AvgPool { kernel: usize }
