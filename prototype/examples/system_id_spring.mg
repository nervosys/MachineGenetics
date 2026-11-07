// system_id_spring.mg — fit a damped spring's stiffness and damping (plan C4).
//
// The spring obeys x'' = -k x - c x'. The reference trajectory below was
// integrated with k = 4.0 and c = 0.6, from x(0) = 1.0 and vel(0) = 0.0, by
// semi-implicit Euler at dt = 0.1, one sample per step. It is noise-free.
//
// The objective is the sum of squared position error between a simulation
// with the candidate (k, c) and the reference. Each parameter's domain comes
// from the `@param` const whose name matches it.
//
//   mage-parse --solve prototype/examples/system_id_spring.mg sse
//   mage-parse --solve prototype/examples/system_id_spring.mg sse --seed 7

@param(0.5, 10.0)
const K: f64 = 1.0;

@param(0.0, 3.0)
const C: f64 = 0.5;

f sse(k: f64, c: f64) -> f64 {
    val reference = [0.960000, 0.884000, 0.777200, 0.645720, 0.496300, 0.335993, 0.171865, 0.010710, -0.141204, -0.278355, -0.396143, -0.491018, -0.560560, -0.603506, -0.619736, -0.610203, -0.576833, -0.522392, -0.450322, -0.364563, -0.269368, -0.169109, -0.068102, 0.029570, 0.120198, 0.200580, 0.268116, 0.320876, 0.357635, 0.377883];
    var x = 1.0;
    var vel = 0.0;
    var err = 0.0;
    for i in 0..30 {
        val a = (0.0 - k) * x - c * vel;
        vel = vel + 0.1 * a;
        x = x + 0.1 * vel;
        val d = x - reference[i];
        err = err + d * d;
    }
    err
}
