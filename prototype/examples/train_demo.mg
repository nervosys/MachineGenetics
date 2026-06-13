// Phase-11 training demo — defines an MLP, supplies the dataset inline,
// and trains for N epochs.
//
// Run: mage-parse --target=abl-train prototype/examples/train_demo.mg

net Regressor {
    layer fc1: Linear(2, 4);
    layer fc2: Linear(4, 1);
    forward { fc1 }
}

train FitLinear {
    net: Regressor;
    optimizer: SGD(0.05);
    loss: MSE;
    epochs: 100;
    // Learn y = 1.5*x1 + 0.5*x2 from four samples.
    inputs: [[0.5, 0.5], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    targets: [[1.0], [1.5], [0.5], [2.0]];
}
