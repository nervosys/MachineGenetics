// framewerx::layers::linear — affine transform y = xW^T + b
//
// Maps to Agentic Binary Language opcode 0x0002 (LINEAR) via the bridge. Agent declares
// it inside a `net` block; the framework provides this file purely as
// documentation + ontology hooks. The actual compute lives in
// rmi/compute/cpu.

// Example net using Linear:
//
//   net Affine {
//       layer fc: Linear(8, 4);
//       forward { fc }
//   }
//
// The `(in_features, out_features)` arguments lower to constructor
// args of the LINEAR opcode. Bias is enabled by default; pass three
// args `Linear(in, out, 0)` to disable.

S LinearSpec {
    in_features: usize,
    out_features: usize,
    bias: bool,
}

I LinearSpec {
    +f new(in_features: usize, out_features: usize) -> LinearSpec {
        @LinearSpec { in_features: in_features, out_features: out_features, bias: 1b }
    }
    +f no_bias(in_features: usize, out_features: usize) -> LinearSpec {
        @LinearSpec { in_features: in_features, out_features: out_features, bias: 0b }
    }
}
