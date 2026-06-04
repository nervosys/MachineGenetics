// framewerx::architectures::sequence — full RNN / SSM sequence models
//
// Compose recurrent and state-space layers into trainable sequence
// models. Embedding -> recurrent stack -> readout.

// Plain LSTM language model.
S LSTMLanguageModel {
    vocab_size: usize,
    embed_dim: usize,
    hidden_dim: usize,
    num_layers: usize,
    dropout: f32,
}

I LSTMLanguageModel {
    +f new(vocab_size: usize, embed_dim: usize, hidden_dim: usize, num_layers: usize) -> LSTMLanguageModel {
        @LSTMLanguageModel {
            vocab_size: vocab_size,
            embed_dim: embed_dim,
            hidden_dim: hidden_dim,
            num_layers: num_layers,
            dropout: 0.2,
        }
    }
}

// Mamba language model: embedding + stack of MambaBlocks + LM head.
S Mamba {
    vocab_size: usize,
    dim: usize,
    num_layers: usize,
    state_dim: usize,
}

I Mamba {
    +f new(vocab_size: usize, dim: usize, num_layers: usize) -> Mamba {
        @Mamba {
            vocab_size: vocab_size,
            dim: dim,
            num_layers: num_layers,
            state_dim: 16,
        }
    }
}

// Hyena: long-convolution sequence model (alternative to attention).
S Hyena {
    vocab_size: usize,
    dim: usize,
    num_layers: usize,
    filter_order: usize,
}

I Hyena {
    +f new(vocab_size: usize, dim: usize, num_layers: usize) -> Hyena {
        @Hyena {
            vocab_size: vocab_size,
            dim: dim,
            num_layers: num_layers,
            filter_order: 2,
        }
    }
}

// RWKV: receptance-weighted-key-value, hybrid attention+RNN.
S RWKV {
    vocab_size: usize,
    dim: usize,
    num_layers: usize,
}

I RWKV {
    +f new(vocab_size: usize, dim: usize, num_layers: usize) -> RWKV {
        @RWKV {
            vocab_size: vocab_size,
            dim: dim,
            num_layers: num_layers,
        }
    }
}
