//! Physical trajectories as held-out data (plan P1–P2).
//!
//! A deterministic generator of five classical systems, and a byte encoding
//! of their trajectories. Later items feed these bytes to byte-level learners
//! as held-out data whose structure is known exactly: the next value is a
//! function of the last few, so a learner that predicts it has learned
//! physics, and one that does not is measurably behind.
//!
//! ## Systems and integrators
//!
//! * **Pendulum**, `θ'' = -(g/L) sin θ`: velocity Verlet (symplectic).
//! * **Damped spring**, `x'' = -(k/m) x - (c/m) x'`: the exact closed-form
//!   solution of the linear ODE, for underdamped, critical and overdamped
//!   cases. Exact rather than semi-implicit, so the `c = 0` energy check
//!   measures rounding and not integrator error.
//! * **Double pendulum**: the Lagrangian equations written out in
//!   [`double_accel`], integrated by classical fourth-order Runge–Kutta. RK4
//!   is **not** symplectic, so its energy drifts slowly; the drift test
//!   bounds it at a small step.
//! * **N-body gravity**, n = 3 in 2D, with Plummer softening: velocity Verlet
//!   on pairwise forces. Pairwise forces make total momentum conserved by
//!   construction, up to rounding.
//! * **Bouncing ball**: velocity Verlet, which is exact for constant gravity,
//!   plus contact handling. When a step crosses the floor, the crossing time
//!   is solved for within the step, the velocity is reflected there with
//!   restitution `e`, and the rest of the step is flown from the floor.
//!
//! ## Byte encoding
//!
//! See [`encode`] and [`decode`]. Header: the magic `PHY1`, a system byte,
//! the parameters as `f64` little-endian, the quantum, the channel count and
//! the timestep count as `u32` little-endian. Body: per timestep, each channel
//! as `i32` little-endian, `round(value / quantum)`.
//!
//! ## Quanta
//!
//! Each system's quantum is chosen so that its largest plausible magnitude
//! (angles, positions, velocities) fits in `i32` with headroom of at least
//! 2^8. Out-of-range values saturate, because Rust's float-to-int casts do.
//! The quanta are: pendulum, spring and bounce `1e-5`; double pendulum and
//! n-body `1e-4`. [`System::default_quantum`] is the source of truth.
//!
//! ## Measured LZ bits per byte (`measure::lz_bits_per_byte`)
//!
//! The first 64 KiB of an eight-trajectory [`corpus`] per system, Train range,
//! seed 1, 1,100 steps each:
//!
//! | source | bits/byte |
//! |---|---|
//! | random bytes | 6.83 |
//! | pendulum | 4.29 |
//! | spring | 3.81 |
//! | double pendulum | 3.70 |
//! | n-body | 3.50 |
//! | bounce | 4.21 |
//!
//! LZ76 is biased low on 64 KiB, so random bytes measure 6.83, not 8. Every
//! physical system sits below random, which is a first, crude learnability
//! signal. It is not evidence that a learner can predict these bytes (plan
//! P4). Reproduce with `cargo test -- --ignored --nocapture lz_bits_per_byte_64_kib`.

use crate::grammar::Rng;
use crate::kernel::Corpus;
use crate::learner::{build, LearnerSpec};
use std::f64::consts::PI;

/// The magic that opens every encoded trajectory.
pub const MAGIC: &[u8; 4] = b"PHY1";

/// Gravitational acceleration for the pendulum, double pendulum and bounce.
pub const GRAVITY: f64 = 9.81;

/// Gravitational constant for the n-body system, in its own units.
const NBODY_G: f64 = 1.0;

/// Plummer softening for the n-body system.
const NBODY_SOFTENING: f64 = 0.05;

/// One timestep of a system: its channels, in the order documented on
/// [`System::channels`].
pub type State = Vec<f64>;

/// The five systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    Pendulum,
    Spring,
    DoublePendulum,
    NBody,
    Bounce,
}

/// Every system, in code order.
pub const SYSTEMS: [System; 5] = [
    System::Pendulum,
    System::Spring,
    System::DoublePendulum,
    System::NBody,
    System::Bounce,
];

/// Which parameter ranges to draw from. The two are disjoint in every
/// dimension (see [`System::bounds`] and the tests), so a learner trained on
/// `Train` never sees a `HeldOut` parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamRange {
    Train,
    HeldOut,
}

/// A system's parameters. Units are SI for the pendulum, spring and bounce;
/// the n-body system is in its own units (`G = 1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Params {
    /// `length` is `L` in metres.
    Pendulum { g: f64, length: f64 },
    /// `k` stiffness, `m` mass, `c` viscous damping.
    Spring { k: f64, m: f64, c: f64 },
    /// Lengths `l1`, `l2` and masses `m1`, `m2`, with `g`.
    DoublePendulum {
        g: f64,
        l1: f64,
        l2: f64,
        m1: f64,
        m2: f64,
    },
    /// Gravitational constant `g`, Plummer softening, and three masses.
    NBody { g: f64, softening: f64, m: [f64; 3] },
    /// Gravity `g` and coefficient of restitution `e`.
    Bounce { g: f64, e: f64 },
}

impl System {
    /// The byte in the encoded header.
    pub fn code(self) -> u8 {
        match self {
            System::Pendulum => 0,
            System::Spring => 1,
            System::DoublePendulum => 2,
            System::NBody => 3,
            System::Bounce => 4,
        }
    }

    pub fn from_code(code: u8) -> Option<System> {
        SYSTEMS.iter().copied().find(|s| s.code() == code)
    }

    pub fn name(self) -> &'static str {
        match self {
            System::Pendulum => "pendulum",
            System::Spring => "spring",
            System::DoublePendulum => "double-pendulum",
            System::NBody => "nbody",
            System::Bounce => "bounce",
        }
    }

    /// The name a `--heldout-physics` spec uses for this system.
    pub fn spec_name(self) -> &'static str {
        match self {
            System::Pendulum => "pendulum",
            System::Spring => "spring",
            System::DoublePendulum => "double_pendulum",
            System::NBody => "nbody",
            System::Bounce => "bounce",
        }
    }

    /// The system a spec name names, if any.
    pub fn from_spec_name(name: &str) -> Option<System> {
        SYSTEMS.iter().copied().find(|s| s.spec_name() == name)
    }

    /// Channels per timestep, in order:
    ///
    /// * pendulum: `θ, ω`
    /// * spring: `x, v`
    /// * double pendulum: `θ1, θ2, ω1, ω2`
    /// * n-body: `x1, y1, x2, y2, x3, y3, vx1, vy1, vx2, vy2, vx3, vy3`
    /// * bounce: `y, v`
    pub fn channels(self) -> usize {
        match self {
            System::Pendulum | System::Spring | System::Bounce => 2,
            System::DoublePendulum => 4,
            System::NBody => 12,
        }
    }

    /// How many `f64` parameters the header carries for this system.
    pub fn param_count(self) -> usize {
        match self {
            System::Pendulum | System::Bounce => 2,
            System::Spring => 3,
            System::DoublePendulum | System::NBody => 5,
        }
    }

    /// Channels that hold angles, which the encoding wraps to `(-π, π]`.
    pub fn angle_channels(self) -> &'static [usize] {
        match self {
            System::Pendulum => &[0],
            System::DoublePendulum => &[0, 1],
            _ => &[],
        }
    }

    /// The quantum for [`encode`] in this system; see the module docs.
    pub fn default_quantum(self) -> f64 {
        match self {
            System::Pendulum | System::Spring | System::Bounce => 1e-5,
            System::DoublePendulum | System::NBody => 1e-4,
        }
    }

    /// The timestep [`corpus`] integrates with.
    pub fn default_dt(self) -> f64 {
        match self {
            System::Pendulum => 1e-2,
            System::Spring => 5e-2,
            System::DoublePendulum | System::NBody | System::Bounce => 1e-3,
        }
    }

    /// Names of the drawn quantities, in the order of [`bounds`](Self::bounds)
    /// and [`drawn_values`].
    pub fn dims(self) -> &'static [&'static str] {
        match self {
            System::Pendulum => &["length", "amplitude"],
            System::Spring => &["stiffness", "damping", "amplitude"],
            System::DoublePendulum => &["length1", "length2", "mass1", "mass2", "amplitude"],
            System::NBody => &["mass1", "mass2", "mass3", "orbit_radius"],
            System::Bounce => &["restitution", "drop_height"],
        }
    }

    /// The inclusive interval each quantity is drawn from, in the order of
    /// [`dims`](Self::dims).
    ///
    /// "Amplitude" is the initial angle or displacement, which fixes the
    /// initial energy. The `Train` and `HeldOut` intervals are disjoint in
    /// every dimension, and the pendulum, spring and bounce also have disjoint
    /// initial energies. The test
    /// `train_and_heldout_ranges_are_disjoint_over_1000_samples` checks both.
    pub fn bounds(self, range: ParamRange) -> Vec<(f64, f64)> {
        match (self, range) {
            (System::Pendulum, ParamRange::Train) => vec![(0.5, 1.0), (0.2, 1.0)],
            (System::Pendulum, ParamRange::HeldOut) => vec![(1.5, 2.0), (2.0, 2.8)],
            (System::Spring, ParamRange::Train) => vec![(1.0, 4.0), (0.0, 0.3), (0.2, 1.0)],
            (System::Spring, ParamRange::HeldOut) => vec![(6.0, 9.0), (0.5, 0.9), (1.5, 2.5)],
            (System::DoublePendulum, ParamRange::Train) => vec![
                (0.8, 1.0),
                (0.8, 1.0),
                (0.5, 1.0),
                (0.5, 1.0),
                (0.2, 1.0),
            ],
            (System::DoublePendulum, ParamRange::HeldOut) => vec![
                (1.2, 1.4),
                (1.2, 1.4),
                (1.5, 2.0),
                (1.5, 2.0),
                (1.8, 2.8),
            ],
            (System::NBody, ParamRange::Train) => {
                vec![(0.5, 1.0), (0.5, 1.0), (0.5, 1.0), (1.0, 1.5)]
            }
            (System::NBody, ParamRange::HeldOut) => {
                vec![(1.5, 2.0), (1.5, 2.0), (1.5, 2.0), (2.5, 3.0)]
            }
            (System::Bounce, ParamRange::Train) => vec![(0.5, 0.8), (0.5, 1.5)],
            (System::Bounce, ParamRange::HeldOut) => vec![(0.85, 0.95), (3.0, 5.0)],
        }
    }
}

impl Params {
    pub fn system(&self) -> System {
        match self {
            Params::Pendulum { .. } => System::Pendulum,
            Params::Spring { .. } => System::Spring,
            Params::DoublePendulum { .. } => System::DoublePendulum,
            Params::NBody { .. } => System::NBody,
            Params::Bounce { .. } => System::Bounce,
        }
    }

    /// The header order of the parameters.
    fn values(&self) -> Vec<f64> {
        match *self {
            Params::Pendulum { g, length } => vec![g, length],
            Params::Spring { k, m, c } => vec![k, m, c],
            Params::DoublePendulum { g, l1, l2, m1, m2 } => vec![g, l1, l2, m1, m2],
            Params::NBody { g, softening, m } => vec![g, softening, m[0], m[1], m[2]],
            Params::Bounce { g, e } => vec![g, e],
        }
    }

    /// Inverse of [`values`](Self::values). `v` must have `param_count` entries.
    fn from_values(system: System, v: &[f64]) -> Params {
        match system {
            System::Pendulum => Params::Pendulum {
                g: v[0],
                length: v[1],
            },
            System::Spring => Params::Spring {
                k: v[0],
                m: v[1],
                c: v[2],
            },
            System::DoublePendulum => Params::DoublePendulum {
                g: v[0],
                l1: v[1],
                l2: v[2],
                m1: v[3],
                m2: v[4],
            },
            System::NBody => Params::NBody {
                g: v[0],
                softening: v[1],
                m: [v[2], v[3], v[4]],
            },
            System::Bounce => Params::Bounce { g: v[0], e: v[1] },
        }
    }
}

// ── Sampling ─────────────────────────────────────────────────────────────

fn uniform(rng: &mut Rng, (lo, hi): (f64, f64)) -> f64 {
    lo + (hi - lo) * rng.next_f64()
}

/// `amp` with a random sign.
fn signed(rng: &mut Rng, amp: f64) -> f64 {
    if rng.below(2) == 0 {
        amp
    } else {
        -amp
    }
}

/// Draws parameters and initial conditions from `range`, deterministically
/// given the state of `rng`. Velocities start at rest, except the n-body
/// bodies, which orbit, and the centre of mass, which is at rest.
pub fn sample_params(system: System, range: ParamRange, rng: &mut Rng) -> (Params, State) {
    let b = system.bounds(range);
    match system {
        System::Pendulum => {
            let length = uniform(rng, b[0]);
            let amp = uniform(rng, b[1]);
            let theta = signed(rng, amp);
            (
                Params::Pendulum {
                    g: GRAVITY,
                    length,
                },
                vec![theta, 0.0],
            )
        }
        System::Spring => {
            let k = uniform(rng, b[0]);
            let c = uniform(rng, b[1]);
            let amp = uniform(rng, b[2]);
            let x = signed(rng, amp);
            (Params::Spring { k, m: 1.0, c }, vec![x, 0.0])
        }
        System::DoublePendulum => {
            let l1 = uniform(rng, b[0]);
            let l2 = uniform(rng, b[1]);
            let m1 = uniform(rng, b[2]);
            let m2 = uniform(rng, b[3]);
            let amp = uniform(rng, b[4]);
            let th1 = signed(rng, amp);
            // |θ2| ≤ amp, so the amplitude bound still holds for both angles.
            let th2 = amp * (2.0 * rng.next_f64() - 1.0);
            (
                Params::DoublePendulum {
                    g: GRAVITY,
                    l1,
                    l2,
                    m1,
                    m2,
                },
                vec![th1, th2, 0.0, 0.0],
            )
        }
        System::NBody => {
            let m = [
                uniform(rng, b[0]),
                uniform(rng, b[1]),
                uniform(rng, b[2]),
            ];
            let radius = uniform(rng, b[3]);
            let phase = 2.0 * PI * rng.next_f64();
            let total: f64 = m.iter().sum();
            // Half the circular speed for the total mass at this radius.
            let vt = 0.5 * (NBODY_G * total / radius).sqrt();
            let mut pos = [[0.0f64; 2]; 3];
            let mut vel = [[0.0f64; 2]; 3];
            for k in 0..3 {
                let a = phase + 2.0 * PI * k as f64 / 3.0;
                pos[k] = [radius * a.cos(), radius * a.sin()];
                vel[k] = [-vt * a.sin(), vt * a.cos()];
            }
            // Remove the net momentum, so the centre of mass is at rest and
            // total momentum is zero at the start.
            let mut p = [0.0f64; 2];
            for k in 0..3 {
                p[0] += m[k] * vel[k][0];
                p[1] += m[k] * vel[k][1];
            }
            for k in 0..3 {
                vel[k][0] -= p[0] / total;
                vel[k][1] -= p[1] / total;
            }
            let state = nbody_state(&pos, &vel);
            (
                Params::NBody {
                    g: NBODY_G,
                    softening: NBODY_SOFTENING,
                    m,
                },
                state,
            )
        }
        System::Bounce => {
            let e = uniform(rng, b[0]);
            let h = uniform(rng, b[1]);
            (Params::Bounce { g: GRAVITY, e }, vec![h, 0.0])
        }
    }
}

/// The drawn quantities of a sampled system, in the order of
/// [`System::dims`]. The tests check each lies in its [`System::bounds`].
pub fn drawn_values(params: &Params, state: &[f64]) -> Vec<f64> {
    match *params {
        Params::Pendulum { length, .. } => vec![length, state[0].abs()],
        Params::Spring { k, c, .. } => vec![k, c, state[0].abs()],
        Params::DoublePendulum { l1, l2, m1, m2, .. } => {
            vec![l1, l2, m1, m2, state[0].abs()]
        }
        Params::NBody { m, .. } => {
            vec![m[0], m[1], m[2], state[0].hypot(state[1])]
        }
        Params::Bounce { e, .. } => vec![e, state[0]],
    }
}

// ── Integration ──────────────────────────────────────────────────────────

/// Integrates `system` from `state` for `steps` steps of `dt`. Returns
/// `steps + 1` states, the first being `state` itself.
pub fn simulate(
    system: System,
    params: &Params,
    state: State,
    dt: f64,
    steps: usize,
) -> Vec<State> {
    assert_eq!(params.system(), system, "params belong to another system");
    assert_eq!(state.len(), system.channels(), "wrong channel count");
    match *params {
        Params::Pendulum { g, length } => sim_pendulum(g, length, &state, dt, steps),
        Params::Spring { k, m, c } => sim_spring(k, m, c, &state, dt, steps),
        Params::DoublePendulum { g, l1, l2, m1, m2 } => {
            sim_double(g, l1, l2, m1, m2, &state, dt, steps)
        }
        Params::NBody { g, softening, m } => sim_nbody(g, softening, m, &state, dt, steps),
        Params::Bounce { g, e } => sim_bounce(g, e, &state, dt, steps),
    }
}

/// Velocity Verlet on `θ'' = -(g/L) sin θ`.
fn sim_pendulum(g: f64, length: f64, s: &[f64], dt: f64, steps: usize) -> Vec<State> {
    let acc = |th: f64| -(g / length) * th.sin();
    let (mut th, mut w) = (s[0], s[1]);
    let mut out = Vec::with_capacity(steps + 1);
    out.push(s.to_vec());
    for _ in 0..steps {
        let a0 = acc(th);
        th += w * dt + 0.5 * a0 * dt * dt;
        let a1 = acc(th);
        w += 0.5 * (a0 + a1) * dt;
        out.push(vec![th, w]);
    }
    out
}

/// The exact solution of `x'' + 2γ x' + ω² x = 0` at time `t`, with
/// `ω² = k/m` and `γ = c/(2m)`. Covers the underdamped, critical and
/// overdamped cases.
fn spring_exact(k: f64, m: f64, c: f64, x0: f64, v0: f64, t: f64) -> (f64, f64) {
    let w0 = (k / m).sqrt();
    let gam = c / (2.0 * m);
    if (gam - w0).abs() <= 1e-12 * w0 {
        // Critical: x = e^{-γt}(x0 + b t), with b = v0 + γ x0.
        let e = (-gam * t).exp();
        let b = v0 + gam * x0;
        (e * (x0 + b * t), e * (v0 - gam * b * t))
    } else if gam < w0 {
        let wd = (w0 * w0 - gam * gam).sqrt();
        let e = (-gam * t).exp();
        let (cs, sn) = ((wd * t).cos(), (wd * t).sin());
        let a = x0;
        let b = (v0 + gam * x0) / wd;
        (
            e * (a * cs + b * sn),
            e * ((b * wd - gam * a) * cs + (-a * wd - gam * b) * sn),
        )
    } else {
        let s = (gam * gam - w0 * w0).sqrt();
        let r1 = -gam + s;
        let r2 = -gam - s;
        let c1 = (v0 - r2 * x0) / (r1 - r2);
        let c2 = x0 - c1;
        let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
        (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
    }
}

/// The damped spring, by its exact solution. Energy is non-increasing for
/// `c ≥ 0`, because `dE/dt = -c v²`.
fn sim_spring(k: f64, m: f64, c: f64, s: &[f64], dt: f64, steps: usize) -> Vec<State> {
    let mut out = Vec::with_capacity(steps + 1);
    out.push(s.to_vec());
    for n in 1..=steps {
        let (x, v) = spring_exact(k, m, c, s[0], s[1], n as f64 * dt);
        out.push(vec![x, v]);
    }
    out
}

/// The double-pendulum angular accelerations, from the Lagrangian of the
/// system (Goldstein, *Classical Mechanics*, ch. 1; the form written out in
/// Shinbrot, Grebogi, Wisdom & Yorke, "Chaos in a double pendulum", Am. J.
/// Phys. 60, 491 (1992)). Valid for equal or unequal masses and lengths.
///
/// With `Δ = θ1 - θ2` and `D = 2m1 + m2 - m2 cos 2Δ`:
///
/// ```text
/// θ1'' = [ -g(2m1+m2) sin θ1 - m2 g sin(θ1-2θ2)
///          - 2 sin Δ m2 (θ2'² l2 + θ1'² l1 cos Δ) ] / (l1 D)
/// θ2'' = [ 2 sin Δ ( θ1'² l1 (m1+m2) + g(m1+m2) cos θ1
///          + θ2'² l2 m2 cos Δ ) ] / (l2 D)
/// ```
///
/// The energy of the same system is `T + V`, with
/// `T = ½(m1+m2) l1² θ1'² + ½ m2 l2² θ2'² + m2 l1 l2 θ1' θ2' cos Δ` and
/// `V = -(m1+m2) g l1 cos θ1 - m2 g l2 cos θ2`. The energy tests check that the
/// accelerations and the energy agree.
#[allow(clippy::too_many_arguments)]
fn double_accel(
    g: f64,
    l1: f64,
    l2: f64,
    m1: f64,
    m2: f64,
    th1: f64,
    th2: f64,
    w1: f64,
    w2: f64,
) -> (f64, f64) {
    let d = th1 - th2;
    let den = 2.0 * m1 + m2 - m2 * (2.0 * d).cos();
    let a1 = (-g * (2.0 * m1 + m2) * th1.sin()
        - m2 * g * (th1 - 2.0 * th2).sin()
        - 2.0 * d.sin() * m2 * (w2 * w2 * l2 + w1 * w1 * l1 * d.cos()))
        / (l1 * den);
    let a2 = (2.0
        * d.sin()
        * (w1 * w1 * l1 * (m1 + m2) + g * (m1 + m2) * th1.cos() + w2 * w2 * l2 * m2 * d.cos()))
        / (l2 * den);
    (a1, a2)
}

/// Classical RK4 on `(θ1, θ2, ω1, ω2)`. **Not symplectic**: its energy error
/// is bounded only by the step size, so the drift test uses a small `dt`.
#[allow(clippy::too_many_arguments)]
fn sim_double(
    g: f64,
    l1: f64,
    l2: f64,
    m1: f64,
    m2: f64,
    s: &[f64],
    dt: f64,
    steps: usize,
) -> Vec<State> {
    let f = |y: [f64; 4]| -> [f64; 4] {
        let (a1, a2) = double_accel(g, l1, l2, m1, m2, y[0], y[1], y[2], y[3]);
        [y[2], y[3], a1, a2]
    };
    let add = |y: [f64; 4], k: [f64; 4], h: f64| -> [f64; 4] {
        [
            y[0] + h * k[0],
            y[1] + h * k[1],
            y[2] + h * k[2],
            y[3] + h * k[3],
        ]
    };
    let mut y = [s[0], s[1], s[2], s[3]];
    let mut out = Vec::with_capacity(steps + 1);
    out.push(s.to_vec());
    for _ in 0..steps {
        let k1 = f(y);
        let k2 = f(add(y, k1, dt / 2.0));
        let k3 = f(add(y, k2, dt / 2.0));
        let k4 = f(add(y, k3, dt));
        for i in 0..4 {
            y[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
        out.push(y.to_vec());
    }
    out
}

/// Pairwise accelerations with Plummer softening:
/// `a_i = Σ_j G m_j (x_j - x_i) / (|x_j - x_i|² + ε²)^{3/2}`.
/// Each pair is evaluated once and applied to both bodies, which is what
/// makes total momentum conserved.
fn nbody_accel(g: f64, eps: f64, m: &[f64; 3], x: &[[f64; 2]; 3]) -> [[f64; 2]; 3] {
    let mut a = [[0.0f64; 2]; 3];
    for i in 0..3 {
        for j in (i + 1)..3 {
            let dx = x[j][0] - x[i][0];
            let dy = x[j][1] - x[i][1];
            let r2 = dx * dx + dy * dy + eps * eps;
            let inv = g / (r2 * r2.sqrt());
            a[i][0] += m[j] * inv * dx;
            a[i][1] += m[j] * inv * dy;
            a[j][0] -= m[i] * inv * dx;
            a[j][1] -= m[i] * inv * dy;
        }
    }
    a
}

fn nbody_state(x: &[[f64; 2]; 3], v: &[[f64; 2]; 3]) -> State {
    let mut s = Vec::with_capacity(12);
    for p in x {
        s.extend_from_slice(p);
    }
    for p in v {
        s.extend_from_slice(p);
    }
    s
}

/// Velocity Verlet on the pairwise forces.
fn sim_nbody(g: f64, eps: f64, m: [f64; 3], s: &[f64], dt: f64, steps: usize) -> Vec<State> {
    let mut x = [[s[0], s[1]], [s[2], s[3]], [s[4], s[5]]];
    let mut v = [[s[6], s[7]], [s[8], s[9]], [s[10], s[11]]];
    let mut a = nbody_accel(g, eps, &m, &x);
    let mut out = Vec::with_capacity(steps + 1);
    out.push(s.to_vec());
    for _ in 0..steps {
        for i in 0..3 {
            for d in 0..2 {
                x[i][d] += v[i][d] * dt + 0.5 * a[i][d] * dt * dt;
            }
        }
        let a_new = nbody_accel(g, eps, &m, &x);
        for i in 0..3 {
            for d in 0..2 {
                v[i][d] += 0.5 * (a[i][d] + a_new[i][d]) * dt;
            }
        }
        a = a_new;
        out.push(nbody_state(&x, &v));
    }
    out
}

/// Velocity Verlet for constant gravity, with contact handling. Within a
/// step the free flight is exact, so the only approximation is the step
/// boundary. If the step ends below the floor, the contact time `τ` is the
/// smaller root of `y + vτ - ½gτ² = 0` (a positive root, since the ball is
/// above the floor at the start of the step). The velocity reflects there,
/// and the rest of the step is flown from the floor. A ball at rest on the
/// floor stays there. The loop is bounded, so it cannot stall.
fn sim_bounce(g: f64, e: f64, s: &[f64], dt: f64, steps: usize) -> Vec<State> {
    let (mut y, mut v) = (s[0], s[1]);
    let mut out = Vec::with_capacity(steps + 1);
    out.push(vec![y, v]);
    for _ in 0..steps {
        let mut rem = dt;
        for _ in 0..64 {
            let y_end = y + v * rem - 0.5 * g * rem * rem;
            if y_end >= 0.0 {
                y = y_end;
                v -= g * rem;
                break;
            }
            let tau = (((v * v + 2.0 * g * y.max(0.0)).sqrt() + v) / g).clamp(0.0, rem);
            v = -e * (v - g * tau);
            y = 0.0;
            rem -= tau;
            if v <= 1e-12 {
                v = 0.0;
                break;
            }
        }
        out.push(vec![y, v]);
    }
    out
}

// ── Conserved quantities ─────────────────────────────────────────────────

/// Energy per unit mass for the pendulum and bounce, and total energy for the
/// others. Conserved by the conservative systems; the damped spring's is not.
pub fn energy(params: &Params, s: &[f64]) -> f64 {
    match *params {
        Params::Pendulum { g, length } => {
            0.5 * (length * s[1]).powi(2) + g * length * (1.0 - s[0].cos())
        }
        Params::Spring { k, m, .. } => 0.5 * m * s[1] * s[1] + 0.5 * k * s[0] * s[0],
        Params::DoublePendulum { g, l1, l2, m1, m2 } => {
            let (th1, th2, w1, w2) = (s[0], s[1], s[2], s[3]);
            let t = 0.5 * (m1 + m2) * l1 * l1 * w1 * w1
                + 0.5 * m2 * l2 * l2 * w2 * w2
                + m2 * l1 * l2 * w1 * w2 * (th1 - th2).cos();
            let v = -(m1 + m2) * g * l1 * th1.cos() - m2 * g * l2 * th2.cos();
            t + v
        }
        Params::NBody { g, softening, m } => {
            let mut e = 0.0;
            for i in 0..3 {
                let (vx, vy) = (s[6 + 2 * i], s[7 + 2 * i]);
                e += 0.5 * m[i] * (vx * vx + vy * vy);
            }
            for i in 0..3 {
                for j in (i + 1)..3 {
                    let dx = s[2 * j] - s[2 * i];
                    let dy = s[2 * j + 1] - s[2 * i + 1];
                    e -= g * m[i] * m[j] / (dx * dx + dy * dy + softening * softening).sqrt();
                }
            }
            e
        }
        Params::Bounce { g, .. } => 0.5 * s[1] * s[1] + g * s[0],
    }
}

/// The largest deviation of the energy from its initial value, relative to
/// that value: `max_n |E_n - E_0| / |E_0|`.
pub fn energy_drift(params: &Params, states: &[State]) -> f64 {
    let e0 = energy(params, &states[0]);
    assert!(e0 != 0.0, "drift is relative to a nonzero initial energy");
    states
        .iter()
        .map(|s| (energy(params, s) - e0).abs())
        .fold(0.0, f64::max)
        / e0.abs()
}

/// Total linear momentum `Σ m_i v_i` of the n-body state.
pub fn nbody_momentum(m: [f64; 3], s: &[f64]) -> [f64; 2] {
    let mut p = [0.0f64; 2];
    for i in 0..3 {
        p[0] += m[i] * s[6 + 2 * i];
        p[1] += m[i] * s[7 + 2 * i];
    }
    p
}

/// Wraps an angle to `(-π, π]`.
pub fn wrap_angle(theta: f64) -> f64 {
    let x = theta.rem_euclid(2.0 * PI);
    if x > PI {
        x - 2.0 * PI
    } else {
        x
    }
}

// ── Byte encoding (plan P2) ──────────────────────────────────────────────

/// How the body of an encoded trajectory lays out its values. The header is the
/// same for every encoding, and records the quantum the values were stored at.
/// The decoder needs the encoding to read the body; it is not in the header,
/// so [`encode`] and [`decode`] stay byte-for-byte the plan P2 layout, which is
/// [`Encoding::I32`].
///
/// Every encoding stores values in the same order: per timestep, each channel,
/// with angle channels wrapped to `(-π, π]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Each value is `i32` little-endian, `round(value / quantum)`, at
    /// [`System::default_quantum`]. Values past the `i32` range saturate.
    I32,
    /// As `I32`, but at 100 times the default quantum and as `i16`
    /// little-endian. Values past the `i16` range saturate.
    Coarse,
    /// The `I32` values, laid out byte-plane-major within the trajectory: byte 0
    /// of every value in time order, then byte 1, and so on, as Blosc's shuffle.
    Planes,
    /// Per channel, the `i32` second difference `q[t] - 2 q[t-1] + q[t-2]` for
    /// `t >= 2`, with wrapping arithmetic. The first two samples are raw. This
    /// is the residual against constant-velocity extrapolation.
    Diff2,
}

impl Encoding {
    /// Every encoding, in the order `physics-baseline` documents them.
    pub const ALL: [Encoding; 4] = [Encoding::I32, Encoding::Coarse, Encoding::Planes, Encoding::Diff2];

    /// The name `--encoding` takes.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::I32 => "i32",
            Encoding::Coarse => "coarse",
            Encoding::Planes => "planes",
            Encoding::Diff2 => "diff2",
        }
    }

    /// The encoding a `--encoding` name denotes, if any.
    pub fn from_name(name: &str) -> Option<Encoding> {
        Encoding::ALL.into_iter().find(|e| e.name() == name)
    }

    /// The quantum this encoding stores `system`'s values at.
    pub fn quantum(self, system: System) -> f64 {
        match self {
            Encoding::Coarse => 100.0 * system.default_quantum(),
            _ => system.default_quantum(),
        }
    }

    /// Bytes per stored value.
    fn width(self) -> usize {
        match self {
            Encoding::Coarse => 2,
            _ => 4,
        }
    }
}

/// Encodes a trajectory as `i32` values at `quantum`, the plan P2 layout. Angles
/// are wrapped to `(-π, π]`. Each value becomes `round(value / quantum)`. Values
/// past the `i32` range saturate, which Rust's float-to-int cast does, so
/// [`System::default_quantum`] should keep magnitudes in range.
pub fn encode(system: System, params: &Params, states: &[State], quantum: f64) -> Vec<u8> {
    encode_body(system, params, states, quantum, Encoding::I32)
}

/// Encodes a trajectory in `encoding`, at that encoding's quantum for `system`.
/// For [`Encoding::I32`] this is [`encode`] at [`System::default_quantum`].
pub fn encode_as(system: System, params: &Params, states: &[State], encoding: Encoding) -> Vec<u8> {
    encode_body(system, params, states, encoding.quantum(system), encoding)
}

fn encode_body(system: System, params: &Params, states: &[State], quantum: f64, encoding: Encoding) -> Vec<u8> {
    assert_eq!(params.system(), system, "params belong to another system");
    assert!(quantum > 0.0 && quantum.is_finite(), "quantum must be positive");
    let ch = system.channels();
    let mut out = Vec::with_capacity(4 + 1 + 8 * system.param_count() + 8 + 4 + 4);
    out.extend_from_slice(MAGIC);
    out.push(system.code());
    for v in params.values() {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&quantum.to_le_bytes());
    out.extend_from_slice(&(ch as u32).to_le_bytes());
    out.extend_from_slice(&(states.len() as u32).to_le_bytes());
    let angles = system.angle_channels();
    let mut values: Vec<f64> = Vec::with_capacity(states.len() * ch);
    for s in states {
        assert_eq!(s.len(), ch, "state has the wrong channel count");
        for (c, &x) in s.iter().enumerate() {
            values.push(if angles.contains(&c) { wrap_angle(x) } else { x });
        }
    }
    match encoding {
        Encoding::I32 => {
            for &x in &values {
                out.extend_from_slice(&((x / quantum).round() as i32).to_le_bytes());
            }
        }
        Encoding::Coarse => {
            for &x in &values {
                out.extend_from_slice(&((x / quantum).round() as i16).to_le_bytes());
            }
        }
        Encoding::Planes => {
            let q: Vec<[u8; 4]> = values.iter().map(|&x| ((x / quantum).round() as i32).to_le_bytes()).collect();
            for k in 0..4 {
                out.extend(q.iter().map(|b| b[k]));
            }
        }
        Encoding::Diff2 => {
            let q: Vec<i32> = values.iter().map(|&x| (x / quantum).round() as i32).collect();
            for i in 0..q.len() {
                let d = if i < 2 * ch {
                    q[i]
                } else {
                    q[i].wrapping_sub(q[i - ch].wrapping_mul(2)).wrapping_add(q[i - 2 * ch])
                };
                out.extend_from_slice(&d.to_le_bytes());
            }
        }
    }
    out
}

/// The decoded parts of an encoded trajectory.
pub type Decoded = (System, Params, Vec<State>, f64);

struct Header {
    system: System,
    params: Params,
    quantum: f64,
    channels: usize,
    steps: usize,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| format!("truncated: needed {n} bytes at offset {}", self.at))?;
        let s = &self.bytes[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn f64(&mut self) -> Result<f64, String> {
        let b: [u8; 8] = self.take(8)?.try_into().expect("took 8 bytes");
        Ok(f64::from_le_bytes(b))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b: [u8; 4] = self.take(4)?.try_into().expect("took 4 bytes");
        Ok(u32::from_le_bytes(b))
    }
}

fn read_header(bytes: &[u8]) -> Result<(Header, usize), String> {
    let mut r = Reader { bytes, at: 0 };
    let magic = r.take(4)?;
    if magic != &MAGIC[..] {
        return Err(format!(
            "bad magic {magic:02x?}, expected {:?}",
            std::str::from_utf8(MAGIC).unwrap_or("?")
        ));
    }
    let code = r.take(1)?[0];
    let system = System::from_code(code).ok_or_else(|| format!("unknown system code {code}"))?;
    let mut values = Vec::with_capacity(system.param_count());
    for _ in 0..system.param_count() {
        values.push(r.f64()?);
    }
    let params = Params::from_values(system, &values);
    let quantum = r.f64()?;
    if !(quantum.is_finite() && quantum > 0.0) {
        return Err(format!("invalid quantum {quantum}"));
    }
    let channels = r.u32()? as usize;
    if channels != system.channels() {
        return Err(format!(
            "{} has {} channels, header says {channels}",
            system.name(),
            system.channels()
        ));
    }
    let steps = r.u32()? as usize;
    Ok((
        Header {
            system,
            params,
            quantum,
            channels,
            steps,
        },
        r.at,
    ))
}

/// Decodes one encoded trajectory. The input must be exactly one trajectory:
/// use [`encoded_len`] to split a stream.
pub fn decode(bytes: &[u8]) -> Result<Decoded, String> {
    decode_as(bytes, Encoding::I32)
}

/// Decodes one trajectory encoded in `encoding` by [`encode_as`]. As with
/// [`decode`], the input must be exactly one trajectory.
pub fn decode_as(bytes: &[u8], encoding: Encoding) -> Result<Decoded, String> {
    let (h, header_len) = read_header(bytes)?;
    let n = h
        .steps
        .checked_mul(h.channels)
        .ok_or_else(|| "step count overflows".to_string())?;
    let body_len = n
        .checked_mul(encoding.width())
        .ok_or_else(|| "step count overflows".to_string())?;
    let total = header_len
        .checked_add(body_len)
        .ok_or_else(|| "length overflows".to_string())?;
    if bytes.len() != total {
        return Err(format!(
            "expected {total} bytes for this header, got {}",
            bytes.len()
        ));
    }
    let body = &bytes[header_len..];
    let le4 = |b: &[u8]| i32::from_le_bytes(b.try_into().expect("4 bytes"));
    // The stored integers, in stream order: time-major, channel-minor.
    let q: Vec<i32> = match encoding {
        Encoding::I32 => body.chunks_exact(4).map(le4).collect(),
        Encoding::Coarse => body
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes(b.try_into().expect("2 bytes")) as i32)
            .collect(),
        Encoding::Planes => (0..n)
            .map(|i| le4(&[body[i], body[n + i], body[2 * n + i], body[3 * n + i]]))
            .collect(),
        Encoding::Diff2 => {
            let d: Vec<i32> = body.chunks_exact(4).map(le4).collect();
            let mut q: Vec<i32> = Vec::with_capacity(n);
            for i in 0..n {
                let v = if i < 2 * h.channels {
                    d[i]
                } else {
                    d[i]
                        .wrapping_add(q[i - h.channels].wrapping_mul(2))
                        .wrapping_sub(q[i - 2 * h.channels])
                };
                q.push(v);
            }
            q
        }
    };
    let values: Vec<f64> = q.iter().map(|&v| v as f64 * h.quantum).collect();
    let states = if h.channels == 0 {
        Vec::new()
    } else {
        values.chunks_exact(h.channels).map(|s| s.to_vec()).collect()
    };
    Ok((h.system, h.params, states, h.quantum))
}

/// The length in bytes of the encoded trajectory at the start of `bytes`,
/// read from its header. Use it to walk a corpus stream.
pub fn encoded_len(bytes: &[u8]) -> Result<usize, String> {
    encoded_len_as(bytes, Encoding::I32)
}

/// As [`encoded_len`], for a trajectory encoded in `encoding`.
pub fn encoded_len_as(bytes: &[u8], encoding: Encoding) -> Result<usize, String> {
    let (h, header_len) = read_header(bytes)?;
    h.steps
        .checked_mul(h.channels * encoding.width())
        .and_then(|b| b.checked_add(header_len))
        .ok_or_else(|| "length overflows".to_string())
}

/// `trajectories` encoded trajectories of `system`, drawn from `range` with
/// one RNG seeded by `seed`, each of `steps` steps at
/// [`System::default_dt`], concatenated into one byte stream.
pub fn corpus(
    system: System,
    range: ParamRange,
    seed: u64,
    trajectories: usize,
    steps: usize,
) -> Vec<u8> {
    corpus_as(system, range, seed, trajectories, steps, Encoding::I32)
}

/// As [`corpus`], with each trajectory encoded in `encoding`. The same
/// parameters and states are drawn for every encoding, so only the bytes differ.
pub fn corpus_as(
    system: System,
    range: ParamRange,
    seed: u64,
    trajectories: usize,
    steps: usize,
    encoding: Encoding,
) -> Vec<u8> {
    let mut rng = Rng(seed);
    let mut out = Vec::new();
    for _ in 0..trajectories {
        let (p, s) = sample_params(system, range, &mut rng);
        let states = simulate(system, &p, s, system.default_dt(), steps);
        out.extend(encode_as(system, &p, &states, encoding));
    }
    out
}

/// A parsed `--heldout-physics` spec: `<system>:<trajectories>:<steps>:<seed>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicsSpec {
    pub system: System,
    pub trajectories: usize,
    pub steps: usize,
    pub seed: u64,
}

/// Parse `<system>:<trajectories>:<steps>:<seed>`. The system is one of
/// `pendulum`, `spring`, `double_pendulum`, `nbody`, `bounce`. Trajectories
/// and steps are at least 1; the seed is any `u64`.
pub fn parse_spec(spec: &str) -> Result<PhysicsSpec, String> {
    let fields: Vec<&str> = spec.split(':').collect();
    let [sys, trajectories, steps, seed] = fields.as_slice() else {
        return Err(format!(
            "physics spec `{spec}` has {} fields, not 4 (expected <system>:<trajectories>:<steps>:<seed>)",
            fields.len()
        ));
    };
    let system = System::from_spec_name(sys).ok_or_else(|| {
        let names: Vec<&str> = SYSTEMS.iter().map(|s| s.spec_name()).collect();
        format!("physics spec `{spec}`: unknown system `{sys}` (expected one of {})", names.join(", "))
    })?;
    let count = |field: &str, what: &str| -> Result<usize, String> {
        let n: usize = field
            .parse()
            .map_err(|_| format!("physics spec `{spec}`: {what} `{field}` is not a whole number"))?;
        if n == 0 {
            return Err(format!("physics spec `{spec}`: {what} must be at least 1"));
        }
        Ok(n)
    };
    let trajectories = count(trajectories, "trajectories")?;
    let steps = count(steps, "steps")?;
    let seed: u64 = seed
        .parse()
        .map_err(|_| format!("physics spec `{spec}`: seed `{seed}` is not an unsigned 64-bit number"))?;
    Ok(PhysicsSpec { system, trajectories, steps, seed })
}

/// The encoding of `--heldout-physics` corpora. Plan P4b measured four, and
/// byte-planes and second differences both passed its gate. Planes is chosen:
/// it holds the same values as `i32`, only reordered, so what a learner gains
/// on it is learned. `diff2` builds a constant-velocity predictor into the
/// bytes, which does part of the physics for the learner.
pub const HELDOUT_ENCODING: Encoding = Encoding::Planes;

/// The held-out corpus a `--heldout-physics` spec names: `trajectories` whole
/// trajectories drawn from [`ParamRange::HeldOut`], with the spec's seed,
/// encoded as [`HELDOUT_ENCODING`]. Its name is
/// `physics:<system>:<trajectories>:<steps>:seed<seed>:<encoding>`, so a report
/// says what was scored, and figures from different encodings cannot be mixed.
pub fn physics_corpus(spec: &str) -> Result<Corpus, String> {
    let s = parse_spec(spec)?;
    let bytes = corpus_as(s.system, ParamRange::HeldOut, s.seed, s.trajectories, s.steps, HELDOUT_ENCODING);
    let name = format!(
        "physics:{}:{}:{}:seed{}:{}",
        s.system.spec_name(),
        s.trajectories,
        s.steps,
        s.seed,
        HELDOUT_ENCODING.name()
    );
    Ok(Corpus::new(name, bytes))
}

/// Bytes per sequence in a baseline train step. Each chunk is one sequence,
/// so context resets at chunk starts, as it does at program starts in the arena.
pub const BASELINE_CHUNK: usize = 1024;
/// Sequences per train step in the physics baseline.
pub const BASELINE_BATCH: usize = 4;
/// Bytes of each corpus that bits per byte is measured on: the start of the
/// train corpus, and the start of the held-out corpus.
pub const BASELINE_EVAL_BYTES: usize = 64 << 10;
/// Checkpoints across the budget, each a tenth of it, plus the untrained start.
pub const BASELINE_CHECKPOINTS: usize = 10;

/// One checkpoint of a learning curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurvePoint {
    /// Bytes the learner has been trained on so far.
    pub bytes_trained: u64,
    /// Bits per byte on the first [`BASELINE_EVAL_BYTES`] of the train corpus.
    /// Those bytes are in the training stream, so this is a fit measure.
    pub train_bpb: f64,
    /// Bits per byte on the first [`BASELINE_EVAL_BYTES`] of the held-out corpus.
    pub heldout_bpb: f64,
    /// Wall-clock seconds spent in train steps so far, evaluation excluded.
    /// Noisy on a shared machine, so it is a cost reading, not a constant.
    pub seconds: f64,
}

/// A learning curve, and the order-0 (byte-frequency) entropy of the evaluation
/// windows. Order-0 is what a memoryless model achieves, so a learner that is
/// not below it has learned no more than byte frequencies. The fair reference
/// is `train_order0_bpb`: a byte-frequency model fit on the train side, applied
/// to held-out data. `heldout_order0_bpb` is stricter: it is the held-out
/// window's own entropy, an oracle no model could be trained to reach.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    pub points: Vec<CurvePoint>,
    /// Empirical byte entropy of the train evaluation window, in bits per byte.
    pub train_order0_bpb: f64,
    /// Empirical byte entropy of the held-out evaluation window, in bits per byte.
    pub heldout_order0_bpb: f64,
}

/// The empirical order-0 entropy of `bytes`, in bits per byte.
pub fn order0_bits_per_byte(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return f64::NAN;
    }
    let mut counts = [0usize; 256];
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let n = bytes.len() as f64;
    -counts.iter().filter(|&&c| c > 0).map(|&c| (c as f64 / n) * (c as f64 / n).log2()).sum::<f64>()
}

/// Train `spec` on `train` in contiguous chunks, and measure it at each tenth of
/// `budget` bytes (plus the untrained start). Train steps read `train` in order
/// and never revisit a byte, so `budget` must not exceed `train.len()`. The
/// `train` argument need not be physics: the text comparison calls this too.
pub fn learning_curve(
    spec: LearnerSpec,
    train: &[u8],
    heldout: &[u8],
    budget: usize,
    chunk: usize,
    batch: usize,
) -> Result<Curve, String> {
    let per_step = chunk * batch;
    let steps = budget / per_step;
    if steps < BASELINE_CHECKPOINTS {
        return Err(format!(
            "budget of {budget} bytes is under {BASELINE_CHECKPOINTS} train steps of {per_step} bytes"
        ));
    }
    if steps * per_step > train.len() {
        return Err(format!("train corpus is {} bytes, short of the {} bytes the budget trains on", train.len(), steps * per_step));
    }
    if heldout.is_empty() {
        return Err("held-out corpus is empty".into());
    }
    let mut learner = build(spec)?;
    let tw = &train[..train.len().min(BASELINE_EVAL_BYTES)];
    let hw = &heldout[..heldout.len().min(BASELINE_EVAL_BYTES)];
    let mut points = Vec::with_capacity(BASELINE_CHECKPOINTS + 1);
    let mut done = 0usize;
    let mut seconds = 0.0f64;
    for k in 0..=BASELINE_CHECKPOINTS {
        let target = steps * k / BASELINE_CHECKPOINTS;
        let began = std::time::Instant::now();
        while done < target {
            let start = done * per_step;
            let batch_slices: Vec<&[u8]> = (0..batch).map(|i| &train[start + i * chunk..start + (i + 1) * chunk]).collect();
            learner.train(&batch_slices);
            done += 1;
        }
        seconds += began.elapsed().as_secs_f64();
        points.push(CurvePoint {
            bytes_trained: learner.bytes_seen(),
            train_bpb: learner.bits_per_byte(tw),
            heldout_bpb: learner.bits_per_byte(hw),
            seconds,
        });
    }
    Ok(Curve { points, train_order0_bpb: order0_bits_per_byte(tw), heldout_order0_bpb: order0_bits_per_byte(hw) })
}

/// The physics baseline (plan P4): train `spec` directly on `system`'s
/// [`ParamRange::Train`] trajectories, and measure it against the same system's
/// [`ParamRange::HeldOut`] trajectories, over `budget` bytes. `steps` is the
/// length of each trajectory and `seed` seeds both corpora. No agents are involved.
pub fn physics_baseline(
    system: System,
    spec: LearnerSpec,
    budget: usize,
    steps: usize,
    seed: u64,
) -> Result<Curve, String> {
    physics_baseline_as(system, spec, budget, steps, seed, Encoding::I32)
}

/// [`physics_baseline`] with both corpora encoded in `encoding` (plan P4b).
pub fn physics_baseline_as(
    system: System,
    spec: LearnerSpec,
    budget: usize,
    steps: usize,
    seed: u64,
    encoding: Encoding,
) -> Result<Curve, String> {
    // Whole trajectories, just enough to cover the budget. The held-out side is
    // just enough to cover the evaluation window.
    let train_per = corpus_as(system, ParamRange::Train, seed, 1, steps, encoding).len().max(1);
    let held_per = corpus_as(system, ParamRange::HeldOut, seed, 1, steps, encoding).len().max(1);
    let train = corpus_as(system, ParamRange::Train, seed, budget.div_ceil(train_per).max(1), steps, encoding);
    let heldout = corpus_as(
        system,
        ParamRange::HeldOut,
        seed,
        BASELINE_EVAL_BYTES.div_ceil(held_per).max(1),
        steps,
        encoding,
    );
    learning_curve(spec, &train, &heldout, budget, BASELINE_CHUNK, BASELINE_BATCH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::lz_bits_per_byte;

    fn trajectory(system: System, seed: u64, steps: usize) -> (Params, Vec<State>) {
        let mut rng = Rng(seed);
        let (p, s) = sample_params(system, ParamRange::Train, &mut rng);
        let states = simulate(system, &p, s, system.default_dt(), steps);
        (p, states)
    }

    #[test]
    fn pendulum_conserves_energy_to_1e4_over_10000_steps() {
        let p = Params::Pendulum {
            g: GRAVITY,
            length: 1.0,
        };
        let states = simulate(System::Pendulum, &p, vec![1.0, 0.0], 1e-3, 10_000);
        let d = energy_drift(&p, &states);
        eprintln!("pendulum energy drift: {d:e}");
        assert!(d < 1e-4, "pendulum drift {d:e}");
    }

    #[test]
    fn undamped_spring_conserves_energy_to_1e6() {
        let p = Params::Spring {
            k: 2.0,
            m: 1.0,
            c: 0.0,
        };
        let states = simulate(System::Spring, &p, vec![1.0, 0.0], 1e-2, 10_000);
        let d = energy_drift(&p, &states);
        eprintln!("undamped spring energy drift: {d:e}");
        assert!(d < 1e-6, "spring drift {d:e}");
    }

    #[test]
    fn damped_spring_energy_is_non_increasing_at_every_step() {
        let p = Params::Spring {
            k: 2.0,
            m: 1.0,
            c: 0.3,
        };
        let states = simulate(System::Spring, &p, vec![1.0, 0.5], 1e-2, 2_000);
        for n in 1..states.len() {
            let (a, b) = (energy(&p, &states[n - 1]), energy(&p, &states[n]));
            assert!(b <= a + 1e-12 * a.abs(), "energy rose at step {n}: {a} -> {b}");
        }
        assert!(energy(&p, &states[2_000]) < 0.5 * energy(&p, &states[0]));
    }

    #[test]
    fn damped_spring_covers_under_critical_and_over_damping() {
        // Each regime must be continuous with the others and must decay.
        // With k = m = 1 the critical damping is c = 2.
        for c in [0.5, 2.0 - 1e-6, 2.0, 2.0 + 1e-6, 4.0] {
            let p = Params::Spring { k: 1.0, m: 1.0, c };
            let s = simulate(System::Spring, &p, vec![1.0, 0.0], 0.05, 400);
            let last = s.last().expect("steps");
            assert!(last[0].abs() < 1.0 && last[0].is_finite(), "c={c}");
            assert!(energy(&p, last) < energy(&p, &s[0]), "c={c} did not decay");
        }
    }

    #[test]
    fn double_pendulum_conserves_energy_to_1e3_over_2000_rk4_steps() {
        let p = Params::DoublePendulum {
            g: GRAVITY,
            l1: 1.0,
            l2: 0.8,
            m1: 1.0,
            m2: 0.5,
        };
        let states = simulate(System::DoublePendulum, &p, vec![1.2, -0.6, 0.0, 0.0], 1e-3, 2_000);
        let d = energy_drift(&p, &states);
        eprintln!("double pendulum energy drift: {d:e}");
        assert!(d < 1e-3, "double pendulum drift {d:e}");
    }

    #[test]
    fn nbody_conserves_momentum_exactly_and_energy_to_1e3() {
        for seed in 1..=5 {
            let (p, states) = {
                let mut rng = Rng(seed);
                let (p, s) = sample_params(System::NBody, ParamRange::Train, &mut rng);
                (p, simulate(System::NBody, &p, s, 1e-4, 20_000))
            };
            let m = match p {
                Params::NBody { m, .. } => m,
                _ => unreachable!(),
            };
            let p0 = nbody_momentum(m, &states[0]);
            for (n, s) in states.iter().enumerate() {
                let pn = nbody_momentum(m, s);
                let dev = (pn[0] - p0[0]).abs().max((pn[1] - p0[1]).abs());
                assert!(dev < 1e-12, "seed {seed} step {n}: momentum off by {dev:e}");
            }
            let d = energy_drift(&p, &states);
            eprintln!("nbody seed {seed}: energy drift {d:e}, momentum dev {:e}", {
                let pn = nbody_momentum(m, states.last().expect("steps"));
                pn[0].abs().max(pn[1].abs())
            });
            assert!(d < 1e-3, "seed {seed}: n-body drift {d:e}");
        }
    }

    /// Local maxima of the bounce height, skipping the first sample.
    fn peaks(states: &[State]) -> Vec<f64> {
        (1..states.len() - 1)
            .filter(|&n| {
                let y = states[n][0];
                y > 1e-3 && y > states[n - 1][0] && y >= states[n + 1][0]
            })
            .map(|n| states[n][0])
            .collect()
    }

    #[test]
    fn elastic_bounce_keeps_its_peak_height_within_1e3() {
        let p = Params::Bounce { g: GRAVITY, e: 1.0 };
        let states = simulate(System::Bounce, &p, vec![1.0, 0.0], 1e-3, 6_000);
        assert!(states.iter().all(|s| s[0] >= 0.0), "ball went below the floor");
        let h = peaks(&states);
        assert!(h.len() >= 4, "only {} peaks", h.len());
        let worst = h.iter().map(|hk| (hk - 1.0).abs()).fold(0.0, f64::max);
        eprintln!("elastic bounce: {} peaks, worst |h - h0| {worst:e}", h.len());
        for (k, &hk) in h.iter().enumerate() {
            assert!((hk - 1.0).abs() < 1e-3, "peak {k} at {hk}");
        }
    }

    #[test]
    fn inelastic_bounce_peaks_decay_by_e_squared_per_bounce() {
        let e = 0.8;
        let p = Params::Bounce { g: GRAVITY, e };
        let states = simulate(System::Bounce, &p, vec![1.0, 0.0], 1e-3, 8_000);
        assert!(states.iter().all(|s| s[0] >= 0.0), "ball went below the floor");
        let h = peaks(&states);
        assert!(h.len() >= 5, "only {} peaks", h.len());
        let worst = (0..4)
            .map(|k| (h[k + 1] / h[k] - e * e).abs())
            .fold(0.0, f64::max);
        eprintln!("inelastic bounce e=0.8: {} peaks, worst ratio error {worst:e}", h.len());
        for k in 0..4 {
            let ratio = h[k + 1] / h[k];
            assert!(
                (ratio - e * e).abs() < 1e-3,
                "peak ratio {k}: {ratio} vs e² = {}",
                e * e
            );
        }
    }

    #[test]
    fn same_seed_gives_a_bit_identical_trajectory() {
        for system in SYSTEMS {
            let (pa, a) = trajectory(system, 42, 300);
            let (pb, b) = trajectory(system, 42, 300);
            assert_eq!(pa, pb, "{}", system.name());
            assert_eq!(a.len(), b.len());
            for (sa, sb) in a.iter().zip(&b) {
                for (x, y) in sa.iter().zip(sb) {
                    assert_eq!(x.to_bits(), y.to_bits(), "{}", system.name());
                }
            }
            assert_eq!(
                corpus(system, ParamRange::Train, 42, 2, 50),
                corpus(system, ParamRange::Train, 42, 2, 50),
                "{}",
                system.name()
            );
        }
    }

    #[test]
    fn train_and_heldout_ranges_are_disjoint_over_1000_samples() {
        for system in SYSTEMS {
            let train = system.bounds(ParamRange::Train);
            let held = system.bounds(ParamRange::HeldOut);
            assert_eq!(train.len(), system.dims().len());
            for (i, (&(tl, th), &(hl, hh))) in train.iter().zip(&held).enumerate() {
                assert!(
                    th < hl || hh < tl,
                    "{} dimension {} overlaps",
                    system.name(),
                    system.dims()[i]
                );
            }
            let mut rng = Rng(9);
            let mut train_e = f64::NEG_INFINITY;
            let mut held_e = f64::INFINITY;
            for _ in 0..1000 {
                for (range, bounds) in [(ParamRange::Train, &train), (ParamRange::HeldOut, &held)] {
                    let (p, s) = sample_params(system, range, &mut rng);
                    for (v, &(lo, hi)) in drawn_values(&p, &s).iter().zip(bounds.iter()) {
                        assert!(lo - 1e-12 <= *v && *v <= hi + 1e-12, "{}", system.name());
                    }
                    // Initial energies must also separate, where they are
                    // defined by amplitude alone.
                    if matches!(
                        system,
                        System::Pendulum | System::Spring | System::Bounce
                    ) {
                        let e = energy(&p, &s);
                        match range {
                            ParamRange::Train => train_e = train_e.max(e),
                            ParamRange::HeldOut => held_e = held_e.min(e),
                        }
                    }
                }
            }
            if matches!(system, System::Pendulum | System::Spring | System::Bounce) {
                assert!(
                    train_e < held_e,
                    "{} initial energies overlap: {train_e} vs {held_e}",
                    system.name()
                );
            }
        }
    }

    #[test]
    fn encode_decode_round_trips_each_value_within_half_a_quantum() {
        for system in SYSTEMS {
            let (p, states) = trajectory(system, 7, 200);
            let q = system.default_quantum();
            let bytes = encode(system, &p, &states, q);
            let (s2, p2, back, q2) = decode(&bytes).expect("decodes");
            assert_eq!(s2, system);
            assert_eq!(p2, p);
            assert_eq!(q2.to_bits(), q.to_bits());
            assert_eq!(back.len(), states.len());
            for (orig, got) in states.iter().zip(&back) {
                for (c, (&x, &y)) in orig.iter().zip(got).enumerate() {
                    let want = if system.angle_channels().contains(&c) {
                        wrap_angle(x)
                    } else {
                        x
                    };
                    assert!(
                        (y - want).abs() <= q / 2.0 + 1e-12,
                        "{} channel {c}: {y} vs {want}",
                        system.name()
                    );
                }
            }
        }
    }

    /// Asserts that `encoding` decodes to exactly the values the i32 encoding
    /// decodes to, bit for bit, and that its header and length are as documented.
    fn check_exact(system: System, p: &Params, states: &[State], encoding: Encoding) {
        let bytes = encode_as(system, p, states, encoding);
        let (s2, p2, back, q2) = decode_as(&bytes, encoding).expect("decodes");
        assert_eq!(s2, system);
        assert_eq!(&p2, p);
        assert_eq!(q2.to_bits(), encoding.quantum(system).to_bits());
        assert_eq!(encoded_len_as(&bytes, encoding).expect("len"), bytes.len());
        let want = decode(&encode(system, p, states, system.default_quantum())).expect("i32 decodes").2;
        assert_eq!(back.len(), want.len());
        for (a, b) in back.iter().zip(&want) {
            for (x, y) in a.iter().zip(b) {
                assert_eq!(x.to_bits(), y.to_bits(), "{} {}", system.name(), encoding.name());
            }
        }
    }

    /// The header length of every encoding of `system`: magic, system code,
    /// parameters, quantum, channel count and step count.
    fn header_len(system: System) -> usize {
        4 + 1 + 8 * system.param_count() + 8 + 4 + 4
    }

    #[test]
    fn encoding_names_round_trip() {
        for e in Encoding::ALL {
            assert_eq!(Encoding::from_name(e.name()), Some(e));
        }
        assert_eq!(Encoding::from_name("i16"), None);
        assert_eq!(Encoding::from_name(""), None);
    }

    #[test]
    fn i32_encode_as_is_the_plan_p2_layout_unchanged() {
        for system in SYSTEMS {
            let (p, states) = trajectory(system, 5, 50);
            assert_eq!(
                encode_as(system, &p, &states, Encoding::I32),
                encode(system, &p, &states, system.default_quantum()),
                "{}",
                system.name()
            );
        }
    }

    #[test]
    fn planes_round_trip_exactly_keep_the_header_and_group_byte_zero_first() {
        for system in SYSTEMS {
            let (p, states) = trajectory(system, 7, 200);
            check_exact(system, &p, &states, Encoding::Planes);
            let plain = encode(system, &p, &states, system.default_quantum());
            let planes = encode_as(system, &p, &states, Encoding::Planes);
            let hl = header_len(system);
            assert_eq!(planes[..hl], plain[..hl], "{} header", system.name());
            assert_eq!(planes.len(), plain.len());
            // Byte 0 of every value comes first, in stream order.
            let n = states.len() * system.channels();
            for i in 0..n {
                assert_eq!(planes[hl + i], plain[hl + 4 * i], "{} value {i}", system.name());
            }
        }
    }

    #[test]
    fn diff2_round_trips_exactly_to_the_i32_values() {
        for system in SYSTEMS {
            let (p, states) = trajectory(system, 7, 200);
            check_exact(system, &p, &states, Encoding::Diff2);
        }
    }

    #[test]
    fn diff2_wraps_and_still_round_trips_values_past_the_i32_range() {
        // Values past the i32 range saturate, and their second differences then
        // overflow, so this exercises the wrapping arithmetic on both sides.
        let p = Params::Spring {
            k: 2.0,
            m: 1.0,
            c: 0.0,
        };
        let states = vec![
            vec![1e30, -1e30],
            vec![-1e30, 1e30],
            vec![1e30, 0.0],
            vec![0.0, -1e30],
            vec![3.0, 4.0],
        ];
        check_exact(System::Spring, &p, &states, Encoding::Diff2);
    }

    #[test]
    fn diff2_of_constant_velocity_is_zero_after_the_first_two_samples() {
        // x = 1e-4 (3 + 5t) and v = 1e-4 (-2 + 7t) are linear in t, so at the
        // spring's 1e-5 quantum each channel is an exact arithmetic sequence.
        let p = Params::Spring {
            k: 2.0,
            m: 1.0,
            c: 0.0,
        };
        let states: Vec<State> = (0..20)
            .map(|t| vec![1e-4 * (3.0 + 5.0 * t as f64), 1e-4 * (-2.0 + 7.0 * t as f64)])
            .collect();
        let bytes = encode_as(System::Spring, &p, &states, Encoding::Diff2);
        let hl = header_len(System::Spring);
        let ch = System::Spring.channels();
        for i in 2 * ch..states.len() * ch {
            assert_eq!(&bytes[hl + 4 * i..hl + 4 * i + 4], &[0, 0, 0, 0], "residual {i}");
        }
        check_exact(System::Spring, &p, &states, Encoding::Diff2);
    }

    #[test]
    fn coarse_round_trips_within_its_quantum() {
        for system in SYSTEMS {
            let (p, states) = trajectory(system, 7, 200);
            let q = Encoding::Coarse.quantum(system);
            let bytes = encode_as(system, &p, &states, Encoding::Coarse);
            let (s2, p2, back, q2) = decode_as(&bytes, Encoding::Coarse).expect("decodes");
            assert_eq!(s2, system);
            assert_eq!(p2, p);
            assert_eq!(q2.to_bits(), q.to_bits());
            assert_eq!(encoded_len_as(&bytes, Encoding::Coarse).expect("len"), bytes.len());
            assert_eq!(bytes.len(), header_len(system) + 2 * states.len() * system.channels());
            for (orig, got) in states.iter().zip(&back) {
                for (c, (&x, &y)) in orig.iter().zip(got).enumerate() {
                    let want = if system.angle_channels().contains(&c) { wrap_angle(x) } else { x };
                    assert!((y - want).abs() <= q / 2.0 + 1e-12, "{} channel {c}: {y} vs {want}", system.name());
                }
            }
        }
    }

    #[test]
    fn coarse_saturates_instead_of_wrapping() {
        let p = Params::Spring {
            k: 2.0,
            m: 1.0,
            c: 0.0,
        };
        let q = Encoding::Coarse.quantum(System::Spring);
        let states = vec![vec![1e6, -1e6]];
        let (_, _, back, _) = decode_as(&encode_as(System::Spring, &p, &states, Encoding::Coarse), Encoding::Coarse)
            .expect("decodes");
        assert_eq!(back[0][0].to_bits(), (i16::MAX as f64 * q).to_bits());
        assert_eq!(back[0][1].to_bits(), (i16::MIN as f64 * q).to_bits());
    }

    #[test]
    fn angles_outside_minus_pi_to_pi_are_wrapped_on_encode() {
        let p = Params::DoublePendulum {
            g: GRAVITY,
            l1: 1.0,
            l2: 1.0,
            m1: 1.0,
            m2: 1.0,
        };
        let s = vec![vec![3.5, -4.0, 2.0, -1.0]];
        let (_, _, back, q) = decode(&encode(System::DoublePendulum, &p, &s, 1e-4)).expect("ok");
        assert!((back[0][0] - wrap_angle(3.5)).abs() <= q / 2.0);
        assert!((back[0][1] - wrap_angle(-4.0)).abs() <= q / 2.0);
        assert!(back[0][0] > -PI && back[0][0] <= PI);
        assert!(back[0][1] > -PI && back[0][1] <= PI);
        // Non-angle channels are not wrapped.
        assert!((back[0][2] - 2.0).abs() <= q / 2.0);
    }

    #[test]
    fn header_round_trips_exactly() {
        let p = Params::NBody {
            g: NBODY_G,
            softening: NBODY_SOFTENING,
            m: [0.7, 1.1, 0.9],
        };
        let s = vec![vec![0.0; 12]; 3];
        let bytes = encode(System::NBody, &p, &s, 1e-4);
        assert_eq!(&bytes[..4], b"PHY1");
        assert_eq!(bytes[4], System::NBody.code());
        let (h, len) = read_header(&bytes).expect("header");
        assert_eq!(h.system, System::NBody);
        assert_eq!(h.params, p);
        assert_eq!(h.quantum.to_bits(), 1e-4f64.to_bits());
        assert_eq!(h.channels, 12);
        assert_eq!(h.steps, 3);
        assert_eq!(len, 4 + 1 + 8 * 5 + 8 + 4 + 4);
        assert_eq!(encoded_len(&bytes).expect("len"), bytes.len());
    }

    #[test]
    fn a_corrupted_magic_is_refused() {
        let (p, states) = trajectory(System::Bounce, 1, 10);
        let mut bytes = encode(System::Bounce, &p, &states, 1e-5);
        assert!(decode(&bytes).is_ok());
        bytes[0] ^= 0xFF;
        let err = decode(&bytes).expect_err("must refuse");
        assert!(err.contains("bad magic"), "{err}");
    }

    #[test]
    fn truncated_and_padded_input_is_refused() {
        let (p, states) = trajectory(System::Pendulum, 1, 10);
        let bytes = encode(System::Pendulum, &p, &states, 1e-5);
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
        let mut padded = bytes.clone();
        padded.push(0);
        assert!(decode(&padded).is_err());
        assert!(decode(&bytes[..10]).is_err());
    }

    #[test]
    fn corpus_is_a_stream_of_whole_trajectories() {
        let bytes = corpus(System::Spring, ParamRange::Train, 3, 4, 50);
        let mut at = 0;
        let mut n = 0;
        while at < bytes.len() {
            let len = encoded_len(&bytes[at..]).expect("header");
            let (system, _, states, _) = decode(&bytes[at..at + len]).expect("trajectory");
            assert_eq!(system, System::Spring);
            assert_eq!(states.len(), 51);
            at += len;
            n += 1;
        }
        assert_eq!(n, 4);
    }

    #[test]
    fn quanta_keep_every_value_inside_i32_with_headroom() {
        // Largest tolerated |value / quantum|: 2^31 / 256.
        let limit = 2f64.powi(31) / 256.0;
        for system in SYSTEMS {
            for range in [ParamRange::Train, ParamRange::HeldOut] {
                let mut rng = Rng(11);
                let mut worst = 0.0f64;
                for _ in 0..50 {
                    let (p, s) = sample_params(system, range, &mut rng);
                    for st in simulate(system, &p, s, system.default_dt(), 1_000) {
                        for x in st {
                            worst = worst.max(x.abs() / system.default_quantum());
                        }
                    }
                }
                assert!(worst < limit, "{} {range:?}: {worst:e}", system.name());
            }
        }
    }

    /// The LZ bits/byte of the first `n` bytes of each system's corpus, and of
    /// `n` random bytes, which is the baseline.
    fn lz_table(n: usize) -> (f64, Vec<(System, f64)>) {
        let mut rng = Rng(0x5EED);
        let mut random = Vec::with_capacity(n + 8);
        while random.len() < n {
            random.extend_from_slice(&rng.next_u64().to_le_bytes());
        }
        random.truncate(n);
        let r = lz_bits_per_byte(&random);
        let rows = SYSTEMS
            .iter()
            .map(|&system| {
                let bytes = corpus(system, ParamRange::Train, 1, 8, 1_100);
                assert!(bytes.len() >= n, "{} corpus too short", system.name());
                (system, lz_bits_per_byte(&bytes[..n]))
            })
            .collect();
        (r, rows)
    }

    /// The fast check: even at 8 KiB, every system is more compressible than
    /// random bytes.
    #[test]
    fn lz_bits_per_byte_puts_every_system_below_random_bytes() {
        let (r, rows) = lz_table(8_192);
        // The LZ76 estimate is biased low on short inputs, even for random
        // bytes, so the random value is the baseline rather than 8.
        assert!((6.0..=8.0).contains(&r), "random LZ bits/byte {r}");
        for (system, lz) in rows {
            assert!(lz < r, "{} ({lz}) is no more compressible than random ({r})", system.name());
        }
    }

    /// The recorded measurement: 64 KiB per system, against random. Slow in a
    /// debug build (LZ76 on 64 KiB takes about 20 s), so it is run on demand:
    /// `cargo test -- --ignored --nocapture lz_bits_per_byte_64_kib`.
    #[test]
    #[ignore]
    fn lz_bits_per_byte_64_kib() {
        let (r, rows) = lz_table(65_536);
        eprintln!("random bytes: {r:.4}");
        for (system, lz) in rows {
            eprintln!("{:>15}: {lz:.4}", system.name());
        }
    }

    #[test]
    fn a_physics_spec_parses_and_each_bad_field_is_named() {
        assert_eq!(
            parse_spec("pendulum:8:1100:1"),
            Ok(PhysicsSpec { system: System::Pendulum, trajectories: 8, steps: 1100, seed: 1 })
        );
        assert_eq!(parse_spec("double_pendulum:2:50:0").map(|s| s.system), Ok(System::DoublePendulum));
        assert_eq!(parse_spec("bounce:1:1:18446744073709551615").map(|s| s.seed), Ok(u64::MAX));
        // The hyphenated `System::name` is not a spec name.
        for bad in ["double-pendulum:2:50:1", "foo:2:50:1"] {
            let e = parse_spec(bad).expect_err(bad);
            assert!(e.contains("unknown system"), "{bad}: {e}");
        }
        assert!(parse_spec("pendulum:8:1100").unwrap_err().contains("3 fields"));
        assert!(parse_spec("pendulum:8:1100:1:9").unwrap_err().contains("5 fields"));
        assert!(parse_spec("pendulum:0:1100:1").unwrap_err().contains("trajectories must be at least 1"));
        assert!(parse_spec("pendulum:8:0:1").unwrap_err().contains("steps must be at least 1"));
        assert!(parse_spec("pendulum:eight:1100:1").unwrap_err().contains("trajectories `eight`"));
        assert!(parse_spec("pendulum:8:1100:-1").unwrap_err().contains("seed `-1`"));
        assert!(physics_corpus("nbody:-2:50:1").is_err(), "a negative count is refused");
    }

    #[test]
    fn the_same_physics_spec_gives_the_same_corpus_and_a_new_seed_a_new_one() {
        let a = physics_corpus("pendulum:2:200:1").expect("valid");
        let b = physics_corpus("pendulum:2:200:1").expect("valid");
        let c = physics_corpus("pendulum:2:200:2").expect("valid");
        assert_eq!(a.sha256(), b.sha256(), "the same spec, the same bytes");
        assert_ne!(a.sha256(), c.sha256(), "a different seed, different bytes");
        assert_eq!(a.name(), "physics:pendulum:2:200:seed1:planes");
        assert_eq!(a.len(), corpus(System::Pendulum, ParamRange::HeldOut, 1, 2, 200).len());
    }

    #[test]
    fn the_suite_digest_covers_a_physics_corpus() {
        use crate::succession::suite_digest;
        let text = Corpus::new("text", b"the quick brown fox".to_vec());
        let physics = physics_corpus("spring:2:100:1").expect("valid");
        assert_ne!(suite_digest(std::slice::from_ref(&text)), suite_digest(&[text.clone(), physics.clone()]));
        assert_ne!(suite_digest(std::slice::from_ref(&physics)), suite_digest(&[physics_corpus("spring:2:100:2").unwrap()]));
    }

    fn tiny_run_cfg() -> crate::arena::Config {
        crate::arena::Config {
            rounds: 4,
            programs_per_round: 8,
            learners: vec![LearnerSpec::Ngram(crate::learner::LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 })],
            eval_every: 2,
            replay_per_round: 2,
            ..crate::arena::Config::default()
        }
    }

    #[test]
    fn a_run_on_physics_held_out_data_alone_reports_it_at_every_checkpoint() {
        use crate::energy::WallClock;
        let meters: Vec<Box<dyn crate::energy::Meter>> = vec![Box::new(WallClock::new("cpu-estimate", 65.0))];
        let heldout = vec![physics_corpus("pendulum:4:300:1").expect("valid")];
        let report = crate::arena::run(tiny_run_cfg(), heldout, meters, vec![]).expect("runs on physics alone");
        assert!(!report.checkpoints.is_empty());
        for c in &report.checkpoints {
            let keys: Vec<&String> = c.heldout_bpb.keys().collect();
            assert_eq!(keys, vec!["physics:pendulum:4:300:seed1:planes"], "checkpoint at round {}", c.round);
            assert!(c.heldout_bpb.values().all(|b| b.is_finite()));
        }
    }

    #[test]
    fn order0_is_the_byte_frequency_entropy() {
        assert_eq!(order0_bits_per_byte(&[7; 16]), 0.0, "a constant window has no entropy");
        assert!((order0_bits_per_byte(&[0, 1, 0, 1]) - 1.0).abs() < 1e-12, "two equal bytes are one bit each");
        assert!((order0_bits_per_byte(&(0..=255).collect::<Vec<u8>>()) - 8.0).abs() < 1e-12, "all bytes equally: eight bits");
        assert!(order0_bits_per_byte(&[]).is_nan());
    }

    #[test]
    fn a_physics_baseline_is_finite_and_trains_below_uniform() {
        let spec = LearnerSpec::Ngram(crate::learner::LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 });
        // Ten steps of 4 KiB each: the smallest budget the curve accepts.
        let budget = BASELINE_CHUNK * BASELINE_BATCH * BASELINE_CHECKPOINTS;
        let points = physics_baseline(System::Pendulum, spec, budget, 200, 1).expect("runs").points;
        assert_eq!(points.len(), BASELINE_CHECKPOINTS + 1);
        assert!((points[0].train_bpb - 8.0).abs() < 1e-9, "untrained is uniform, not {}", points[0].train_bpb);
        let last = points.last().expect("points");
        assert!(last.train_bpb.is_finite() && last.heldout_bpb.is_finite());
        assert!(last.train_bpb < 8.0, "train bpb {} is not below uniform", last.train_bpb);
        assert!(last.bytes_trained as usize >= budget);
    }

    #[test]
    fn a_curve_records_cumulative_train_seconds_at_every_checkpoint() {
        // Plan L3: the seconds axis. Zero before any training, then never falling,
        // and the last value is the run's whole training time.
        let spec = LearnerSpec::Ngram(crate::learner::LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 });
        let budget = BASELINE_CHUNK * BASELINE_BATCH * BASELINE_CHECKPOINTS;
        let points = physics_baseline(System::Pendulum, spec, budget, 200, 1).expect("runs").points;
        assert!(points[0].seconds < 1e-2, "nothing is trained at the start, so no time is spent: {}", points[0].seconds);
        assert!(points.windows(2).all(|w| w[1].seconds >= w[0].seconds), "seconds fall: {points:?}");
        let last = points.last().expect("points");
        assert!(last.seconds > 0.0 && last.seconds.is_finite(), "{}", last.seconds);
    }
}
