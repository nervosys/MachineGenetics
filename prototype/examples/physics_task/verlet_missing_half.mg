// Candidate for the physics program task (plan C3): velocity Verlet with a
// wrong dt factor. SUBTLY WRONG: the position update drops the 1/2 on
// dt^2 a. It reads like Verlet, and its one-step energy change is smaller than
// symplectic Euler's, but the map is not area-preserving and the energy
// drifts by 41% over the first held-out rollout, so the contract refuses it.

@role(candidate)
f step(s: [f64], k: f64, dt: f64) -> [f64] {
    val a = 0.0 - k * s[0];
    val x = s[0] + dt * s[1] + dt * dt * a;
    val vel = s[1] + 0.5 * dt * (a - k * x);
    [x, vel]
}
