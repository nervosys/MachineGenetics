// Candidate for the physics program task (plan C3): symplectic Euler.
// Kick, then drift with the new velocity. First order, and symplectic, so its
// energy error stays bounded instead of growing.

@role(candidate)
f step(s: [f64], k: f64, dt: f64) -> [f64] {
    val vel = s[1] - dt * k * s[0];
    [s[0] + dt * vel, vel]
}
