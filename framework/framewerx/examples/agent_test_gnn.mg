// P85 self-test: graph net using GCNLayer (P77 catalog, P78 bridge).
// 16-dim node features -> 32 hidden -> 8-class node logits.

net AgentGNN {
    layer gcn1: GCNLayer(16, 32);
    layer act: SiLU;
    layer gcn2: GCNLayer(32, 8);
    forward { gcn2(act(gcn1)) }
}
