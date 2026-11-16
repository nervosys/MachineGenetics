// `mage-parse --heal` example: a compile error healed under the program's intent.
//
// `len` returns a `usize` and `count` promises an `i64`. The heal loop accepts
// the `as i64` cast because it removes the error, keeps every signature, and
// `t_twice`, which passed before, still passes. Changing the return type to
// `usize` would also remove the error; it is refused because it changes the
// signature.

f count(xs: [i64]) -> i64 {
    len(xs)
}

f twice(n: i64) -> i64 {
    n * 2
}

@t
f t_twice() {
    assert(twice(3) == 6, "twice")
}
