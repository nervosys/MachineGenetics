// Agent-built (P83 self-test) using a P77-catalog layer that didn't
// exist before P78. Proves the bridge mapping FlashAttention -> ATTN
// flows end-to-end without me knowing internal implementation details.

net AgentFlashBlock {
    layer n1: LayerNorm(64);
    layer attn: FlashAttention(64, 4);
    layer n2: LayerNorm(64);
    layer ffn: Linear(64, 128);
    layer act: GELU;
    layer out: Linear(128, 64);
    forward { out(act(ffn(n2(attn(n1))))) }
}
