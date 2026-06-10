// framewerx::layers::attention — multi-head attention
//
// Maps to RMI opcode 0x0004 (ATTN). The CpuBackend has a full
// forward+backward implementation; agents compose it into transformer
// blocks here.

S Attention {
    dim: usize,
    heads: usize,
    dropout: f32,
}

I Attention {
    +f new(dim: usize, heads: usize) -> Attention {
        @Attention { dim: dim, heads: heads, dropout: 0.0 }
    }
}

// Standard transformer block: norm -> attn -> residual -> norm -> ffn -> residual.
// Declared as a net so the bridge lowers the whole composition to
// Agentic Binary Language in one pass.
//
//   net TransformerBlock {
//       layer n1: LayerNorm(dim);
//       layer attn: Attention(dim, heads);
//       layer n2: LayerNorm(dim);
//       layer ffn1: Linear(dim, dim * 4);
//       layer ffn_act: GELU;
//       layer ffn2: Linear(dim * 4, dim);
//       forward { ffn2(ffn_act(ffn1(n2(attn(n1))))) }
//   }
