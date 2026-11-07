// Candidate for the physics program task (plan C3): the state never moves.
// WRONG, and the contract cannot see it: energy is conserved exactly. The
// rollout error is what ranks it last. Conservation is necessary, not
// sufficient.

@role(candidate)
f step(s: [f64], k: f64, dt: f64) -> [f64] {
    s
}
