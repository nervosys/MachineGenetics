// ResNet-style image classifier. Conv stem -> residual blocks ->
// global avg pool -> linear head.

net ResNetClassifier {
    layer stem: Conv2D(3, 64, 7);
    layer pool0: MaxPool(3);
    layer block1: Conv2D(64, 64, 3);
    layer act1: ReLU;
    layer block2: Conv2D(64, 128, 3);
    layer act2: ReLU;
    layer block3: Conv2D(128, 256, 3);
    layer act3: ReLU;
    layer gap: GlobalAvgPool;
    layer head: Linear(256, 1000);
    forward { head(gap(act3(block3(act2(block2(act1(block1(pool0(stem))))))))) }
}
