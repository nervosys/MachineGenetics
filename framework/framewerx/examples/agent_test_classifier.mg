// Agent-built (P83 self-test). 4 input features -> 8 hidden -> 2 class logits.
// Adapted from mlp_classifier.mg template.

net AgentTestClassifier {
    layer fc1: Linear(4, 8);
    layer act: ReLU;
    layer head: Linear(8, 2);
    forward { head(act(fc1)) }
}
