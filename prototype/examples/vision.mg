// Phase-9 vision pipeline: real Conv2D + pooling + activation chain.
//
// Run: MechGen-parse --target=abl-compute prototype/examples/vision.mg
//
// Input is shaped [1, 8, 8] (CpuBackend default) — though the example
// expects a real CV input it still demonstrates the full op chain.

net Backbone {
    layer c1: Conv2D(1, 4, 3);
    layer act1: ReLU;
    layer c2: Conv2D(4, 8, 3);
    layer act2: ReLU;
    layer p: MaxPool(2);
    forward { c1 }
}

net Classifier {
    layer fc: Linear(16, 4);
    layer out: Softmax;
    forward { fc }
}
