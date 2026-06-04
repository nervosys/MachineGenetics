// framewerx::layers::norm — normalisation layers
//
// Maps to RMI Normalisation opcodes 0x0020+:
//   LayerNorm     -> 0x0020
//   BatchNorm     -> 0x0021
//   RMSNorm       -> 0x0022
//   GroupNorm     -> 0x0023
//   InstanceNorm  -> 0x0024

S LayerNorm { dim: usize, eps: f32 }
S RMSNorm { dim: usize, eps: f32 }
S BatchNorm { dim: usize, momentum: f32 }
S GroupNorm { num_groups: usize, num_channels: usize }
S InstanceNorm { dim: usize }

I LayerNorm {
    +f new(dim: usize) -> LayerNorm {
        @LayerNorm { dim: dim, eps: 0.00001 }
    }
}

I RMSNorm {
    +f new(dim: usize) -> RMSNorm {
        @RMSNorm { dim: dim, eps: 0.00001 }
    }
}
