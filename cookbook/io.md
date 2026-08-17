# I/O & Files

> Recipes for files, directories and the environment. Agent-mode syntax; every
> block was verified with `mage-parse --check`.

The previous version of this page was written against a standard library that
does not exist — `u std.io.{File, BufReader, Read}`, `s.new()`,
`fs.read(path)?` returning `R[s, IoError]`, `TempFile`, `fs.watch`. There is
no module system and nothing to import: `fs`, `io`, `env` and `time` are
**capability namespaces**, in scope everywhere, and reaching one is what puts
its effect in the function's inferred set.

---

### Read a file to a string

**Problem**: Read the entire contents of a text file.

**Solution**:

```MAGE
+f main() -> i32 / fs, io {
    v content = fs.read_to_string("config.txt")
    p"Contents: {content}"
    0
}
```

**Discussion**: `fs.read_to_string` is a capability call: reaching `fs` is what puts `fs` in the inferred set, and a public function must declare it. Printing is a *different* capability, so `io` is declared too — neither implies the other.

---

### Write a string to a file

**Problem**: Write text to a file, creating it if it does not exist.

**Solution**:

```MAGE
+f main() -> i32 / fs {
    fs.write("output.txt", "Hello, MAGE!")
    0
}
```

**Discussion**: This overwrites. The `fs` namespace also has `open`, `create`, `remove`, `rename`, `mkdir` and `stat`; every one of them performs `fs`.

---

### Read a file line by line

**Problem**: Process a file one line at a time.

**Solution**:

```MAGE
// There is no `BufReader` and no streaming reader: read the file, then walk
// the lines. `lines` is part of the standard vocabulary.
+f main() -> i32 / fs, io {
    v content = fs.read_to_string("large.log")
    m n = 0
    @ line in lines(content) {
        n += 1
        p"{n}: {line}"
    }
    n
}
```

**Discussion**: There is no `BufReader`, no `File` type and no iterator protocol. `lines` splits the contents; `words` and `chars` are the other two.

---

### Parse a CSV file

**Problem**: Read a CSV file into a collection of records.

**Solution**:

```MAGE
+S Record { name: s, age: s, city: s }

// `split` gives the fields. There is no `.trim().parse()?` chain — the fields
// stay strings unless you convert them yourself.
f record_of(line: s) -> Record {
    v fields = split(line, ",")
    @Record { name: fields[0], age: fields[1], city: fields[2] }
}

+f parse_csv(path: s) -> [Record]~ / fs {
    v rows = lines(fs.read_to_string(path))
    // Skip the header row.
    v body = filter(rows, |row| row != first_row(rows))
    map(body, |line| record_of(line))
}

f first_row(rows: [s]~) -> s {
    ?= first(rows) {
        Some(row) => row,
        None => "",
    }
}
```

**Discussion**: `split` returns `[s]~`, and the fields stay strings. There is no `parse::<T>()` and no `?` error conversion, so a numeric column is yours to convert and validate.

---

### Walk a directory tree

**Problem**: Find all files matching a pattern in a directory and below it.

**Solution**:

```MAGE
// `fs.walk` is a capability call like any other, and its effect is `fs`. Its
// result type is not known to the checker, so annotate the binding.
+f find_mg_files(dir: s) -> [s]~ / fs {
    v entries: [s]~ = fs.walk(dir)
    filter(entries, |path| contains(path, ".mg"))
}

+f main() -> i32 / fs, io {
    v files = find_mg_files("src")
    p"Found {len(files)} .mg files"
    @ path in files {
        p"  {path}"
    }
    0
}
```

**Discussion**: A capability call returns a type the checker does not know, so annotate the binding when you need a specific one — `v entries: [s]~ = fs.walk(dir)`.

---

### Copy a file

**Problem**: Copy a file from one location to another.

**Solution**:

```MAGE
// There is no `fs.copy`. Read, then write — two capability calls, one effect.
+f copy_file(from: s, to: s) -> i32 / fs {
    fs.write(to, fs.read_to_string(from))
    0
}

+f main() -> i32 / fs, io {
    copy_file("source.txt", "backup.txt")
    p"File copied"
    0
}
```

**Discussion**: There is no `fs.copy`. Read then write — two calls, one effect.

---

### Create a temporary file

**Problem**: Write data to a temporary file and clean it up.

**Solution**:

```MAGE
// There is no `TempFile` type and no scope-based cleanup. A temporary file is
// a path you choose and remove yourself; `defer` runs at scope exit.
+f with_temp(data: s) -> i32 / fs {
    v path = "build/tmp.dat"
    defer fs.remove(path)
    fs.write(path, data)
    len(data) as i32
}
```

**Discussion**: There is no `TempFile` and no drop-based cleanup. `defer` runs its expression at scope exit, which is the mechanism you have.

---

### Watch a file for changes

**Problem**: React when a file is modified.

**Solution**:

```MAGE
// There is no file watcher. Poll: read the file, compare, sleep.
+f poll_once(path: s, previous: s) -> s / fs, io {
    v current = fs.read_to_string(path)
    ? current != previous {
        p"File modified: {path}"
    }
    current
}

+f watch(path: s, rounds: i32) -> s / fs, io, time {
    m seen = fs.read_to_string(path)
    @ _ in range(rounds as usize) {
        seen = poll_once(path, seen)
        time.sleep(1)
    }
    seen
}
```

**Discussion**: There is no watcher and no event stream. Poll: read, compare, sleep. `time.sleep` performs `time`, which is a capability of its own — a program that waits says so in its signature.

---

### Read environment-specific config

**Problem**: Load a different config file based on an environment variable.

**Solution**:

```MAGE
+S Config { host: s, port: i32, debug: bool }

// `env` is a capability namespace and `env` is its effect. Reading the
// variable and reading the file are two capabilities, so both are declared.
+f load_config() -> Config / env, fs {
    v name = env.get_env("APP_ENV")
    v path = f"config/{name}.toml"
    v raw = fs.read_to_string(path)
    @Config { host: raw, port: 8080, debug: 0b }
}
```

**Discussion**: `env.get_env` performs `env`; reading the file performs `fs`. Two capabilities, two declarations. The pattern generalises: the annotation is a list of every resource the function can reach.

---
