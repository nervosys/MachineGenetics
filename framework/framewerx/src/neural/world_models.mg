// framewerx::neural::world_models — predictive / self-supervised world models.

// JEPA (Joint-Embedding Predictive Architecture, LeCun).
S JEPA {
    context_encoder_dim: usize,
    target_encoder_dim: usize,
    predictor_dim: usize,
    target_ema: f32,
}

I JEPA {
    +f new(encoder_dim: usize, predictor_dim: usize) -> JEPA {
        @JEPA {
            context_encoder_dim: encoder_dim,
            target_encoder_dim: encoder_dim,
            predictor_dim: predictor_dim,
            target_ema: 0.996,
        }
    }
}

// I-JEPA: image variant operating on patch embeddings.
S IJEPA { patch_size: usize, embed_dim: usize, predictor_depth: usize }

// V-JEPA: video variant.
S VJEPA { patch_size: usize, num_frames: usize, embed_dim: usize }

// Dreamer-V3 world model: encoder + dynamics + reward + continue heads.
S DreamerV3 {
    obs_dim: usize,
    action_dim: usize,
    hidden_dim: usize,
    stoch_dim: usize,
    deter_dim: usize,
}

// SimSiam / BYOL / DINO: self-supervised representation learners.
S SimSiam { encoder_dim: usize, projector_dim: usize, predictor_dim: usize }
S BYOL { encoder_dim: usize, projector_dim: usize, ema: f32 }
S DINO { encoder_dim: usize, projector_dim: usize, ema: f32, num_crops: usize }

// MAE (Masked Autoencoder): mask 75% of patches, reconstruct.
S MAE { patch_size: usize, embed_dim: usize, mask_ratio: f32, decoder_dim: usize }

I MAE {
    +f new(patch_size: usize, embed_dim: usize) -> MAE {
        @MAE { patch_size: patch_size, embed_dim: embed_dim, mask_ratio: 0.75, decoder_dim: 512 }
    }
}
