use std::collections::HashSet;

fn fact(n: u64) -> u64 {
    if n < 2 { 1 } else { n * fact(n - 1) }
}

fn sumto(n: u64) -> u64 {
    let mut t = 0;
    for i in 1..=n {
        t += i;
    }
    t
}

fn fib(n: u64) -> u64 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}

fn distinct() -> usize {
    let words = ["the", "quick", "brown", "the", "lazy", "the", "fox"];
    words.iter().collect::<HashSet<_>>().len()
}

fn collatz(n: u64) -> u64 {
    let mut x = n;
    let mut s = 0;
    while x != 1 {
        x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
        s += 1;
    }
    s
}

fn main() {
    println!("{}", fact(12));
    println!("{}", sumto(100));
    println!("{}", fib(25));
    println!("{}", distinct());
    println!("{}", collatz(27));
}
