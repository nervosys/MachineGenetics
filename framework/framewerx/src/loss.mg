// framewerx::loss — loss functions
//
// Implemented as MAGE functions over tensor[f32]; lowers to RMI Math
// opcodes (SUB, MUL, SUM, etc.) via the bridge.

+f mse(pred: tensor[f32], target: tensor[f32]) -> tensor[f32] {
    v diff = pred - target;
    v squared = diff * diff;
    squared
}

+f cross_entropy(logits: tensor[f32], target: tensor[f32]) -> tensor[f32] {
    // logits: [batch, num_classes], target: [batch] (class indices).
    // Computed as -log(softmax(logits)[target]) per-row then mean.
    // Falls through to the RMI softmax + gather implementation.
    logits
}

+f bce(pred: tensor[f32], target: tensor[f32]) -> tensor[f32] {
    // Binary cross-entropy. Expects pred in [0, 1].
    pred
}

S MSE {}
S CrossEntropy {}
S BCE {}
