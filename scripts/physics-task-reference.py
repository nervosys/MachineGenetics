#!/usr/bin/env python3
"""Reference trajectories for the physics program task (IMPLEMENTATION_PLAN.md C3).

The system is an undamped unit-mass spring, x'' = -k x. Its exact solution is

    x(t) = x0 cos(w t) + (v0 / w) sin(w t),    w = sqrt(k),

so the reference is the true trajectory, not another integrator's output, and
a candidate's error is its own error and nothing else's.

Each held-out case is (k, x0, v0), sampled at t = dt, 2 dt, ..., n dt. This
prints the MAGE `val` lines that `prototype/examples/physics_task/task.mg.in`
embeds; paste them over the ones there after changing a case.

    python scripts/physics-task-reference.py
"""

import math

DT = 0.05
N = 100
# (name, k, x0, v0). w dt is 0.1 and 0.075: small enough that a symplectic
# integrator's energy error stays a few percent, and large enough that
# explicit Euler's per-step gain (1 + w^2 dt^2) crosses 10% within 20 steps.
CASES = [
    ("ref_a", 4.0, 1.0, 0.0),
    ("ref_b", 2.25, 0.0, 1.5),
]


def trajectory(k, x0, v0):
    w = math.sqrt(k)
    return [x0 * math.cos(w * i * DT) + v0 / w * math.sin(w * i * DT) for i in range(1, N + 1)]


def main():
    for name, k, x0, v0 in CASES:
        xs = trajectory(k, x0, v0)
        body = ", ".join(f"{x:.9f}" for x in xs)
        print(f"    // k = {k}, x0 = {x0}, v0 = {v0}, dt = {DT}, {N} samples")
        print(f"    val {name} = [{body}];")


if __name__ == "__main__":
    main()
