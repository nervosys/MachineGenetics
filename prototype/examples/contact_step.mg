// contact_step.mg — one time step of a ball on a floor, as a box-constrained
// quadratic program (plan C5).
//
// The free step ignores the floor. For a ball of height y and velocity v under
// gravity g, over a step dt, it is
//
//   y_free = y + v * dt - 0.5 * g * dt * dt
//   v_free = v - g * dt
//
// The contact step is the next position and velocity `(yp, vp)` that minimise
//
//   (yp - y_free)^2 + (vp - v_free)^2     subject to   0 <= yp <= 100
//
// The box on `yp` is the floor, and the box on `vp` is wide enough never to
// bind. Projected gradient returns `yp = max(0, y_free)` and `vp = v_free`.
//
// Each function fixes its own state (y, v, dt) in its body, so the solver's
// decision variables are only `yp` and `vp`. The body is a sum of squares of
// affine terms, which is the form the convexity pass certifies as convex.
//
// The reference steps come from `sim_bounce` in arena/src/physics.rs, with
// g = 9.81 (GRAVITY). Both functions are checked by prototype/tests/contact_step.rs.
//
//   mage-parse --solve prototype/examples/contact_step.mg contact_free
//   mage-parse --solve prototype/examples/contact_step.mg contact_floor

@param(0.0, 100.0)
const YP: f64 = 1.0;

@param(-50.0, 50.0)
const VP: f64 = 0.0;

// The ball is at height 1.0 with rest velocity, and the free step stays above
// the floor: y_free = 0.999995095, so yp = y_free.
f contact_free(yp: f64, vp: f64) -> f64 {
    val y0 = 1.0;
    val v0 = 0.0;
    val dt = 0.001;
    val g = 9.81;
    val y_free = y0 + v0 * dt - 0.5 * g * dt * dt;
    val v_free = v0 - g * dt;
    (yp - y_free) * (yp - y_free) + (vp - v_free) * (vp - v_free)
}

// The ball is 1 mm above the floor and falling at 1 m/s, and the free step
// would penetrate: y_free = -0.0094905, so yp = 0.
f contact_floor(yp: f64, vp: f64) -> f64 {
    val y0 = 0.001;
    val v0 = -1.0;
    val dt = 0.01;
    val g = 9.81;
    val y_free = y0 + v0 * dt - 0.5 * g * dt * dt;
    val v_free = v0 - g * dt;
    (yp - y_free) * (yp - y_free) + (vp - v_free) * (vp - v_free)
}
