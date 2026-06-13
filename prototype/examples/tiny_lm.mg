// Phase-21 tiny LM: trains to memorise a 5-token cycle, then generates.
//
// Step 1 — train:
//   mage-parse --target=abl-train prototype/examples/tiny_lm.mg
// Step 2 — generate:
//   mage-parse --target=abl-generate prototype/examples/tiny_lm.mg

net TinyLM {
    layer tok: Embedding(8, 4);
    layer head: Linear(4, 8, 1);
    forward { tok }
}

// Teach the model: given token i, predict token (i+1) mod 5.
// Inputs are single-token sequences, targets are one-hot over vocab=8.
train Learn {
    net: TinyLM;
    optimizer: Adam(0.1);
    loss: CrossEntropy;
    epochs: 300;
    inputs: [[0.0], [1.0], [2.0], [3.0], [4.0]];
    targets: [[0, 1, 0, 0, 0, 0, 0, 0],
              [0, 0, 1, 0, 0, 0, 0, 0],
              [0, 0, 0, 1, 0, 0, 0, 0],
              [0, 0, 0, 0, 1, 0, 0, 0],
              [1, 0, 0, 0, 0, 0, 0, 0]];
    checkpoint: "prototype/examples/tiny_lm.ckpt";
    prompt: [0];
    max_tokens: 10;
    temperature: 0.8;
    top_k: 3;
    top_p: 0.9;
    seed: 42;
    clip_grad: 1.0;
    warmup_steps: 20;
    lr_schedule: cosine;
    weight_decay: 0.001;
}

// Second train block using plateau LR + early stop on val.
train LearnPlateau {
    net: TinyLM;
    optimizer: Adam(0.1);
    loss: CrossEntropy;
    epochs: 120;
    inputs: [[0.0], [1.0], [2.0], [3.0], [4.0]];
    targets: [[0,1,0,0,0,0,0,0], [0,0,1,0,0,0,0,0],
              [0,0,0,1,0,0,0,0], [0,0,0,0,1,0,0,0],
              [1,0,0,0,0,0,0,0]];
    val_split: 0.2;
    lr_schedule: plateau;
    plateau_patience: 8;
    lr_factor: 0.5;
}
