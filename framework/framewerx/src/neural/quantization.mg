// framewerx::neural::quantization — low-precision representations.

S Int8Linear { in_features: usize, out_features: usize, per_channel: bool }
S Int4Linear { in_features: usize, out_features: usize, group_size: usize }

// BitNet b1.58: ternary weights {-1, 0, 1} with absmean scaling.
S BitNetLinear { in_features: usize, out_features: usize }

// GPTQ quantization spec.
S GPTQConfig { bits: usize, group_size: usize, sym: bool, true_sequential: bool }

// AWQ (activation-aware weight quantization) spec.
S AWQConfig { bits: usize, group_size: usize, zero_point: bool }

// FP8 / FP4 mixed-precision training config.
S MixedPrecision { compute_dtype: s, param_dtype: s, master_dtype: s }

I MixedPrecision {
    +f fp8() -> MixedPrecision {
        @MixedPrecision { compute_dtype: "fp8_e4m3", param_dtype: "fp8_e4m3", master_dtype: "fp32" }
    }
    +f bf16() -> MixedPrecision {
        @MixedPrecision { compute_dtype: "bf16", param_dtype: "bf16", master_dtype: "fp32" }
    }
}
