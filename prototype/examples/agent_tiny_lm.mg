// Agent-built (dogfooding session): a tiny char-cycle language model, built
// on MAGE's functional path (net → Agentic Binary Language → train → generate via RMI).
//
// Distinct from the bundled tiny_lm example: vocab=6, learns the 6-cycle
// i -> (i+1) mod 6, then generates from a prompt. Embedding + Linear head,
// CrossEntropy + Adam, checkpointed.
//
//   mage-parse --target=abl-train    examples/agent_tiny_lm.mg
//   mage-parse --target=abl-generate examples/agent_tiny_lm.mg

net CycleLM {
    layer tok: Embedding(6, 5);
    layer head: Linear(5, 6, 1);
    forward { tok }
}

train LearnCycle {
    net: CycleLM;
    optimizer: Adam(0.1);
    loss: CrossEntropy;
    epochs: 400;
    // token i predicts (i+1) mod 6; targets are one-hot over vocab=6.
    inputs: [[0.0], [1.0], [2.0], [3.0], [4.0], [5.0]];
    targets: [[0, 1, 0, 0, 0, 0],
              [0, 0, 1, 0, 0, 0],
              [0, 0, 0, 1, 0, 0],
              [0, 0, 0, 0, 1, 0],
              [0, 0, 0, 0, 0, 1],
              [1, 0, 0, 0, 0, 0]];
    checkpoint: "agent_tiny_lm.ckpt";
    prompt: [0];
    max_tokens: 8;
    temperature: 0.5;
    top_k: 2;
}
