// Phase-15 train + infer demo.
//
// Step 1 (train and save):
//   mage-parse --target=abl-train prototype/examples/train_infer.mg
//
// Step 2 (load and predict on the same `inputs:`):
//   mage-parse --target=abl-infer prototype/examples/train_infer.mg

net Affine {
    layer fc: Linear(1, 1, 1);
    forward { fc }
}

// y = x + 1. Bias is necessary to fit the intercept.
train FitAffine {
    net: Affine;
    optimizer: Adam(0.1);
    loss: MSE;
    epochs: 100;
    inputs: [[0.0], [1.0], [2.0], [3.0]];
    targets: [[1.0], [2.0], [3.0], [4.0]];
    checkpoint: "prototype/examples/affine.ckpt";
}
