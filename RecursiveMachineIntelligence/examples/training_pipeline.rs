//! Training Pipeline Example
//!
//! Demonstrates the training loop, model serialization, and evaluation
//! workflow using the RMI neural module.

use rmi::neural::layers::Layer;
use rmi::neural::serialization::{params_to_json, ModelSerializer};
use rmi::neural::training::{Dataset, Trainer, TrainerConfig};
use rmi::neural::{Adam, Linear, MSELoss};

fn main() {
    println!("╔══════════════════════════════════════════╗");
    println!("║   RMI Training Pipeline Demo             ║");
    println!("╚══════════════════════════════════════════╝\n");

    // ── 1. Prepare dataset ──────────────────────────────────────────────
    println!("=== Step 1: Build Synthetic Dataset ===\n");

    let n = 100;
    let inputs: Vec<Vec<f32>> = (0..n)
        .map(|i| {
            let x = i as f32 / n as f32;
            vec![x, x * 2.0, (x * std::f32::consts::PI).sin()]
        })
        .collect();
    let targets: Vec<Vec<f32>> = inputs
        .iter()
        .map(|x| vec![x[0] * 2.0 + x[1] * 0.5 + x[2] * 0.3])
        .collect();
    let dataset = Dataset::new(inputs, targets);

    println!("  Samples:    {}", dataset.len());
    println!("  Input dim:  {}", dataset.input_dim());
    println!("  Target dim: {}", dataset.target_dim());

    // ── 2. Build model ──────────────────────────────────────────────────
    println!("\n=== Step 2: Build Model ===\n");

    let layers: Vec<Box<dyn Layer>> = vec![
        Box::new(Linear::new(3, 16)),
        Box::new(Linear::new(16, 8)),
        Box::new(Linear::new(8, 1)),
    ];

    let total_params: usize = layers.iter().map(|l| l.num_parameters()).sum();
    for (i, layer) in layers.iter().enumerate() {
        println!(
            "  Layer {}: {} ({} params)",
            i,
            layer.name(),
            layer.num_parameters()
        );
    }
    println!("  Total parameters: {}", total_params);

    // ── 3. Configure training ───────────────────────────────────────────
    println!("\n=== Step 3: Train ===\n");

    let config = TrainerConfig {
        epochs: 20,
        batch_size: 16,
        shuffle: true,
        log_interval: 0,
        clip_grad_norm: 5.0,
        validation_split: 0.2,
    };

    let mut trainer = Trainer::new(
        layers,
        Box::new(MSELoss::new()),
        Box::new(Adam::new(0.01)),
        config,
    );

    println!("  Epochs:           {}", 20);
    println!("  Batch size:       {}", 16);
    println!("  Validation split: 20%");
    println!("  Grad clip norm:   5.0");
    println!("  Optimizer:        Adam (lr=0.01)");
    println!("  Loss:             MSE");

    let history = trainer.fit(&dataset);

    println!("\n  Training complete!");
    println!("  Final train loss: {:.6}", history.losses.last().unwrap());
    if let Some(val) = history.val_losses.last() {
        println!("  Final val loss:   {:.6}", val);
    }
    if let Some(best) = history.best_loss() {
        println!("  Best train loss:  {:.6}", best);
    }
    if let Some(best_val) = history.best_val_loss() {
        println!("  Best val loss:    {:.6}", best_val);
    }

    // Show loss curve
    println!("\n  Loss curve:");
    for (i, loss) in history.losses.iter().enumerate() {
        let bar_len = (40.0 * loss / history.losses[0].max(1e-6)).min(40.0) as usize;
        let bar: String = "█".repeat(bar_len);
        print!("    Epoch {:>2}: {:.6} {}", i + 1, loss, bar);
        if let Some(vl) = history.val_losses.get(i) {
            print!("  (val: {:.6})", vl);
        }
        println!();
    }

    // ── 4. Evaluate ─────────────────────────────────────────────────────
    println!("\n=== Step 4: Evaluate ===\n");

    let eval_loss = trainer.evaluate(&dataset);
    println!("  Full dataset eval loss: {:.6}", eval_loss);

    // ── 5. Inspect parameters ───────────────────────────────────────────
    println!("\n=== Step 5: Parameter Summary ===\n");

    let json = params_to_json(&trainer.layers);
    for (name, info) in json.as_object().unwrap() {
        println!(
            "  {} → shape={}, numel={}, mean={:.4}, range=[{:.4}, {:.4}]",
            name, info["shape"], info["numel"], info["mean"], info["min"], info["max"],
        );
    }

    // ── 6. Save & reload ────────────────────────────────────────────────
    println!("\n=== Step 6: Save & Reload ===\n");

    let model_path = "trained_model.rmi";
    ModelSerializer::save_layers(model_path, &trainer.layers, Some(&history)).unwrap();
    println!("  Saved to: {}", model_path);

    let (params, meta) = ModelSerializer::load(model_path).unwrap();
    println!("  Loaded {} parameters from file", params.len());
    println!("  Version: {}", meta.version);
    println!("  Total params: {}", meta.total_parameters);
    if let Some(ref h) = meta.training_history {
        println!("  Training epochs recorded: {}", h.losses.len());
    }

    // Cleanup
    let _ = std::fs::remove_file(model_path);

    println!("\n╔══════════════════════════════════════════╗");
    println!("║   Training pipeline complete!            ║");
    println!("╚══════════════════════════════════════════╝");
}
