// MLP classifier in Framewerx-MG.
// 28*28 = 784 input pixels -> 10 class logits.
// All `layer` lines lower to RMI Neural opcodes via the bridge.

net Classifier {
    layer fc1: Linear(784, 128);
    layer act1: ReLU;
    layer fc2: Linear(128, 64);
    layer act2: ReLU;
    layer head: Linear(64, 10);
    forward { head(act2(fc2(act1(fc1)))) }
}
