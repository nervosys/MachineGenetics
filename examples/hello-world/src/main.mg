// hello-world — a minimal, runnable MAGE program.
//
// `forge run` evaluates `main` and prints its result. This `main` returns a
// formatted greeting (the evaluator has no side-effecting I/O yet), showing:
//   - value bindings (val) and f-string interpolation `f"…{expr}…"`
//   - the standard vocabulary (len) over a string
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

f main() {
    val name = "MAGE"
    val n = len(name)
    f"Hello, {name}! (your name has {n} letters)"
}
