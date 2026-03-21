use std::io::{self, Read};

fn main() {
    let mut input = String::new();

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        // Read from file
        input = std::fs::read_to_string(&args[1]).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {e}", args[1]);
            std::process::exit(1);
        });
    } else {
        // Read from stdin
        io::stdin().read_to_string(&mut input).unwrap_or_else(|e| {
            eprintln!("Error reading stdin: {e}");
            std::process::exit(1);
        });
    }

    print!("{}", rust2redox::transpile(&input));
}
