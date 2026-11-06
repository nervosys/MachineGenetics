// The arena's reward, in MAGE.
//
// Plan task 5.1: the policy the loop is judged by is a MAGE program, run by
// the compiler's fuel-bounded evaluator under the `evaluator` role, rather
// than Rust. The loop can vary a MAGE program; it cannot vary the Rust it is
// compiled into. The role makes the language, not the arena, refuse a reward
// that reaches a file, the network or a process.
//
// Compression progress on a probe: the bits a round's training removed from
// a program's output it never saw, discounted by how often its shape has been
// seen (novelty), by its length in tokens (the description-length charge), and
// by a structure factor (1 unless the entropy floor is enabled).
@role(evaluator)
f reward(before: f64, after: f64, len: f64, seen: f64, tokens: f64, charge: f64, structure: f64) -> f64 {
    max(before - after, 0.0) * len / (1.0 + seen) * exp(0.0 - charge * tokens) * structure
}
