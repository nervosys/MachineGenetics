// LoRA fine-tuning template. The base Linear is frozen; the LoRA
// addend (low-rank) is the only trainable part. The bridge lowers
// LoRA to MATMUL so the dispatch sees the low-rank matrix shape.

net LoRAFineTuned {
    layer base: Linear(4096, 4096);
    layer lora: LoRA(4096, 8);
    forward { lora(base) }
}
