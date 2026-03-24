// hello-world — Minimal MechGen program.
//
// Demonstrates:
//   - Entry point (pub fn main)
//   - Variable binding (let)
//   - String type (String)
//   - Print macro (println!("..."))
//   - Format strings (format!("..."))

pub fn main() {
    // Simple print.
    println!("Hello, MechGen!");

    // Variable binding and format string.
    let name: String = "World";
    let greeting: String = format!("Hello, {name}!");
    println!("{greeting}");

    // Mutable variable.
    let mut counter: i32 = 0;
    counter = counter + 1;
    println!("Counter: {counter}");

    // Boolean literals.
    let is_mechgen: bool = true;
    let is_legacy: bool = false;
    println!("is_mechgen={is_mechgen}, is_legacy={is_legacy}");

    // Return value (implicit — last expression).
    println!("Goodbye!");
}
