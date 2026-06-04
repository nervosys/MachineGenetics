// framewerx::neural::diffusion_advanced — modern diffusion variants.

// DiT (Diffusion Transformer, Peebles & Xie).
S DiT {
    image_size: usize,
    patch_size: usize,
    hidden_dim: usize,
    num_heads: usize,
    depth: usize,
    num_classes: usize,
}

I DiT {
    +f new(image_size: usize, patch_size: usize, hidden_dim: usize, depth: usize) -> DiT {
        @DiT {
            image_size: image_size,
            patch_size: patch_size,
            hidden_dim: hidden_dim,
            num_heads: 16,
            depth: depth,
            num_classes: 1000,
        }
    }
}

// EDM (Karras et al.) noise schedule.
S EDMSchedule { sigma_min: f32, sigma_max: f32, sigma_data: f32, rho: f32 }

// Consistency Model (Song et al.): direct mapping from noise to data.
S ConsistencyModel { input_dim: usize, conditioning: bool, ema_decay: f32 }

// Rectified Flow.
S RectifiedFlow { input_dim: usize, hidden: usize, num_layers: usize }

// Flow Matching with optimal transport.
S FlowMatching { input_dim: usize, hidden: usize, schedule: s }

// Classifier-Free Guidance wrapper.
S ClassifierFreeGuidance { guidance_scale: f32, unconditional_prob: f32 }
