// Candidate for the physics program task (plan C3): explicit Euler. WRONG.
// Both updates read the old state. On an undamped spring that multiplies the
// energy by exactly 1 + k dt^2 every step, so the task's energy contract
// refuses it.

@role(candidate)
f step(s: [f64], k: f64, dt: f64) -> [f64] {
    [s[0] + dt * s[1], s[1] - dt * k * s[0]]
}
