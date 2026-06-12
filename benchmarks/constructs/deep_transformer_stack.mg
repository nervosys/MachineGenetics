net DeepT {
    stack 12 {
        layer attn: MultiHeadAttention(256, 8);
        layer norm1: LayerNorm;
        layer ff1: Linear(256, 1024);
        layer act: GELU;
        layer ff2: Linear(1024, 256);
        layer norm2: LayerNorm;
    }
    forward { attn_0 }
}
