// hello.mg — minimal MAGE example
//
// Demonstrates:
//   - Value binding (val)
//   - Mutable variable (var)
//   - Expression-body functions

// `/ io` is required: `io.println` is a capability handle, and performing a
// capability from a `pub` function means declaring it.
pub fn main() / io {
    // Immutable value binding.
    val greeting: String = "Hello, MAGE!";
    io.println(greeting);

    // Mutable variable.
    var counter: i32 = 0;
    counter = counter + 1;
    io.println(counter);

    io.println("Goodbye!");
}

// Expression-body function: single expression after `=`.
fn square(x: i32) -> i32 = x * x
