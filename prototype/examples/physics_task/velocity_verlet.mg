// Candidate for the physics program task (plan C3): velocity Verlet.
// Second order and symplectic: drift with half the old acceleration, then
// kick with the mean of the old and new accelerations.

@role(candidate)
f step(s: [f64], k: f64, dt: f64) -> [f64] {
    val a = 0.0 - k * s[0];
    val x = s[0] + dt * s[1] + 0.5 * dt * dt * a;
    val vel = s[1] + 0.5 * dt * (a - k * x);
    [x, vel]
}
