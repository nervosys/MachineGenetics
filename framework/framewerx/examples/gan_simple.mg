// Simple GAN: 100-dim noise -> 784-dim sample (MNIST-shaped) +
// 784 -> 1 discriminator. The trainer orchestrates the two networks
// in alternation via the train block.

net Generator {
    layer fc1: Linear(100, 256);
    layer act1: ReLU;
    layer fc2: Linear(256, 512);
    layer act2: ReLU;
    layer fc3: Linear(512, 784);
    layer out: Tanh;
    forward { out(fc3(act2(fc2(act1(fc1))))) }
}

net Discriminator {
    layer fc1: Linear(784, 512);
    layer act1: ReLU;
    layer fc2: Linear(512, 256);
    layer act2: ReLU;
    layer fc3: Linear(256, 1);
    layer out: Sigmoid;
    forward { out(fc3(act2(fc2(act1(fc1))))) }
}
