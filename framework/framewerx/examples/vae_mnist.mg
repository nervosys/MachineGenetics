// VAE for MNIST. Encoder produces (mu, log_var); decoder reconstructs
// from a 16-dim latent sample.

net VaeEncoder {
    layer fc1: Linear(784, 256);
    layer act1: ReLU;
    layer fc2: Linear(256, 128);
    layer act2: ReLU;
    layer mu_head: Linear(128, 16);
    forward { mu_head(act2(fc2(act1(fc1)))) }
}

net VaeDecoder {
    layer fc1: Linear(16, 128);
    layer act1: ReLU;
    layer fc2: Linear(128, 256);
    layer act2: ReLU;
    layer fc3: Linear(256, 784);
    layer out: Sigmoid;
    forward { out(fc3(act2(fc2(act1(fc1))))) }
}
