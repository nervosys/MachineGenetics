// Unified MechGen + RMI example.
//
// Demonstrates the dual-IR pipeline:
//   - `fn` items lower to MLIR  (systems code)
//   - `net`, `kb`, `agent`, `swarm` items lower to RMIL (neurosymbolic IR)
//
// Run: MechGen-parse --target=rmil prototype/examples/unified.mg
//
// Expected output: per-item RMIL stats (nodes, depth, content hash,
// binary wire size). A full transformer block fits in ~50 bytes.

// ── Systems code (→ MLIR) ──────────────────────────────────────────

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn main() {
    let x = add(2, 3);
}

// ── Neural network (→ RMIL pipeline) ───────────────────────────────

net TransformerBlock {
    layer ln1: LayerNorm;
    layer attn: Attention;
    layer drop1: Dropout;
    layer ln2: LayerNorm;
    layer ff1: Linear;
    layer gelu: GELU;
    layer ff2: Linear;
    layer drop2: Dropout;
    forward { ln1 }
}

net MLP {
    layer fc1: Linear;
    layer act: ReLU;
    layer fc2: Linear;
    forward { fc1 }
}

net ResNetStage {
    layer conv1: Conv2D;
    layer bn1: BatchNorm;
    layer relu: ReLU;
    layer conv2: Conv2D;
    layer bn2: BatchNorm;
    layer pool: MaxPool;
    forward { conv1 }
}

// ── Multi-agent swarm with distributed transport (→ RMIL agent ops) ──

swarm Workers {
    agent: Worker;
    topology: ring;
    consensus: majority;
    transport: rmi_quic;
}

// ── Knowledge base (→ RMIL symbolic ops) ───────────────────────────

kb FamilyKb {
    fact parent(a, b);
    fact parent(b, c);
    rule grandparent(x: i32, y: i32) {
        x
    }
}
