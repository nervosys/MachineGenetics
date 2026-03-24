# I/O & Files

---

### Read a file to a string

**Problem**: Read the entire contents of a text file.

**Solution**:

```mg
u std.fs

+f main() / io {
    v content = fs.read("config.txt")?
    p"Contents: {content}"
}
```

**Discussion**: `fs.read` returns `R[s, IoError]`. For binary files, use
`fs.read_bytes` which returns `R[[u8]~, IoError]`.

---

### Write a string to a file

**Problem**: Write text to a file, creating it if it doesn't exist.

**Solution**:

```mg
u std.fs

+f main() / io {
    fs.write("output.txt", "Hello, MechGen!")?
}
```

**Discussion**: This overwrites the file if it exists. To append instead, use
`fs.append("output.txt", "more text\n")?`.

---

### Read a file line by line

**Problem**: Process a large file without loading it all into memory.

**Solution**:

```mg
u std.io.{File, BufReader, Read}

+f main() / io {
    v file = File.open("large.log")?
    v reader = BufReader.new(file)

    m line = s.new()
    m line_num = 0u64
    loop {
        v n = reader.read_line(&!line)?
        ? n == 0 { break }
        line_num += 1
        ? line.contains("ERROR") {
            p"Line {line_num}: {line.trim()}"
        }
        line.clear()
    }
}
```

**Discussion**: `BufReader` uses an internal buffer (default 8 KiB) to minimize
system calls. Call `line.clear()` between iterations to reuse the allocation.

---

### Parse a CSV file

**Problem**: Read a CSV file into a collection of records.

**Solution**:

```mg
u std.fs
u std.str.split

@d(Debug)
S Record {
    name: s,
    age: u32,
    city: s,
}

+f parse_csv(path: &s) -> R[[Record]~, Error] / io {
    v content = fs.read(path)?
    m records = [Record]~.new()

    v lines = content.lines()
    // Skip header
    v lines = lines.skip(1)

    @ line : lines {
        v fields: [&s]~ = line.split(',').collect()
        ? fields.len() >= 3 {
            records.push(Record @{
                name: fields[0].trim().into(),
                age: fields[1].trim().parse()?,
                city: fields[2].trim().into(),
            })
        }
    }
    Ok(records)
}
```

**Discussion**: For production CSV parsing with quoting and escaping, consider a
dedicated CSV library. This recipe handles simple comma-separated data.

---

### Walk a directory tree

**Problem**: Find all files matching a pattern in a directory and its
subdirectories.

**Solution**:

```mg
u std.fs

+f find_mg_files(dir: &s) -> R[[s]~, IoError] / io {
    m results = [s]~.new()
    v walker = fs.walk(dir)

    @ entry : walker {
        ? entry.is_file() && entry.path().ends_with(".mg") {
            results.push(entry.path().to_string())
        }
    }
    Ok(results)
}

+f main() / io {
    v files = find_mg_files("src")?
    p"Found {files.len()} .mg files"
    @ f : &files {
        p"  {f}"
    }
}
```

---

### Copy a file

**Problem**: Copy a file from one location to another.

**Solution**:

```mg
u std.fs

+f main() / io {
    fs.copy("source.txt", "backup.txt")?
    p"File copied"
}
```

---

### Create a temporary file

**Problem**: Write data to a temporary file that is cleaned up automatically.

**Solution**:

```mg
u std.fs.{TempFile, Write}

+f main() / io {
    v tmp = TempFile.new()?
    tmp.write("temporary data")?
    p"Temp file at: {tmp.path()}"

    // File is deleted when tmp goes out of scope
}
```

---

### Watch a file for changes

**Problem**: React when a file is modified.

**Solution**:

```mg
u std.fs.{watch, WatchEvent}

+f main() / io {
    v watcher = watch("config.toml")?

    p"Watching config.toml for changes..."
    @ event : watcher {
        ? event {
            WatchEvent.Modified(path) => {
                p"File modified: {path}"
                v content = fs.read(&path)?
                p"New size: {content.len()} bytes"
            },
            WatchEvent.Deleted(path) => {
                p"File deleted: {path}"
            },
            _ => {},
        }
    }
}
```

---

### Read environment-specific config

**Problem**: Load different config files based on an environment variable.

**Solution**:

```mg
u std.env
u std.fs

S Config {
    host: s,
    port: u16,
    debug: bool,
}

+f load_config() -> R[Config, Error] / io {
    v env = env.var("APP_ENV").unwrap_or("development".into())
    v path = f"config/{env}.toml"

    ? !fs.exists(&path) {
        ret Err(Error.new(f"Config file not found: {path}"))
    }

    v content = fs.read(&path)?
    parse_config(&content)
}
```

**Discussion**: This pattern lets you maintain `config/development.toml`,
`config/staging.toml`, and `config/production.toml` side by side.
