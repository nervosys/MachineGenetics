// framewerx::layers::conv — convolution and pooling
//
// Maps to RMI opcodes:
//   Conv2D       -> 0x0003
//   MaxPool      -> (pooling family)
//   AvgPool      -> (pooling family)
//
// Conv2D arguments lower to the RMI ATTN/CONV2D constructor.
// Stride, padding, and dilation default to (1, 0, 1) and can be
// specified via the explicit constructor.

S Conv2D {
    in_channels: usize,
    out_channels: usize,
    kernel: usize,
    stride: usize,
    padding: usize,
    dilation: usize,
}

I Conv2D {
    +f new(in_channels: usize, out_channels: usize, kernel: usize) -> Conv2D {
        @Conv2D {
            in_channels: in_channels,
            out_channels: out_channels,
            kernel: kernel,
            stride: 1,
            padding: 0,
            dilation: 1,
        }
    }

    +f strided(in_channels: usize, out_channels: usize, kernel: usize, stride: usize) -> Conv2D {
        @Conv2D {
            in_channels: in_channels,
            out_channels: out_channels,
            kernel: kernel,
            stride: stride,
            padding: 0,
            dilation: 1,
        }
    }
}

S MaxPool { kernel: usize, stride: usize }
S AvgPool { kernel: usize, stride: usize }
S GlobalAvgPool {}

I MaxPool {
    +f new(kernel: usize) -> MaxPool {
        @MaxPool { kernel: kernel, stride: kernel }
    }
}

I AvgPool {
    +f new(kernel: usize) -> AvgPool {
        @AvgPool { kernel: kernel, stride: kernel }
    }
}

// Example: small CNN feature extractor
//
//   net SmallCnn {
//       layer c1: Conv2D(3, 16, 3);
//       layer a1: ReLU;
//       layer p1: MaxPool(2);
//       layer c2: Conv2D(16, 32, 3);
//       layer a2: ReLU;
//       layer p2: GlobalAvgPool;
//       forward { p2(a2(c2(p1(a1(c1))))) }
//   }
