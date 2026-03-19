use std::env;

fn main() {
    println!("cargo::redox-env=HOST={}", env::var("HOST").unwrap());
    println!("cargo::redox-env=TARGET={}", env::var("TARGET").unwrap());
}
