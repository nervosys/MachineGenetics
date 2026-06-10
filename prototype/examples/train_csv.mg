// Phase-12 CSV training demo.
//
// Run: MechGen-parse --target=abl-train prototype/examples/train_csv.mg

net Regressor {
    layer fc1: Linear(2, 4);
    layer fc2: Linear(4, 1);
    forward { fc1 }
}

train FitFromCsv {
    net: Regressor;
    optimizer: Adam(0.05);
    loss: MSE;
    epochs: 200;
    dataset: "prototype/examples/linear_data.csv";
    val_split: 0.25;
    checkpoint: "prototype/examples/fit.ckpt";
    batch_size: 2;
    patience: 15;
}
