// framewerx::architectures::cnn — convolutional network templates
//
// Re-export common CNN topologies as composable specs. Each lowers
// through the bridge to a chain of Conv2D + activation + pool ops.

// Plain CNN feature extractor: Nx (Conv -> ReLU -> Pool).
S CNN {
    input_channels: usize,
    feature_dims: [usize]~,
    num_classes: usize,
}

I CNN {
    +f new(input_channels: usize, feature_dims: [usize]~, num_classes: usize) -> CNN {
        @CNN {
            input_channels: input_channels,
            feature_dims: feature_dims,
            num_classes: num_classes,
        }
    }
}

// ResNet block: two 3x3 convs with a residual skip.
S ResNetBlock {
    in_channels: usize,
    out_channels: usize,
    stride: usize,
}

I ResNetBlock {
    +f new(in_channels: usize, out_channels: usize) -> ResNetBlock {
        @ResNetBlock {
            in_channels: in_channels,
            out_channels: out_channels,
            stride: 1,
        }
    }
}

// MobileNet-style depthwise-separable conv block.
S DepthwiseSeparable {
    in_channels: usize,
    out_channels: usize,
    kernel: usize,
    stride: usize,
}

I DepthwiseSeparable {
    +f new(in_channels: usize, out_channels: usize, kernel: usize) -> DepthwiseSeparable {
        @DepthwiseSeparable {
            in_channels: in_channels,
            out_channels: out_channels,
            kernel: kernel,
            stride: 1,
        }
    }
}

// U-Net: encoder-decoder with skip connections.
S UNet {
    in_channels: usize,
    out_channels: usize,
    base_features: usize,
    depth: usize,
}

I UNet {
    +f new(in_channels: usize, out_channels: usize) -> UNet {
        @UNet {
            in_channels: in_channels,
            out_channels: out_channels,
            base_features: 64,
            depth: 4,
        }
    }
}
