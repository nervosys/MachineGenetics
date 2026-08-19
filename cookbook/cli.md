# CLI & Terminal

> Recipes for command-line programs. Agent-mode syntax; every block was
> verified with `mage-parse --check`.

The previous version of this page was written against a standard library that
does not exist — `u std.io.{stdin, stdout, Write}`, `Command.new("git")`,
`parts.iter().skip(1)`, `.parse[u32]()`, `s.new()`. There is no module system
and nothing to import: `env`, `io`, `process` and `time` are **capability
namespaces**, in scope everywhere, and reaching one puts its effect in the
function's inferred set.

---

### Parse command-line arguments

**Problem**: Accept flags and positional arguments.

**Solution**:

```MAGE
+S Args { verbose: bool, output: s, files: [s]~ }

// `env.args()` performs `env`. There is no iterator with `.next()`, so walk
// the list by index and read the value that follows a flag.
+f parse_args() -> Args / env {
    v raw: [s]~ = env.args()
    m verbose = 0b
    m output = ""
    m files: [s]~ = []

    @ i in range(len(raw)) {
        v arg = raw[i]
        ?= arg {
            "-v" => verbose = 1b,
            "--verbose" => verbose = 1b,
            "-o" => output = raw[i + 1],
            _ => files = flatten([files, [arg]]),
        }
    }

    @Args { verbose: verbose, output: output, files: files }
}

+f main() -> i32 / env, io {
    v args = parse_args()
    ? args.verbose { p"verbose mode on" }
    p"files: {len(args.files)}"
    0
}
```

**Discussion**: `env.args()` performs `env`. There is no iterator protocol — no `.iter()`, `.next()` or `.skip()` — so walk the list by index. `flatten([xs, [x]])` is how you append.

---

### Progress indicator

**Problem**: Show progress for a long-running operation.

**Solution**:

```MAGE
// A progress bar is string building: `join` over a list of blocks. There is
// no `stdout()` handle, no `write`/`flush` pair, and no `\r` redraw — `p"…"`
// prints a line.
f bar(current: i32, total: i32) -> s {
    v pct = (current * 100) / total
    v filled = pct / 2
    v blocks = map(range(filled as usize), |n| "#")
    v gaps = map(range((50 - filled) as usize), |n| ".")
    f"[{join(blocks, "")}{join(gaps, "")}] {pct}%"
}

+f main() -> i32 / io, time {
    @ i in range(101) {
        p"{bar(i as i32, 100)}"
        time.sleep(1)
    }
    0
}
```

**Discussion**: There is no `stdout()` handle and no `flush`, so a bar is a string you build with `join` and print. `time.sleep` performs `time`, a capability of its own.

---

### Read user input

**Problem**: Prompt the user and read the response.

**Solution**:

```MAGE
// `io.read_line` is the console capability. There is no `stdout().flush()`,
// and no `.parse[u32]()` — a line stays a string until you convert it.
+f prompt(question: s) -> s / io {
    p"{question} "
    io.read_line()
}

+f main() -> i32 / io {
    v name = prompt("What is your name?")
    v age = prompt("How old are you?")
    p"Hello {name}, you said {age}"
    0
}
```

**Discussion**: `io.read_line` is the console capability. A line stays a string: there is no `.parse[u32]()`, and no `?` to convert a parse failure into your error type.

---

### Run an external command

**Problem**: Execute a command and capture its output.

**Solution**:

```MAGE
// `process` is the capability namespace; its effect kind is `proc` — the two
// spellings differ, and `/ process` is not a valid annotation.
+f run_git() -> s / proc, io {
    v output: s = process.spawn("git log --oneline -5")
    p"git log:\n{output}"
    output
}
```

**Discussion**: **The namespace and the effect have different names.** The capability is `process`; the effect kind it performs is `proc`, and `/ process` is rejected as an unknown effect. This is the one place in the language where the two spellings differ.

---

### REPL (Read-Eval-Print Loop)

**Problem**: Build an interactive command loop.

**Solution**:

```MAGE
+E Command {
    Add(f64, f64),
    Mul(f64, f64),
    Quit,
    Unknown(s),
}

f parse_command(input: s) -> Command {
    v parts = words(input)
    guard len(parts) > 0 else { ret Unknown("") }
    ?= parts[0] {
        "add" => Add(1.0, 2.0),
        "mul" => Mul(3.0, 4.0),
        "quit" => Quit,
        _ => Unknown(parts[0]),
    }
}

f evaluate(command: Command) -> s {
    ?= command {
        Add(a, b) => f"{a + b}",
        Mul(a, b) => f"{a * b}",
        Quit => "bye",
        Unknown(text) => f"unknown: {text}",
    }
}

+f main() -> i32 / io {
    @@ {
        v line = io.read_line()
        v command = parse_command(line)
        p"{evaluate(command)}"
        ?= command {
            Quit => !,
            _ => 0,
        }
    }
    0
}
```

**Discussion**: A sum type for the command, a `?=` for the dispatch, `@@` for the loop and `!` for break. Variants construct bare (`Quit`) unless two sums share a name, in which case the unqualified spelling is an error naming both rather than a silent pick.

---

### Key-value config parser

**Problem**: Parse a simple `key = value` configuration file.

**Solution**:

```MAGE
// A `key = value` file: split into lines, keep the ones with a separator,
// and fold them into a map.
+f parse_config(path: s) -> {s: s} / fs {
    m settings = {"": ""}
    @ line in lines(fs.read_to_string(path)) {
        v parts = split(line, "=")
        ? len(parts) == 2 {
            settings[parts[0]] = parts[1]
        }
    }
    settings
}
```

**Discussion**: `{s: s}` is the map type and `{"": ""}` a map literal — there is no `HashMap::new()`. Indexed assignment (`settings[k] = v`) is how you insert.

---
