// Neurosymbolic QA: neural retrieval validated against a symbolic KB.
// Demonstrates the Hybrid composition pattern that's the reliability
// piece of Framewerx-MG.

// Knowledge base: declared facts and one rule. Lowers via the SKB
// adapter to RMI Concept entries under namespace air.skb.FactBase.
kb FactBase {
    fact entity(rust);
    fact entity(MechGen);
    fact entity(framewerx);
    rule known(x: i32) { entity(x) }
}

// Neural side: simple embedding-then-classify pipeline. Lowers to
// RMI Embed + Linear opcodes.
net Retriever {
    layer embed: Embed(1024, 64);
    layer head: Linear(64, 4);
    forward { head(embed) }
}
