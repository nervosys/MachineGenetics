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
// src/main.mg
u clap.{Parser, Subcommand}
u csv.Reader
u serde.Deserialize
u std.fs
u std.path.PathBuf

@d(Parser)
@command(name: "csvtool", version, about: "CSV manipulation tool")
+S Cli {
    @command(subcommand)
    command: Commands,
}

@d(Subcommand)
+E Commands {
    /// Count rows in a CSV file
    Count {
        @arg(short, long)
        file: PathBuf,
    },
    /// Filter rows by column value
    Filter {
        @arg(short, long)
        file: PathBuf,
        @arg(short, long)
        column: s,
        @arg(short, long)
        value: s,
    },
}

@d(Debug, Deserialize)
S Record {
    @serde(flatten)
    fields: {s: s},
}

f count_rows(path: &PathBuf) -> R[usize, ^dyn Error] / io {
    m rdr = Reader.from_path(path)?
    Ok(rdr.records().count())
}

f filter_rows(
    path: &PathBuf,
    column: &s,
    value: &s,
) -> R[Vec[Record], ^dyn Error] / io {
    m rdr = Reader.from_path(path)?
    m results = [Record]~.new()
    @ result ~ rdr.deserialize() {
        v record: Record = result?
        ? record.fields.get(column) == Some(&s.from(value)) {
            results.push(record)
        }
    }
    Ok(results)
}

+f main() -> R[(), ^dyn Error] / io {
    v cli = Cli.parse()

    ? cli.command {
        Commands.Count @{ file } => {
            v count = count_rows(&file)?
            p"{count} rows"
        },
        Commands.Filter @{ file, column, value } => {
            v rows = filter_rows(&file, &column, &value)?
            p"{rows.len()} matching rows"
            @ row ~ &rows {
                p"{row.fields:?}"
            }
        },
    }

    Ok(())
}

@cfg(test)
M tests {
    u super.*

    f create_csv() -> s / io {
        v content = "name,age,city\nAlice,30,NYC\nBob,25,LA\nCarol,30,NYC\n"
        content.to_owned()
    }

    @test
    f test_count() {
        v count = handle / io {
            count_rows(&PathBuf.from("test.csv"))
        } with {
            Reader.from_path(_) => mock_reader(create_csv()),
        }
        assert_eq!(count.unwrap(), 3)
    }

    @test
    f test_filter() {
        v results = handle / io {
            filter_rows(&PathBuf.from("test.csv"), "city", "NYC")
        } with {
            Reader.from_path(_) => mock_reader(create_csv()),
        }
        assert_eq!(results.unwrap().len(), 2)
    }
}
```

### 8.1.5 Key Observations

| Aspect     | Rust → MAGE                                     |
| ---------- | ------------------------------------------------ |
| Lines      | 95 → 82 (14% reduction)                          |
| Keywords   | `fn`, `let`, `pub`, `match` → `f`, `v`, `+`, `?` |
| Paths      | `std::path::PathBuf` → `std.path.PathBuf`        |
| Generics   | `Vec<Record>` → `Vec[Record]`                    |
| Type sugar | `HashMap<String,String>` → `{s: s}`              |
| Match      | `match cli.command { }` → `? cli.command { }`    |
| Tests      | tempfile crate → effect mocking                  |
| Effects    | implicit I/O → explicit `/ io`                   |

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
// src/main.mg
u web.{Router, routing.{get, post}, Json, Extension}
u db.postgres.PgPoolOptions
u std.sync.Arc
u web.cors.CorsLayer

+af main() -> R[(), ^dyn Error] / net, db, log {
    log.init()

    v pool = PgPoolOptions.new()
        .max_connections(5)
        .connect(&env.var("DATABASE_URL")?)
        .await?

    v state = @AppState @{ db: pool }

    v app = Router.new()
        .route("/users", get(list_users))
        .route("/users", post(create_user))
        .route("/users/:id", get(get_user))
        .layer(CorsLayer.permissive())
        .layer(Extension(state))

    v listener = net.TcpListener.bind("0.0.0.0:3000").await?
    web.serve(listener, app).await?
    Ok(())
}
```

```MAGE
// src/handlers.mg
u web.{Extension, Json}
u db.PgPool

+af list_users(
    Extension(state): Extension[@AppState],
) -> R[Json[[User]~], AppError] / db {
    v users = db.query_as!(User, "SELECT id, name, email FROM users")
        .fetch_all(&state.db)
        .await?
    Ok(Json(users))
}

+af create_user(
    Extension(state): Extension[@AppState],
    Json(input): Json[CreateUser],
) -> R[Json[User], AppError] / db {
    v user = db.query_as!(
        User,
        "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, name, email",
        input.name,
        input.email
    )
    .fetch_one(&state.db)
    .await?
    Ok(Json(user))
}
```

### 8.2.4 Testing With Effect Mocking

```MAGE
@cfg(test)
M tests {
    u super.*

    @test
    af test_list_users() {
        v result = handle / db {
            list_users(Extension(@AppState @{ db: mock_pool() })).await
        } with {
            db.query_as!(User, _) => Ok([
                User @{ id: 1, name: s.from("Alice"), email: s.from("alice@example.com") },
            ]~),
        }

        v Json(users) = result.unwrap()
        assert_eq!(users.len(), 1)
        assert_eq!(users[0].name, "Alice")
    }
}
```

### 8.2.5 Key Observations

| Aspect   | Change                                         |
| -------- | ---------------------------------------------- |
| Runtime  | tokio removed — built-in async                 |
| Effects  | All handlers annotated `/ db`, `/ net`         |
| Paths    | `sqlx::query_as!` → `db.query_as!`             |
| Generics | `Arc<AppState>` → `@AppState`                  |
| Types    | `Vec<User>` → `[User]~`                        |
| Main     | `#[tokio::main]` → `+af main() / net, db, log` |
| Testing  | wiremock/mockall → `handle / db` blocks        |

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
// src/pipeline.mg
u std.path.{Path, PathBuf}
u std.agent.{Agent, Swarm}

+S Pipeline {
    input_dir: PathBuf,
    output_dir: PathBuf,
    batch_size: usize,
}

I ~ Pipeline {
    +f new(input: PathBuf, output: PathBuf, batch_size: usize) -> Self {
        Self @{ input_dir: input, output_dir: output, batch_size }
    }

    +af run(&self) -> R[Stats, PipelineError] / io, agent {
        v files = fs.read_dir(&self.input_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension() == Some("csv".as_ref()))
            .collect[Vec[_]]()

        // Fan-out to agent Swarm (replaces rayon)
        v swarm = Swarm.new()
        @ entry ~ &files {
            swarm.spawn(FileProcessor @{
                path: entry.path(),
                output_dir: self.output_dir.clone(),
            })
        }
        v results = swarm.join_all().await?

        m stats = Stats.default()
        @ result ~ results {
            ? result {
                Ok(s) => stats.merge(s),
                Err(e) => stats.add_error(e),
            }
        }
        Ok(stats)
    }
}

S FileProcessor {
    path: PathBuf,
    output_dir: PathBuf,
}

I Agent ~ FileProcessor {
    +af execute(&!self) -> R[Stats, PipelineError] / io, agent {
        v records = read_csv(&self.path)?
        v transformed = transform_batch(&records)
        v output_path = self.output_dir.join(
            self.path.file_stem().unwrap()
        ).with_extension("parquet")
        write_parquet(&output_path, &transformed)?

        Ok(Stats @{ rows: records.len(), files: 1, errors: 0 })
    }
}
```

```MAGE
// src/transform.mg
+f normalize_floats(data: &![f64]) / unsafe {
    v max = data.iter().cloned().fold(f64.MIN, f64.max)
    ? max == 0.0 { ret }

    // SIMD normalization — effect annotation makes the unsafe explicit
    u std.arch.x86_64.*
    v divisor = _mm256_set1_pd(max)
    v chunks = data.len() / 4
    @ i ~ 0..chunks {
        v ptr = data.as_mut_ptr().add(i * 4)
        v vals = _mm256_loadu_pd(ptr)
        v normed = _mm256_div_pd(vals, divisor)
        _mm256_storeu_pd(ptr, normed)
    }
    // Handle remainder
    @ val ~ data[data.len() - data.len() % 4..].iter_mut() {
        *val /= max
    }
}
```

### 8.3.5 Forge.toml — Capability Grants

```toml
[package]
name = "etl-pipeline"
version = "0.1.0"
edition = "2025"

[effects]
io = true
agent = true

[capabilities]
allow-unsafe = ["src/transform.mg"]   # SIMD only
allow-io = ["src/pipeline.mg"]
allow-agent = ["src/pipeline.mg"]
```

### 8.3.6 Key Observations

| Aspect       | Change                                   |
| ------------ | ---------------------------------------- |
| Parallelism  | rayon `par_iter` → Agent + Swarm         |
| Unsafe       | Raw SIMD → `/ unsafe` effect annotation  |
| Capability   | Unsafe scoped to `transform.mg` only    |
| Dependencies | rayon removed                            |
| Backpressure | rayon auto-tuning → `Swarm.with_limit()` |
| Testing      | Direct calls → `handle / io` mocking     |

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
