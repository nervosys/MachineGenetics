// Calling a trained net from an ordinary function.
//
// `forward(Net, xs)` runs a net's forward pass on one input vector and returns
// the output as a list of f64. The weights come from the checkpoint of the
// `train` block that names the net, which is the file `--target=abl-infer`
// reads. The checkpoint is committed beside this file, so the example runs from
// a clean checkout. To regenerate it:
//
//   mage-parse --target=abl-train prototype/examples/net_call.mg
//
// Then compare: `mage-parse --target=abl-infer prototype/examples/net_call.mg`
// prints the same predictions that `classify` returns.

net Affine {
    layer fc: Linear(1, 1, 1);
    forward { fc }
}

// y = x + 1, as in train_infer.mg.
train FitAffine {
    net: Affine;
    optimizer: Adam(0.1);
    loss: MSE;
    epochs: 100;
    inputs: [[0.0], [1.0], [2.0], [3.0]];
    targets: [[1.0], [2.0], [3.0], [4.0]];
    checkpoint: "prototype/examples/net_call.ckpt";
}

// The net is reached through this function, which states the shapes it relies on.
// `forward` reads the checkpoint, so the function declares the `fs` effect.
@req(len(xs) == 1)
@ens(len(result) == 1)
f classify(xs: [f64]) -> [f64] / fs {
    forward(Affine, xs)
}

@t
f t_classify_one() / fs {
    assert(len(classify([2.0])) == 1, "one output for one input");
}

@t
f t_classify_near_identity_plus_one() / fs {
    val y = classify([2.0]);
    assert(y[0] > 2.9 && y[0] < 3.1, "y = x + 1, within the fit");
}
