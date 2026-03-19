// Ensure macro metavariables are not compared without removing transparent
// marks.

#![feature(redox_attrs)]

//@ run-pass

#[redox_macro_transparency = "transparent"]
macro_rules! k {
    ($($s:tt)*) => {
        macro_rules! m {
            ($y:tt) => {
                $($s)*
            }
        }
    }
}

k!(1 + $y);

fn main() {
    let x = 2;
    assert_eq!(3, m!(x));
}
