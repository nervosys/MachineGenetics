// effects-showcase — Algebraic effect system in MechGen.
//
// Demonstrates:
//   - Effect declarations (effect keyword)
//   - Effect annotations on functions (/ io, / net, etc.)
//   - Effect composition (/ io + net)
//   - Effect polymorphism (generic over effects)
//   - Pure functions (no effect annotation)
//   - Effect handlers (handle keyword)
//   - Effect constraints in specs (@fx)
//   - Practical patterns: logging, database, async I/O

use std::io;
use std::fmt;
use std::col;

// ─────────────────────────────────────────────────────────────────────
// §1 — Declaring custom effects
// ─────────────────────────────────────────────────────────────────────
//
// An `effect` block declares a set of operations that a function may
// perform.  The compiler tracks which effects each function uses —
// if a function has no annotation, it is proven pure.

// Standard I/O effect (provided by std, shown here for illustration).
effect io {
    fn read(fd: i32, buf: &mut [u8]) -> isize;
    fn write(fd: i32, buf: &[u8]) -> isize;
}

// Logging effect — separates "what to log" from "how to log".
effect log {
    fn info(msg: &str);
    fn warn(msg: &str);
    fn error(msg: &str);
}

// Database effect — abstract over storage backend.
effect db {
    fn get(key: &str) -> Option<String>;
    fn put(key: &str, value: &str);
    fn delete(key: &str) -> bool;
}

// Random number generation effect.
effect rng {
    fn next_u64() -> u64;
    fn next_f64() -> f64;
}

// ─────────────────────────────────────────────────────────────────────
// §2 — Pure functions (no effects)
// ─────────────────────────────────────────────────────────────────────
//
// Functions without an effect annotation are guaranteed pure by the
// compiler — no I/O, no mutation of global state, no randomness.

fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}

fn reverse<T: Clone>(items: &Vec<T>) -> Vec<T> {
    let mut result: Vec<T> = Vec::new();
    let mut i = items.len();
    for _ in 0..items.len() {
        i = i - 1;
        result.push(items[i].clone());
    }
    result
}

// ─────────────────────────────────────────────────────────────────────
// §3 — Annotating effects on functions
// ─────────────────────────────────────────────────────────────────────
//
// The `/ effect` annotation declares what effects a function performs.
// The compiler verifies that the function body only uses operations
// from its declared effect set.

// Single effect — this function does I/O.
fn greet(name: &str) / io {
    println!("Hello, {name}!");
}

// Single effect — logging only.
fn log_startup() / log {
    log::info("Application starting");
    log::info("Loading configuration");
}

// Database access.
fn lookup_user(id: &str) / db -> Option<String> {
    db::get(id)
}

// ─────────────────────────────────────────────────────────────────────
// §4 — Effect composition
// ─────────────────────────────────────────────────────────────────────
//
// Functions that use multiple effects compose them with `+`.

fn initialize_db(entries: &Vec<(&str, &str)>) / db + log {
    log::info("Seeding database");
    for (key, value) in entries {
        db::put(key, value);
        log::info(&format!("  stored: {key}"));
    }
    log::info(&format!("Seeded {entries.len()} entries"));
}

fn process_request(user_id: &str) / io + db + log -> Result<String, String> {
    log::info(&format!("Processing request for user {user_id}"));

    let user = db::get(user_id);
    match user {
        Some(name) => {
            println!("Found user: {name}");
            log::info(&format!("User {user_id} found"));
            Ok(name)
        },
        None => {
            log::warn(&format!("User {user_id} not found"));
            Err(format!("unknown user: {user_id}"))
        },
    }
}

// ─────────────────────────────────────────────────────────────────────
// §5 — Effect polymorphism
// ─────────────────────────────────────────────────────────────────────
//
// Generic functions can be polymorphic over an effect variable `E`,
// so they work with any effect set determined by their arguments.

fn transform<T, U, E>(items: &Vec<T>, func: fn(&T) -> E U) -> E Vec<U> {
    let mut results: Vec<U> = Vec::new();
    for item in items {
        results.push(func(item));
    }
    results
}

// When called with a pure closure, `transform` is pure.
// When called with an effectful closure, `transform` inherits that
// effect.

fn demo_polymorphism() / io {
    let numbers = vec![1, 2, 3, 4, 5];

    // Pure usage — transform inherits no effects.
    let doubled = transform(&numbers, |n| n * 2);
    println!("Doubled: {doubled:?}");

    // Effectful usage — transform inherits / io from the closure.
    let printed = transform(&numbers, |n| {
        println!("  processing {n}");
        n * 10
    });
    println!("Processed: {printed:?}");
}

// ─────────────────────────────────────────────────────────────────────
// §6 — Effect handlers
// ─────────────────────────────────────────────────────────────────────
//
// Handlers intercept effect operations and provide concrete
// implementations.  This separates policy from mechanism.

// A console-based log handler.
struct ConsoleLogger;

impl ConsoleLogger {
    fn new() -> ConsoleLogger {
        ConsoleLogger
    }
}

handle log for ConsoleLogger {
    fn info(msg: &str) {
        println!("[INFO]  {msg}");
    }
    fn warn(msg: &str) {
        println!("[WARN]  {msg}");
    }
    fn error(msg: &str) {
        eprintln!("[ERROR] {msg}");
    }
}

// An in-memory database handler (useful for testing).
struct MemoryDb {
    store: HashMap<String, String>,
}

impl MemoryDb {
    fn new() -> MemoryDb {
        MemoryDb { store: HashMap::new() }
    }
}

handle db for MemoryDb {
    fn get(key: &str) -> Option<String> {
        self.store.get(key).cloned()
    }
    fn put(key: &str, value: &str) {
        self.store.insert(key.clone(), value.clone());
    }
    fn delete(key: &str) -> bool {
        self.store.remove(key).is_some()
    }
}

// A deterministic RNG handler (repeatable tests).
struct FixedRng {
    value: u64,
}

handle rng for FixedRng {
    fn next_u64() -> u64 {
        self.value
    }
    fn next_f64() -> f64 {
        (self.value % 1000) as f64 / 1000.0
    }
}

// ─────────────────────────────────────────────────────────────────────
// §7 — Effect constraints in specs
// ─────────────────────────────────────────────────────────────────────
//
// Specs can use @fx to constrain which effects a conforming
// implementation may use.

spec PureSort<T: Ord> {
    @fx();                          // must be pure — no effects allowed
    @req(items.len() > 0);
    @ens(|result| result.len() == items.len());
}

spec DatabaseService {
    @fx(db, log);                   // may only use db and log effects
    @ens(|result| result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────
// §8 — Practical example: layered architecture
// ─────────────────────────────────────────────────────────────────────
//
// Effects naturally enforce architectural layering:
//   - Domain logic is pure (no effects)
//   - Application layer uses db + log
//   - Presentation layer adds io

// Domain layer — pure business rules.
struct Order {
    id: u64,
    item: String,
    quantity: u32,
    price_cents: u64,
}

fn calculate_total(order: &Order) -> u64 {
    order.quantity as u64 * order.price_cents
}

fn apply_discount(total: u64, percent: u32) -> u64 {
    total - (total * percent as u64 / 100)
}

fn validate_order(order: &Order) -> Result<(), String> {
    if order.quantity == 0 {
        return Err("quantity must be > 0".to_string());
    }
    if order.price_cents == 0 {
        return Err("price must be > 0".to_string());
    }
    Ok(())
}

// Application layer — orchestrated with db + log effects.
fn save_order(order: &Order) / db + log -> Result<(), String> {
    validate_order(order)?;
    let total = calculate_total(order);
    let discounted = apply_discount(total, 10);

    db::put(
        &format!("{order.id}"),
        &format!("{order.item}|{order.quantity}|{discounted}"),
    );
    log::info(&format!("Order {order.id} saved — total: {discounted} cents"));
    Ok(())
}

// Presentation layer — adds io for user interaction.
fn print_receipt(order: &Order) / io + db + log {
    let total = calculate_total(order);
    let discounted = apply_discount(total, 10);

    println!("╔══════════════════════════╗");
    println!("║       RECEIPT            ║");
    println!("╠══════════════════════════╣");
    println!("║ Order:    {order.id:<15}║");
    println!("║ Item:     {order.item:<15}║");
    println!("║ Qty:      {order.quantity:<15}║");
    println!("║ Subtotal: {total:<15}║");
    println!("║ Discount: 10%            ║");
    println!("║ Total:    {discounted:<15}║");
    println!("╚══════════════════════════╝");

    log::info(&format!("Receipt printed for order {order.id}"));
}

// ─────────────────────────────────────────────────────────────────────
// §9 — Entry point — wiring handlers to effects
// ─────────────────────────────────────────────────────────────────────

pub fn main() / io {
    println!("=== MechGen Effects Showcase ===");
    println!("");

    // §2 — Pure functions.
    println!("-- Pure functions --");
    println!("add(3, 4) = {add(3, 4)}");
    println!("fibonacci(10) = {fibonacci(10)}");
    let items = vec![1, 2, 3];
    println!("reverse([1,2,3]) = {reverse(&items):?}");
    println!("");

    // §3 — Single effect annotation.
    println!("-- Single effect (/ io) --");
    greet("MechGen");
    println!("");

    // §5 — Effect polymorphism.
    println!("-- Effect polymorphism --");
    demo_polymorphism();
    println!("");

    // §6 + §8 — Handlers + layered architecture.
    println!("-- Layered architecture with handlers --");
    let mut db_handler = MemoryDb::new();
    let logger = ConsoleLogger::new();

    let order = Order {
        id: 1001,
        item: "Widget".to_string(),
        quantity: 3,
        price_cents: 1500,
    };

    // Install handlers and run effectful code.
    handle log = logger, db = db_handler {
        save_order(&order).unwrap();
        print_receipt(&order);
    }

    println!("");
    println!("=== Done ===");
}
