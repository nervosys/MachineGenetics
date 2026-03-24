# CLI & Tooling

---

### Parse command-line arguments

**Problem**: Accept flags and positional arguments.

**Solution**:

```mg
u std.env

S Args {
    verbose: bool,
    output: ?s,
    files: [s]~,
}

+f parse_args() -> Args {
    v raw = env.args()
    m verbose = 0b
    m output: ?s = None
    m files = [s]~.new()
    m iter = raw.iter().skip(1)  // skip program name

    loop {
        ? iter.next() => Some(arg) {
            ? arg.as_str() {
                "-v" | "--verbose" => verbose = 1b,
                "-o" | "--output" => {
                    output = iter.next().map(|s| s.clone())
                },
                _ => files.push(arg.clone()),
            }
        } : { break }
    }

    Args @{ verbose, output, files }
}

+f main() / io {
    v args = parse_args()
    ? args.verbose { p"Verbose mode on" }
    ? args.output => Some(o) { p"Output: {o}" }
    p"Files: {args.files.len()}"
}
```

---

### Progress indicator

**Problem**: Show a progress bar for a long-running operation.

**Solution**:

```mg
u std.io.{stdout, Write}
u std.time.Duration

+f progress_bar(current: usize, total: usize) / io {
    v pct = (current * 100) / total
    v filled = pct / 2
    v empty = 50 - filled

    m bar = s.new()
    @ _ : 0..filled { bar.push('█') }
    @ _ : 0..empty { bar.push('░') }

    v out = stdout()
    out.write(f"\r[{bar}] {pct}%")?
    out.flush()?

    ? current == total {
        out.write("\n")?
    }
}

+f main() / io {
    v total = 100
    @ i : 0..=total {
        progress_bar(i, total)
        sleep(Duration.from_millis(50))
    }
}
```

---

### Read user input

**Problem**: Prompt the user and read their response.

**Solution**:

```mg
u std.io.{stdin, stdout, Write}

+f prompt(question: &s) -> R[s, Error] / io {
    stdout().write(f"{question} ")?
    stdout().flush()?
    v answer = stdin().read_line()?
    Ok(answer.trim().to_string())
}

+f main() / io {
    v name = prompt("What is your name?")?
    v age = prompt("How old are you?")?.parse[u32]()?
    p"Hello {name}, you are {age} years old!"
}
```

---

### Run an external command

**Problem**: Execute a shell command and capture its output.

**Solution**:

```mg
u std.process.Command

+f main() / io, process {
    v output = Command.new("git")
        .args(&["log", "--oneline", "-5"])
        .capture()?

    ? output.success() {
        p"Git log:\n{output.stdout()}"
    } : {
        p"Error: {output.stderr()}"
    }
}
```

---

### REPL (Read-Eval-Print Loop)

**Problem**: Build an interactive command loop.

**Solution**:

```mg
u std.io.{stdin, stdout, Write}

E Command {
    Add(f64, f64),
    Mul(f64, f64),
    Quit,
    Unknown(s),
}

f parse_command(input: &s) -> Command {
    v parts: [&s]~ = input.trim().split_whitespace().collect()
    ? parts.len() {
        0 => Command.Unknown("".into()),
        _ => {
            v cmd = parts[0]
            ? cmd {
                "add" => {
                    v a = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0)
                    v b = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0)
                    Command.Add(a, b)
                },
                "mul" => {
                    v a = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0)
                    v b = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0)
                    Command.Mul(a, b)
                },
                "quit" | "exit" => Command.Quit,
                other => Command.Unknown(other.to_string()),
            }
        },
    }
}

+f main() / io {
    p"Calculator REPL. Commands: add <a> <b>, mul <a> <b>, quit"

    loop {
        stdout().write("> ")?
        stdout().flush()?
        v line = stdin().read_line()?

        ? parse_command(&line) {
            Command.Add(a, b) => p"= {a + b}",
            Command.Mul(a, b) => p"= {a * b}",
            Command.Quit => { p"Bye!"; break },
            Command.Unknown(s) => p"Unknown command: {s}",
        }
    }
}
```

---

### Key-value config parser

**Problem**: Parse a simple `key = value` configuration file.

**Solution**:

```mg
u std.fs

+f parse_config(path: &s) -> R[{s: s}, Error] / io {
    v content = fs.read(path)?
    m config: {s: s} = {s: s}.new()

    @ line : content.lines() {
        v line = line.trim()
        // Skip comments and empty lines
        ? line.is_empty() || line.starts_with('#') { continue }

        ? line.split_once('=') => Some((key, value)) {
            config.insert(
                key.trim().to_string(),
                value.trim().to_string(),
            )
        }
    }
    Ok(config)
}

+f main() / io {
    v config = parse_config("app.conf")?
    ? config.get("port") => Some(port) {
        p"Port: {port}"
    }
}
```
