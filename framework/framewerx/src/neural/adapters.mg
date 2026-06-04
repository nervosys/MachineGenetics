// framewerx::neural::adapters — parameter-efficient fine-tuning (PEFT).

// LoRA: low-rank update W' = W + B*A where rank(B*A) << rank(W).
S LoRA {
    base_dim: usize,
    rank: usize,
    alpha: f32,
    dropout: f32,
    target_modules: [s]~,
}

I LoRA {
    +f new(base_dim: usize, rank: usize) -> LoRA {
        @LoRA {
            base_dim: base_dim,
            rank: rank,
            alpha: 16.0,
            dropout: 0.05,
            target_modules: [],
        }
    }
}

// QLoRA: LoRA on top of 4-bit quantized base weights.
S QLoRA {
    base_dim: usize,
    rank: usize,
    alpha: f32,
    quant_bits: usize,
    double_quant: bool,
}

I QLoRA {
    +f new(base_dim: usize, rank: usize) -> QLoRA {
        @QLoRA {
            base_dim: base_dim,
            rank: rank,
            alpha: 16.0,
            quant_bits: 4,
            double_quant: 1b,
        }
    }
}

// DoRA: decomposed LoRA (weight-decomposed low-rank adaptation).
S DoRA { base_dim: usize, rank: usize, alpha: f32 }

// IA3: scale activations rather than add weights.
S IA3 { dim: usize }

// Prefix tuning: prepend learnable prefix vectors to attention KV.
S PrefixTuning { prefix_len: usize, dim: usize, num_layers: usize }

// Prompt tuning: learnable embedding prepended to input tokens.
S PromptTuning { num_virtual_tokens: usize, dim: usize }

// Adapter (Houlsby): bottleneck FF block inserted after each sublayer.
S Adapter { dim: usize, bottleneck: usize, activation: s }

I Adapter {
    +f new(dim: usize, bottleneck: usize) -> Adapter {
        @Adapter { dim: dim, bottleneck: bottleneck, activation: "gelu" }
    }
}
