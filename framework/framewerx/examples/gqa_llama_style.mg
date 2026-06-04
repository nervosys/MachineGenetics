// Llama-style decoder block: GroupedQueryAttention + RMSNorm + SwiGLU.
// Each layer name is wired in the P78 bridge mapping table.
//
// Dispatch note: a real SwiGLU computes `down(act(gate) * up)`, which
// requires elementwise multiplication that the bridge currently models
// as a Linear chain. This example uses a single FFN projection so the
// pipeline shape-composes for end-to-end dispatch testing while still
// showing the canonical Llama component naming.

net LlamaBlock {
    layer n1: RMSNorm(4096);
    layer attn: GroupedQueryAttention(4096, 32);
    layer n2: RMSNorm(4096);
    layer ffn: Linear(4096, 4096);
    layer act: SwiGLU;
    forward { act(ffn(n2(attn(n1)))) }
}
