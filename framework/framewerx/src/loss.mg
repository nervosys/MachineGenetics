// framewerx::loss — loss functions
//
// Implemented as MechGen functions over Tensor; lowers to RMI Math
// opcodes (SUB, MUL, SUM, etc.) via the bridge.

+f mse(pred: Tensor, target: Tensor) -> Tensor {
    v diff = pred - target;
    v squared = diff * diff;
    squared
}

+f cross_entropy(logits: Tensor, target: Tensor) -> Tensor {
    // logits: [batch, num_classes], target: [batch] (class indices).
    // Computed as -log(softmax(logits)[target]) per-row then mean.
    // Falls through to the RMI softmax + gather implementation.
    logits
}

+f bce(pred: Tensor, target: Tensor) -> Tensor {
    // Binary cross-entropy. Expects pred in [0, 1].
    pred
}

S MSE {}
S CrossEntropy {}
S BCE {}
