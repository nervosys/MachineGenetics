// `mage-parse --heal` example: a failing @test, debugged and repaired.
//
// `t_eighteen` fails: `age > 18` excludes 18. The debugger reports the failed
// assertion with a call trace, then tries single-site mutations of `adult` and
// accepts `>` -> `>=`, the first that makes both tests pass. The repair changes
// what `adult` computes and is reported that way; the file is rewritten only
// with `--write`.

f adult(age: i64) -> bool {
    if age > 18 { 1b } else { 0b }
}

@t
f t_eighteen() {
    assert(adult(18), "18 is adult")
}

@t
f t_ten() {
    assert(adult(10) == 0b, "10 is not")
}
