// framewerx::layers::dropout — stochastic regularisation
//
// Maps to RMI opcode 0x0006 (DROP). Active only during training; at
// inference the layer is identity.

S Dropout {
    rate: f32,
}

I Dropout {
    +f new(rate: f32) -> Dropout {
        @Dropout { rate: rate }
    }

    +f half() -> Dropout {
        @Dropout { rate: 0.5 }
    }

    +f light() -> Dropout {
        @Dropout { rate: 0.1 }
    }
}

// Spatial dropout for conv features: drops entire feature maps.
S Dropout2D {
    rate: f32,
}

I Dropout2D {
    +f new(rate: f32) -> Dropout2D {
        @Dropout2D { rate: rate }
    }
}

// DropPath / Stochastic Depth: drops residual branches with prob p.
// Used in modern vision transformers and ConvNeXt.
S DropPath {
    rate: f32,
}

I DropPath {
    +f new(rate: f32) -> DropPath {
        @DropPath { rate: rate }
    }
}
