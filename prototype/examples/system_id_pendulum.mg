// system_id_pendulum.mg — fit a pendulum's length from its swing (plan C4).
//
// A full (not small-angle) pendulum obeys theta'' = -(g / L) sin(theta), with
// g = 9.81. The reference trajectory below was integrated with L = 1.5, from
// theta(0) = 0.6 and omega(0) = 0.0, by semi-implicit Euler at dt = 0.1, one
// sample per step. It is noise-free and covers about one and a half swings.
//
// The objective is the sum of squared angle error between a simulation with
// the candidate length and the reference. The domain comes from the `@param`
// const whose name matches the parameter.
//
//   mage-parse --solve prototype/examples/system_id_pendulum.mg sse
//   mage-parse --solve prototype/examples/system_id_pendulum.mg sse --seed 7

@param(0.5, 3.0)
const L: f64 = 1.0;

f sse(l: f64) -> f64 {
    val reference = [0.563072, 0.491235, 0.388548, 0.261084, 0.116738, -0.035225, -0.184884, -0.322521, -0.439429, -0.528514, -0.584621, -0.604635, -0.587472, -0.534060, -0.447357, -0.332364, -0.196032, -0.046961, 0.105180, 0.250455, 0.379521, 0.484357, 0.558741, 0.598455, 0.601325, 0.567196, 0.497929, 0.397427, 0.271612, 0.128251, -0.023474, -0.173665, -0.312555, -0.431334, -0.522772, -0.581556, -0.604414, -0.590107, -0.539407, -0.455117];
    val g = 9.81;
    var theta = 0.6;
    var omega = 0.0;
    var err = 0.0;
    for i in 0..40 {
        val accel = (0.0 - g / l) * sin(theta);
        omega = omega + 0.1 * accel;
        theta = theta + 0.1 * omega;
        val d = theta - reference[i];
        err = err + d * d;
    }
    err
}
