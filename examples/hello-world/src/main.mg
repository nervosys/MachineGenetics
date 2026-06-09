// hello-world — Minimal MechGen program.
//
// Demonstrates:
//   - Entry point (pub fn main)
//   - Value binding (val) — replaces `let`
//   - Mutable variable (var) — replaces `val mut`
//   - String type (String)
//   - Print macro (println!("..."))
//   - Format strings (format!("..."))

pub fn main() {
    // Simple print.
    println!("Hello, MechGen!");

    // Value binding: immutable by default.
    val name: String = "World";
    val greeting: String = format!("Hello, {name}!");
    println!("{greeting}");

    // Mutable variable.
    var counter: i32 = 0;
    counter = counter + 1;
    println!("Counter: {counter}");

    // Boolean literals.
    val is_mechgen: bool = true;
    val is_legacy: bool = false;
    println!("is_mechgen={is_mechgen}, is_legacy={is_legacy}");

    // Return value (implicit — last expression).
    println!("Goodbye!");
}
