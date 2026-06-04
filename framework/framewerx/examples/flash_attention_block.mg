// FlashAttention-based transformer block. Exercises the P78 bridge
// mapping: FlashAttention -> ATTN with the same semantics as the basic
// Attention layer but the backend can specialise on the variant tag.

net FlashAttnBlock {
    layer n1: LayerNorm(512);
    layer attn: FlashAttention(512, 8);
    layer n2: LayerNorm(512);
    layer ffn1: Linear(512, 2048);
    layer act: GELU;
    layer ffn2: Linear(2048, 512);
    forward { ffn2(act(ffn1(n2(attn(n1))))) }
}
