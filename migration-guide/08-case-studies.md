# Chapter 8: Case Studies

Three complete migration walkthroughs — a CLI tool, an HTTP service, and a
data-processing pipeline — showing every step from assessment to running MAGE.

---

## 8.1 Case Study: CLI Tool (`csvtool`)

A command-line CSV manipulation utility. Small crate, no async, minimal
dependencies. An ideal first migration target.

### 8.1.1 Assessment

| Metric           | Value            |
| ---------------- | ---------------- |
| Lines of Rust    | 620              |
| Unsafe blocks    | 0                |
| Async code       | none             |
| Dependencies     | clap, csv, serde |
| Estimated effort | 0.5 days         |

### 8.1.2 Rust Source (Before)

```rust
// src/main.rs
use clap::{Parser, Subcommand};
use csv::Reader;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "csvtool", version, about = "CSV manipulation tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Count rows in a CSV file
    Count {
        #[arg(short, long)]
        file: PathBuf,
    },
    /// Filter rows by column value
    Filter {
        #[arg(short, long)]
        file: PathBuf,
        #[arg(short, long)]
        column: String,
        #[arg(short, long)]
        value: String,
    },
}

#[derive(Debug, Deserialize)]
struct Record {
    #[serde(flatten)]
    fields: std::collections::HashMap<String, String>,
}

fn count_rows(path: &PathBuf) -> Result<usize, Box<dyn Error>> {
    let mut rdr = Reader::from_path(path)?;
    Ok(rdr.records().count())
}

fn filter_rows(
    path: &PathBuf,
    column: &str,
    value: &str,
) -> Result<Vec<Record>, Box<dyn Error>> {
    let mut rdr = Reader::from_path(path)?;
    let mut results = Vec::new();
    for result in rdr.deserialize() {
        let record: Record = result?;
        if record.fields.get(column) == Some(&value.to_string()) {
            results.push(record);
        }
    }
    Ok(results)
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Count { file } => {
            let count = count_rows(&file)?;
            println!("{} rows", count);
        }
        Commands::Filter { file, column, value } => {
            let rows = filter_rows(&file, &column, &value)?;
            println!("{} matching rows", rows.len());
            for row in &rows {
                println!("{:?}", row.fields);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_csv() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "name,age,city").unwrap();
        writeln!(file, "Alice,30,NYC").unwrap();
        writeln!(file, "Bob,25,LA").unwrap();
        writeln!(file, "Carol,30,NYC").unwrap();
        file
    }

    #[test]
    fn test_count() {
        let file = create_csv();
        assert_eq!(count_rows(&file.path().to_path_buf()).unwrap(), 3);
    }

    #[test]
    fn test_filter() {
        let file = create_csv();
        let results = filter_rows(
            &file.path().to_path_buf(),
            "city",
            "NYC",
        ).unwrap();
        assert_eq!(results.len(), 2);
    }
}
```

### 8.1.3 Migration Steps

**Step 1: Create MAGE project alongside**

```bash
mg init --alongside   # adds Forge.toml, keeps Cargo.toml
```

**Step 2: Run automated translation**

```bash
mg migrate src/main.rs -o src/main.mg
```

**Step 3: Review and refine**

The automated output needs manual adjustments for effect annotations and
MAGE idioms. Final result:

### 8.1.4 MAGE Source (After)

```MAGE
// src/main.mg — csvtool, after migration.
//
// No `clap`, no derive attributes and no `PathBuf`: the command is a sum, and
// parsing is `env.args()` plus a `?=`. What the crate's 340 lines of derive
// macros bought, a 20-line function does here.

+E Command {
    Count(s),
    Filter(s, s, s),
    Usage,
}

+f parse_command(argv: [s]~) -> Command {
    guard len(argv) > 1 else { ret Usage }
    ?= argv[1] {
        "count" => ? len(argv) > 2 { Count(argv[2]) } : { Usage },
        "filter" => ? len(argv) > 4 { Filter(argv[2], argv[3], argv[4]) } : { Usage },
        _ => Usage,
    }
}

f count_rows(path: s) -> i32 / fs {
    (len(lines(fs.read_to_string(path))) as i32) - 1
}

f filter_rows(path: s, column: s, value: s) -> [s]~ / fs {
    v rows = lines(fs.read_to_string(path))
    filter(rows, |row| contains(row, value) && contains(row, column))
}

+f run(argv: [s]~) -> i32 / fs, io {
    ?= parse_command(argv) {
        Count(path) => {
            v n = count_rows(path)
            p"{n}"
            n
        },
        Filter(path, column, value) => {
            v kept = filter_rows(path, column, value)
            @ row in kept { p"{row}" }
            len(kept) as i32
        },
        Usage => {
            ep"usage: csvtool count <file> | filter <file> <column> <value>"
            1
        },
    }
}
```

### 8.1.5 Key Observations

| Aspect     | Rust → MAGE |
| ---------- | ----------- |
| Keywords   | `fn`, `let`, `pub`, `match` → `f`, `v`, `+`, `?=` |
| Type sugar | `HashMap<String,String>` → `{s: s}`, `Vec<T>` → `[T]~` |
| Paths      | **deleted** — there is no module system, and `std.path.PathBuf` does not exist either |
| Derives    | **deleted** — no `@d(Parser)`, no `clap`; the command is a sum and `env.args()` is the parser |
| Effects    | implicit I/O → explicit `/ fs, io`, and reading a file is `fs`, not `io` |
| Tests      | tempfile crate → effect mocking (§8.2.4) |

---

## 8.2 Case Study: HTTP Service (`user-api`)

A REST API service using axum + tokio + sqlx. Moderate complexity with async,
database access, and middleware.

### 8.2.1 Assessment

| Metric           | Value                                    |
| ---------------- | ---------------------------------------- |
| Lines of Rust    | 1,450                                    |
| Unsafe blocks    | 1 (FFI for argon2 binding)               |
| Async code       | heavy (axum + tokio + sqlx)              |
| Dependencies     | axum, tokio, sqlx, serde, tower, tracing |
| Estimated effort | 3 days                                   |

### 8.2.2 Rust Source (Key Excerpts)

```rust
// src/main.rs
use axum::{Router, routing::get, routing::post, Json, Extension};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::init();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;

    let state = Arc::new(AppState { db: pool });

    let app = Router::new()
        .route("/users", get(list_users))
        .route("/users", post(create_user))
        .route("/users/:id", get(get_user))
        .layer(CorsLayer::permissive())
        .layer(Extension(state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// src/handlers.rs
use axum::{Extension, Json};
use sqlx::PgPool;
use std::sync::Arc;

pub async fn list_users(
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Json<Vec<User>>, AppError> {
    let users = sqlx::query_as!(User, "SELECT id, name, email FROM users")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(users))
}

pub async fn create_user(
    Extension(state): Extension<Arc<AppState>>,
    Json(input): Json<CreateUser>,
) -> Result<Json<User>, AppError> {
    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, name, email",
        input.name,
        input.email
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(user))
}
```

### 8.2.3 MAGE Source (After)

```MAGE
// src/main.mg — user-api, after migration.
//
// There is no web framework, no `Router`, and no `sqlx`. A route is a sum, a
// handler is a function, and the store is a **declared effect** — which is
// what makes the handlers testable without a database.

+S User { id: i32, name: s, email: s }

effect Db {
    f all_users() -> [s]~;
    f insert_user(name: s, email: s) -> i32;
}

+E Route {
    ListUsers,
    CreateUser(s, s),
    NotFound(s),
}

+f route_of(path: s, body: s) -> Route {
    v parts = filter(split(path, "/"), |part| len(part) > 0)
    guard len(parts) > 0 else { ret NotFound(path) }
    ?= parts[0] {
        "users" => ? len(body) > 0 { CreateUser(body, body) } : { ListUsers },
        _ => NotFound(path),
    }
}

// Each handler declares the store it reaches. `db` is the effect, and every
// caller inherits it up to the boundary that handles it.
+f handle_route(route: Route) -> s / db {
    ?= route {
        ListUsers => join(Db.all_users(), ","),
        CreateUser(name, email) => f"{Db.insert_user(name, email)}",
        NotFound(path) => f"404 {path}",
    }
}

// The entry point declares the union: the socket and the store.
+f serve(path: s, body: s) -> s / db, net {
    net.listen("0.0.0.0:3000")
    handle_route(route_of(path, body))
}
```

```MAGE
// src/handlers.mg
//
// A handler is an ordinary function over the effect declared in `main.mg`.
// There is no extractor, no `Json[T]` wrapper and no macro: the request is
// values in, and the response is a string out.

effect Db {
    f all_users() -> [s]~;
    f insert_user(name: s, email: s) -> i32;
}

+f list_users() -> s / db {
    join(Db.all_users(), ",")
}

+f create_user(name: s, email: s) -> R[i32, s] / db {
    guard len(name) > 0 else { ret Err("name is required") }
    guard contains(email, "@") else { ret Err("invalid email") }
    Ok(Db.insert_user(name, email))
}
```

### 8.2.4 Testing With Effect Mocking

```MAGE
// Testing the handlers with no database.
//
// `handle … with` substitutes the operations of one declared effect for the
// block it wraps, so the test is pure — and an unhandled call anywhere else
// still reports.

effect Db {
    f all_users() -> [s]~;
    f insert_user(name: s, email: s) -> i32;
}

f list_users() -> s / db { join(Db.all_users(), ",") }

@test
+f test_list_users() -> bool {
    handle {
        list_users()
    } with Db {
        all_users() => ["Alice", "Bob"],
        insert_user(name, email) => 0,
    } == "Alice,Bob"
}
```

### 8.2.5 Key Observations

| Aspect   | Change |
| -------- | ------ |
| Runtime  | tokio removed. `async` is a declaration keyword (`+af name(…)`), and there is no `.await` to write |
| Framework | axum removed. There is no router, no extractor and no `Json[T]`: a route is a sum and a handler is a function |
| Store    | sqlx removed. **The database is a declared `effect`** — `effect Db { … }` — which is what makes the handlers testable |
| Effects  | every handler annotated `/ db`; the entry point declares `/ db, net` |
| Types    | `Vec<User>` → `[User]~`, `Arc<AppState>` → a plain value |
| Testing  | wiremock/mockall → `handle { … } with Db { … }`, which is part of the type system rather than a library |

---

## 8.3 Case Study: Data Pipeline (`etl-pipeline`)

A batch ETL pipeline reading CSV files, transforming data, and writing Parquet.
Uses threads for parallelism and unsafe for performance-critical SIMD.

### 8.3.1 Assessment

| Metric           | Value                      |
| ---------------- | -------------------------- |
| Lines of Rust    | 2,100                      |
| Unsafe blocks    | 3 (SIMD intrinsics)        |
| Async code       | none (threaded)            |
| Dependencies     | csv, parquet, rayon, serde |
| Estimated effort | 4 days                     |

### 8.3.2 Rust Source (Key Excerpts)

```rust
// src/pipeline.rs
use rayon::prelude::*;
use std::path::Path;

pub struct Pipeline {
    input_dir: PathBuf,
    output_dir: PathBuf,
    batch_size: usize,
}

impl Pipeline {
    pub fn new(input: PathBuf, output: PathBuf, batch_size: usize) -> Self {
        Self { input_dir: input, output_dir: output, batch_size }
    }

    pub fn run(&self) -> Result<Stats, PipelineError> {
        let files: Vec<_> = std::fs::read_dir(&self.input_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension() == Some("csv".as_ref()))
            .collect();

        let results: Vec<_> = files.par_iter()
            .map(|entry| self.process_file(&entry.path()))
            .collect();

        let mut stats = Stats::default();
        for result in results {
            match result {
                Ok(s) => stats.merge(s),
                Err(e) => stats.add_error(e),
            }
        }
        Ok(stats)
    }

    fn process_file(&self, path: &Path) -> Result<Stats, PipelineError> {
        let records = read_csv(path)?;
        let transformed = transform_batch(&records);
        let output_path = self.output_dir.join(
            path.file_stem().unwrap()
        ).with_extension("parquet");
        write_parquet(&output_path, &transformed)?;

        Ok(Stats { rows: records.len(), files: 1, errors: 0 })
    }
}

// src/transform.rs — performance-critical SIMD
pub fn normalize_floats(data: &mut [f64]) {
    let max = data.iter().cloned().fold(f64::MIN, f64::max);
    if max == 0.0 { return; }

    // SAFETY: aligned f64 slice, length checked
    unsafe {
        use std::arch::x86_64::*;
        let divisor = _mm256_set1_pd(max);
        let chunks = data.len() / 4;
        for i in 0..chunks {
            let ptr = data.as_mut_ptr().add(i * 4);
            let vals = _mm256_loadu_pd(ptr);
            let normed = _mm256_div_pd(vals, divisor);
            _mm256_storeu_pd(ptr, normed);
        }
    }
    // Handle remainder
    for val in data[data.len() - data.len() % 4..].iter_mut() {
        *val /= max;
    }
}
```

### 8.3.3 Migration Strategy

This crate requires a phased approach:

1. **Phase 1**: Migrate `Pipeline` struct and `run` method (thread → Agent/Swarm)
2. **Phase 2**: Migrate `transform.rs` (wrap SIMD in `/ unsafe` effect)
3. **Phase 3**: Migrate tests, remove rayon

### 8.3.4 MAGE Source (After)

```MAGE
// src/pipeline.mg — etl-pipeline, after migration.
//
// rayon's `par_iter` becomes an `agent` declaration plus `map`: the fan-out
// is a vocabulary call and the *capability* is what the annotation records.

agent FileProcessor {
    capabilities: [fs]
}

swarm Pool {
    agent: FileProcessor
    size: 4
    topology: mesh
    consensus: majority
}

+S Stats { rows: i32, files: i32, errors: i32 }

f process_file(path: s) -> Stats / fs {
    v rows = lines(fs.read_to_string(path))
    @Stats { rows: len(rows) as i32, files: 1, errors: 0 }
}

f merge(a: Stats, b: Stats) -> Stats {
    @Stats {
        rows: a.rows + b.rows,
        files: a.files + b.files,
        errors: a.errors + b.errors,
    }
}

+f run(paths: [s]~) -> Stats / fs {
    fold(
        map(paths, |path| process_file(path)),
        @Stats { rows: 0, files: 0, errors: 0 },
        |acc, stats| merge(acc, stats),
    )
}
```

```MAGE
// src/transform.mg
//
// There is no `unsafe`, no SIMD intrinsic and no `/ unsafe` effect — `unsafe`
// does not parse, and the effect kinds are the 17 in §11.2. The transform is
// ordinary code; if it were to reach the GPU, the annotation would be `/ gpu`.

+f normalize(data: [f64]~) -> [f64]~ {
    v peak = fold(data, 0.0, |acc, x| ? x > acc { x } : { acc })
    guard peak != 0.0 else { ret data }
    map(data, |x| x / peak)
}
```

### 8.3.5 Where the capability grant lives

**There is no `[capabilities]` section in `Forge.toml`, and no per-file
grant.** The grant is the `/ effect` annotation on the function, checked at
compile time: a `pub` function that reaches `fs` declares `fs`, and every
caller that reaches it inherits the obligation up to the boundary that handles
it. Scoping a capability to one file is not a thing you configure — it is what
you get by keeping the annotation off everything else.

### 8.3.6 Key Observations

| Aspect       | Change |
| ------------ | ------ |
| Parallelism  | rayon `par_iter` → an `agent` declaration plus `map`; fan-in is `fold` |
| Unsafe       | **deleted.** `unsafe` does not parse, and there is no `/ unsafe` effect — the kinds are the 17 in §11.2 |
| Capability   | the `/ fs` annotation, not a config file |
| Dependencies | rayon removed |
| Backpressure | `swarm` declares `size`; there is no `with_limit` |
| Testing      | direct calls → `handle { … } with E { … }` |

---

## 8.4 Migration Metrics Summary

| Crate        | Rust LOC | MAGE LOC | Reduction | Effort   | Hardest Part        |
| ------------ | -------- | --------- | --------- | -------- | ------------------- |
| csvtool      | 620      | 530       | 15%       | 0.5 days | Effect annotation   |
| user-api     | 1,450    | 1,210     | 17%       | 3 days   | Async runtime swap  |
| etl-pipeline | 2,100    | 1,780     | 15%       | 4 days   | SIMD unsafe scoping |

### Patterns Observed

1. **Line reduction** averages 15-17%, mostly from type sugar and keyword
   brevity.
2. **Effect annotations** are the most manual part — `mg migrate` cannot
   always infer them.
3. **Async migration** is straightforward for simple cases but requires
   rethinking for complex spawn/select patterns.
4. **Unsafe SIMD** migrates intact but gains explicit capability scoping.
5. **Testing** improves the most — effect mocking replaces entire mock
   libraries and test infrastructure.
6. **Dependencies** decrease — built-in async, testing, and benchmarking
   replace tokio, criterion, mockall, tempfile.

---

## 8.5 Next Steps After Migration

1. **Run `mg lint`** — catches Rust idioms that should be MAGE patterns
2. **Run `mg test`** — verify all tests pass in the MAGE runtime
3. **Remove Cargo.toml** — when dual-build is no longer needed
4. **Delete `.rs` files** — keep only `.mg` sources
5. **Update CI** — switch from dual pipeline to MAGE-only
6. **Update README** — note the project now uses MAGE
