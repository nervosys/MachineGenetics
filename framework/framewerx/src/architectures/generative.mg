// framewerx::architectures::generative — VAE / GAN / diffusion
//
// Generative model templates. Each declares the component networks
// (encoder/decoder/generator/discriminator) as fields the trainer
// orchestrates via the train block in MechGen.

// Variational Autoencoder: encoder produces (mu, log_var), decoder
// reconstructs from the reparameterised sample.
S VAE {
    input_dim: usize,
    latent_dim: usize,
    hidden_dim: usize,
}

I VAE {
    +f new(input_dim: usize, latent_dim: usize, hidden_dim: usize) -> VAE {
        @VAE {
            input_dim: input_dim,
            latent_dim: latent_dim,
            hidden_dim: hidden_dim,
        }
    }
}

// Conditional VAE: encoder/decoder both consume a condition vector.
S CVAE {
    input_dim: usize,
    condition_dim: usize,
    latent_dim: usize,
    hidden_dim: usize,
}

I CVAE {
    +f new(input_dim: usize, condition_dim: usize, latent_dim: usize) -> CVAE {
        @CVAE {
            input_dim: input_dim,
            condition_dim: condition_dim,
            latent_dim: latent_dim,
            hidden_dim: 256,
        }
    }
}

// Generative Adversarial Network: generator + discriminator pair.
S GAN {
    noise_dim: usize,
    output_dim: usize,
    generator_hidden: usize,
    discriminator_hidden: usize,
}

I GAN {
    +f new(noise_dim: usize, output_dim: usize) -> GAN {
        @GAN {
            noise_dim: noise_dim,
            output_dim: output_dim,
            generator_hidden: 256,
            discriminator_hidden: 256,
        }
    }
}

// Wasserstein GAN with gradient penalty.
S WGAN_GP {
    noise_dim: usize,
    output_dim: usize,
    gp_lambda: f32,
}

I WGAN_GP {
    +f new(noise_dim: usize, output_dim: usize) -> WGAN_GP {
        @WGAN_GP {
            noise_dim: noise_dim,
            output_dim: output_dim,
            gp_lambda: 10.0,
        }
    }
}

// Denoising Diffusion Probabilistic Model: noise schedule + U-Net.
S DDPM {
    input_dim: usize,
    timesteps: usize,
    beta_start: f32,
    beta_end: f32,
}

I DDPM {
    +f new(input_dim: usize, timesteps: usize) -> DDPM {
        @DDPM {
            input_dim: input_dim,
            timesteps: timesteps,
            beta_start: 0.0001,
            beta_end: 0.02,
        }
    }
}

// Latent Diffusion (Stable Diffusion family): runs DDPM in VAE latent space.
S LatentDiffusion {
    image_size: usize,
    latent_size: usize,
    text_embed_dim: usize,
    timesteps: usize,
}

I LatentDiffusion {
    +f new(image_size: usize, latent_size: usize) -> LatentDiffusion {
        @LatentDiffusion {
            image_size: image_size,
            latent_size: latent_size,
            text_embed_dim: 768,
            timesteps: 1000,
        }
    }
}
