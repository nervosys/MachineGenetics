use std::io::{self, Read};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut mode = Mode::Compact;
    let mut file_arg = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--compact" => mode = Mode::Compact,
            "--expand" => mode = Mode::Expand,
            arg if !arg.starts_with('-') => file_arg = Some(arg.to_string()),
            other => {
                eprintln!("Unknown option: {other}");
                eprintln!("Usage: redoxfmt [--compact|--expand] [FILE]");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let input = if let Some(path) = file_arg {
        std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("Error reading {path}: {e}");
            std::process::exit(1);
        })
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).unwrap_or_else(|e| {
            eprintln!("Error reading stdin: {e}");
            std::process::exit(1);
        });
        buf
    };

    let output = match mode {
        Mode::Compact => redoxfmt::compact(&input),
        Mode::Expand => redoxfmt::expand_source(&input),
    };

    print!("{output}");
}

enum Mode {
    Compact,
    Expand,
}
