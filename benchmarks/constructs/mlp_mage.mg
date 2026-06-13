net MLP {
    layer fc1: Linear(8, 16);
    layer act1: ReLU;
    layer fc2: Linear(16, 4);
    layer act2: Sigmoid;
    forward { fc1 }
}
