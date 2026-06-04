// effects.mg — effect definitions, handlers, closures
//
// Demonstrates:
//   - Effect declarations
//   - Error union types (T or Error)
//   - guard for early exit
//   - val bindings
//   - Pipeline operator (|>)

effect io {
    fn read(fd: i32) -> [u8]~;
    fn write(fd: i32, data: &[u8]) -> i32;
}

effect async {
    fn suspend() -> ();
}

// Error union: `i32 or Error` replaces Result<i32, Error>.
pub fn process_data(input: &[u8]) -> i32 or Error {
    var result = 0;
    for byte in input {
        // guard for early-exit on invalid data.
        guard byte <= 127 else {
            return Err(Error.new("invalid byte"));
        }
        result = result + byte;
    }
    Ok(result)
}

fn transform[T, U](items: [T]~, mapper: fn(T) -> U) -> [U]~ {
    var out = [U]~.new();
    for item in items {
        out.push(mapper(item));
    }
    out
}

fn example() {
    // Pipeline: chain transformations left-to-right.
    val result = [1, 2, 3]
        |> transform(|x| x * 2)
        |> transform(|x| x + 1);
}
