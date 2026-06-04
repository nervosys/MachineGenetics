// framewerx::architectures::transformer — encoder / decoder / full
//
// One TransformerBlock = pre-norm attention + pre-norm FFN with
// residual connections. Stacked into encoder / decoder / decoder-only
// (GPT-style) variants. The bridge lowers attention to opcode 0x0004
// (ATTN) and feed-forward to Linear+activation+Linear.

S TransformerEncoder {
    dim: usize,
    heads: usize,
    layers: usize,
    ffn_mult: usize,
    dropout: f32,
}

I TransformerEncoder {
    +f new(dim: usize, heads: usize, layers: usize) -> TransformerEncoder {
        @TransformerEncoder {
            dim: dim,
            heads: heads,
            layers: layers,
            ffn_mult: 4,
            dropout: 0.1,
        }
    }
}

// Decoder-only (GPT-style): causal masked attention.
S TransformerDecoder {
    dim: usize,
    heads: usize,
    layers: usize,
    vocab_size: usize,
    max_len: usize,
    ffn_mult: usize,
}

I TransformerDecoder {
    +f new(dim: usize, heads: usize, layers: usize, vocab_size: usize, max_len: usize) -> TransformerDecoder {
        @TransformerDecoder {
            dim: dim,
            heads: heads,
            layers: layers,
            vocab_size: vocab_size,
            max_len: max_len,
            ffn_mult: 4,
        }
    }
}

// Encoder-decoder (T5/BART-style): cross-attention bridge.
S EncoderDecoder {
    src_vocab: usize,
    tgt_vocab: usize,
    dim: usize,
    heads: usize,
    enc_layers: usize,
    dec_layers: usize,
}

I EncoderDecoder {
    +f new(src_vocab: usize, tgt_vocab: usize, dim: usize, heads: usize, layers: usize) -> EncoderDecoder {
        @EncoderDecoder {
            src_vocab: src_vocab,
            tgt_vocab: tgt_vocab,
            dim: dim,
            heads: heads,
            enc_layers: layers,
            dec_layers: layers,
        }
    }
}

// Vision Transformer (ViT): patches + position embedding + encoder.
S ViT {
    image_size: usize,
    patch_size: usize,
    dim: usize,
    heads: usize,
    layers: usize,
    num_classes: usize,
}

I ViT {
    +f new(image_size: usize, patch_size: usize, dim: usize, num_classes: usize) -> ViT {
        @ViT {
            image_size: image_size,
            patch_size: patch_size,
            dim: dim,
            heads: 12,
            layers: 12,
            num_classes: num_classes,
        }
    }
}
