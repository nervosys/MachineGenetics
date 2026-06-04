// Vision Transformer. Patch embedding -> N encoder blocks -> CLS head.
// Encoder block follows the pre-norm transformer_block.mg recipe.

net ViTClassifier {
    layer patch_embed: Linear(768, 768);
    layer attn1: Attention(768, 12);
    layer n1: LayerNorm(768);
    layer ffn1: Linear(768, 3072);
    layer act1: GELU;
    layer ffn2: Linear(3072, 768);
    layer n2: LayerNorm(768);
    layer head: Linear(768, 1000);
    forward { head(n2(ffn2(act1(ffn1(n1(attn1(patch_embed))))))) }
}
