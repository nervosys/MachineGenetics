// hello.mg — minimal MechGen example
//
// Demonstrates:
//   - Value binding (val)
//   - Mutable variable (var)
//   - Expression-body functions

pub fn main() {
    // Immutable value binding.
    val greeting: String = "Hello, MechGen!";
    io.println(greeting);

    // Mutable variable.
    var counter: i32 = 0;
    counter = counter + 1;
    io.println(counter);

    io.println("Goodbye!");
}

// Expression-body function: single expression after `=`.
fn square(x: i32) -> i32 = x * x
