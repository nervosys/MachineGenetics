// Standard pre-norm transformer block in Framewerx-MG.
// Composes existing RMI ops: LayerNorm, Attention, Linear, GELU.

net TransformerBlock {
    layer n1: LayerNorm(512);
    layer attn: Attention(512, 8);
    layer n2: LayerNorm(512);
    layer ffn1: Linear(512, 2048);
    layer ffn_act: GELU;
    layer ffn2: Linear(2048, 512);
    forward { ffn2(ffn_act(ffn1(n2(attn(n1))))) }
}
