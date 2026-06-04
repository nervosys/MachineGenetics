// Mamba-style language model. Embedding -> stack of selective SSM
// blocks -> RMS norm -> LM head. The bridge currently lowers each
// MambaBlock to a composition of Linear ops; a dedicated SSM opcode
// is a future addition.

net MambaLM {
    layer embed: Embed(50000, 768);
    layer block1: Linear(768, 768);
    layer block2: Linear(768, 768);
    layer block3: Linear(768, 768);
    layer norm: RMSNorm(768);
    layer head: Linear(768, 50000);
    forward { head(norm(block3(block2(block1(embed))))) }
}
