// Cost Oracle for the Redox compiler (REDOX_PROPOSAL.md P38).
//
// Per-target cost queries for expressions, types, and operations.
// Exposes `cost.query(expr, target)` and `cost.compare(expr, targets)` APIs.
// Seeded with initial cost models for x86-64, AArch64, and WASM.
//
// Cost dimensions: latency (cycles), throughput (ops/sec), memory (bytes),
// energy (millijoules), allocation count, and token count.

use std::collections::BTreeMap;

// ── Cost Types ─────────────────────────────────────────────────────────────

/// A cost estimate for a single expression/type/operation on a specific target.
#[derive(Debug, Clone, PartialEq)]
pub struct Cost {
    /// Latency in CPU cycles (approximate)
    pub latency_cycles: u64,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Number of heap allocations
    pub alloc_count: u32,
    /// Energy in microjoules (approximate)
    pub energy_uj: u64,
    /// Throughput in millions of operations per second (0 if not applicable)
    pub throughput_mops: f64,
    /// Token count in Redox compact form
    pub token_count: u32,
}

impl Cost {
    pub fn zero() -> Self {
        Self {
            latency_cycles: 0,
            memory_bytes: 0,
            alloc_count: 0,
            energy_uj: 0,
            throughput_mops: 0.0,
            token_count: 0,
        }
    }

    /// Combine two costs (sequential execution).
    pub fn add(&self, other: &Cost) -> Cost {
        Cost {
            latency_cycles: self.latency_cycles + other.latency_cycles,
            memory_bytes: self.memory_bytes + other.memory_bytes,
            alloc_count: self.alloc_count + other.alloc_count,
            energy_uj: self.energy_uj + other.energy_uj,
            throughput_mops: 0.0, // not additive
            token_count: self.token_count + other.token_count,
        }
    }

    /// Scale cost by a factor (e.g., loop iterations).
    pub fn scale(&self, factor: u64) -> Cost {
        Cost {
            latency_cycles: self.latency_cycles * factor,
            memory_bytes: self.memory_bytes,
            alloc_count: self.alloc_count * factor as u32,
            energy_uj: self.energy_uj * factor,
            throughput_mops: self.throughput_mops,
            token_count: self.token_count,
        }
    }

    /// Is this cost strictly cheaper than another on all dimensions?
    pub fn dominates(&self, other: &Cost) -> bool {
        self.latency_cycles <= other.latency_cycles
            && self.memory_bytes <= other.memory_bytes
            && self.alloc_count <= other.alloc_count
            && self.energy_uj <= other.energy_uj
    }
}

/// A target architecture for cost modeling.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    X86_64,
    AArch64,
    Wasm32,
    Custom(String),
}

impl Target {
    pub fn name(&self) -> &str {
        match self {
            Target::X86_64 => "x86-64",
            Target::AArch64 => "aarch64",
            Target::Wasm32 => "wasm32",
            Target::Custom(name) => name,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "x86-64" | "x86_64" => Target::X86_64,
            "aarch64" | "arm64" => Target::AArch64,
            "wasm32" | "wasm" => Target::Wasm32,
            other => Target::Custom(other.to_string()),
        }
    }

    pub fn all_standard() -> Vec<Target> {
        vec![Target::X86_64, Target::AArch64, Target::Wasm32]
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// The kind of construct being costed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CostSubject {
    /// A type (e.g., "Vec<i32>", "HashMap<String, u64>")
    Type(String),
    /// An operation (e.g., "add_i32", "div_f64", "alloc")
    Operation(String),
    /// An expression pattern (e.g., "vec_push", "hashmap_insert")
    Expression(String),
}

impl CostSubject {
    pub fn type_of(name: &str) -> Self {
        CostSubject::Type(name.to_string())
    }

    pub fn operation(name: &str) -> Self {
        CostSubject::Operation(name.to_string())
    }

    pub fn expression(name: &str) -> Self {
        CostSubject::Expression(name.to_string())
    }

    pub fn name(&self) -> &str {
        match self {
            CostSubject::Type(n) | CostSubject::Operation(n) | CostSubject::Expression(n) => n,
        }
    }
}

// ── Cost Oracle ────────────────────────────────────────────────────────────

/// The Cost Oracle: queryable cost model per target.
pub struct CostOracle {
    /// Cost entries indexed by (subject, target)
    costs: BTreeMap<(String, String), Cost>,
}

impl CostOracle {
    /// Create a new Cost Oracle pre-seeded with standard cost models.
    pub fn new() -> Self {
        let mut oracle = Self {
            costs: BTreeMap::new(),
        };
        oracle.seed_x86_64();
        oracle.seed_aarch64();
        oracle.seed_wasm32();
        oracle
    }

    /// Create an empty oracle (no cost models).
    pub fn empty() -> Self {
        Self {
            costs: BTreeMap::new(),
        }
    }

    /// Query the cost of a subject on a specific target.
    pub fn query(&self, subject: &CostSubject, target: &Target) -> Option<&Cost> {
        let key = (subject.name().to_string(), target.name().to_string());
        self.costs.get(&key)
    }

    /// Compare the cost of a subject across multiple targets.
    pub fn compare(&self, subject: &CostSubject, targets: &[Target]) -> CostComparison {
        let mut entries = Vec::new();
        for target in targets {
            let cost = self.query(subject, target).cloned();
            entries.push(ComparisonEntry {
                target: target.clone(),
                cost,
            });
        }
        CostComparison {
            subject: subject.clone(),
            entries,
        }
    }

    /// Compare costs across all standard targets.
    pub fn compare_all(&self, subject: &CostSubject) -> CostComparison {
        self.compare(subject, &Target::all_standard())
    }

    /// Register a cost entry.
    pub fn register(&mut self, subject: &CostSubject, target: &Target, cost: Cost) {
        let key = (subject.name().to_string(), target.name().to_string());
        self.costs.insert(key, cost);
    }

    /// Get total number of registered cost entries.
    pub fn entry_count(&self) -> usize {
        self.costs.len()
    }

    /// List all subjects that have cost data for a given target.
    pub fn subjects_for_target(&self, target: &Target) -> Vec<String> {
        let target_name = target.name().to_string();
        self.costs.keys()
            .filter(|(_, t)| *t == target_name)
            .map(|(s, _)| s.clone())
            .collect()
    }

    /// Find the cheapest target for a given subject (by latency).
    pub fn cheapest_target(&self, subject: &CostSubject) -> Option<(Target, &Cost)> {
        let mut best: Option<(Target, &Cost)> = None;
        for target in Target::all_standard() {
            if let Some(cost) = self.query(subject, &target) {
                if best.is_none() || cost.latency_cycles < best.as_ref().unwrap().1.latency_cycles {
                    best = Some((target, cost));
                }
            }
        }
        best
    }

    // ── Seed cost models ──

    fn seed_x86_64(&mut self) {
        let t = Target::X86_64;

        // Arithmetic operations
        self.register(&CostSubject::operation("add_i32"), &t, Cost {
            latency_cycles: 1, memory_bytes: 0, alloc_count: 0,
            energy_uj: 1, throughput_mops: 4000.0, token_count: 1,
        });
        self.register(&CostSubject::operation("add_i64"), &t, Cost {
            latency_cycles: 1, memory_bytes: 0, alloc_count: 0,
            energy_uj: 1, throughput_mops: 4000.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_i32"), &t, Cost {
            latency_cycles: 3, memory_bytes: 0, alloc_count: 0,
            energy_uj: 3, throughput_mops: 1500.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_i64"), &t, Cost {
            latency_cycles: 3, memory_bytes: 0, alloc_count: 0,
            energy_uj: 3, throughput_mops: 1500.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_i32"), &t, Cost {
            latency_cycles: 26, memory_bytes: 0, alloc_count: 0,
            energy_uj: 15, throughput_mops: 200.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_i64"), &t, Cost {
            latency_cycles: 40, memory_bytes: 0, alloc_count: 0,
            energy_uj: 25, throughput_mops: 130.0, token_count: 1,
        });
        self.register(&CostSubject::operation("add_f64"), &t, Cost {
            latency_cycles: 4, memory_bytes: 0, alloc_count: 0,
            energy_uj: 5, throughput_mops: 2000.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_f64"), &t, Cost {
            latency_cycles: 5, memory_bytes: 0, alloc_count: 0,
            energy_uj: 6, throughput_mops: 1600.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_f64"), &t, Cost {
            latency_cycles: 15, memory_bytes: 0, alloc_count: 0,
            energy_uj: 12, throughput_mops: 400.0, token_count: 1,
        });

        // Memory operations
        self.register(&CostSubject::operation("alloc"), &t, Cost {
            latency_cycles: 100, memory_bytes: 0, alloc_count: 1,
            energy_uj: 50, throughput_mops: 50.0, token_count: 2,
        });
        self.register(&CostSubject::operation("dealloc"), &t, Cost {
            latency_cycles: 80, memory_bytes: 0, alloc_count: 0,
            energy_uj: 40, throughput_mops: 60.0, token_count: 0,
        });
        self.register(&CostSubject::operation("memcpy"), &t, Cost {
            latency_cycles: 10, memory_bytes: 0, alloc_count: 0,
            energy_uj: 8, throughput_mops: 800.0, token_count: 0,
        });
        self.register(&CostSubject::operation("cache_miss"), &t, Cost {
            latency_cycles: 200, memory_bytes: 0, alloc_count: 0,
            energy_uj: 100, throughput_mops: 0.0, token_count: 0,
        });

        // Type costs (construction/access overhead)
        self.register(&CostSubject::type_of("Vec<T>"), &t, Cost {
            latency_cycles: 100, memory_bytes: 24, alloc_count: 1,
            energy_uj: 50, throughput_mops: 0.0, token_count: 2,
        });
        self.register(&CostSubject::type_of("[T; N]"), &t, Cost {
            latency_cycles: 0, memory_bytes: 0, alloc_count: 0,
            energy_uj: 0, throughput_mops: 0.0, token_count: 3,
        });
        self.register(&CostSubject::type_of("SmallVec<T, N>"), &t, Cost {
            latency_cycles: 5, memory_bytes: 0, alloc_count: 0,
            energy_uj: 3, throughput_mops: 0.0, token_count: 4,
        });
        self.register(&CostSubject::type_of("HashMap<K, V>"), &t, Cost {
            latency_cycles: 200, memory_bytes: 48, alloc_count: 1,
            energy_uj: 100, throughput_mops: 0.0, token_count: 3,
        });
        self.register(&CostSubject::type_of("BTreeMap<K, V>"), &t, Cost {
            latency_cycles: 150, memory_bytes: 24, alloc_count: 1,
            energy_uj: 80, throughput_mops: 0.0, token_count: 3,
        });
        self.register(&CostSubject::type_of("String"), &t, Cost {
            latency_cycles: 100, memory_bytes: 24, alloc_count: 1,
            energy_uj: 50, throughput_mops: 0.0, token_count: 1,
        });
        self.register(&CostSubject::type_of("Box<T>"), &t, Cost {
            latency_cycles: 100, memory_bytes: 8, alloc_count: 1,
            energy_uj: 50, throughput_mops: 0.0, token_count: 2,
        });
        self.register(&CostSubject::type_of("Arc<T>"), &t, Cost {
            latency_cycles: 120, memory_bytes: 16, alloc_count: 1,
            energy_uj: 60, throughput_mops: 0.0, token_count: 2,
        });
        self.register(&CostSubject::type_of("Rc<T>"), &t, Cost {
            latency_cycles: 110, memory_bytes: 16, alloc_count: 1,
            energy_uj: 55, throughput_mops: 0.0, token_count: 2,
        });
        self.register(&CostSubject::type_of("Option<T>"), &t, Cost {
            latency_cycles: 0, memory_bytes: 0, alloc_count: 0,
            energy_uj: 0, throughput_mops: 0.0, token_count: 2,
        });

        // Expression cost patterns
        self.register(&CostSubject::expression("vec_push"), &t, Cost {
            latency_cycles: 5, memory_bytes: 0, alloc_count: 0,
            energy_uj: 3, throughput_mops: 800.0, token_count: 3,
        });
        self.register(&CostSubject::expression("hashmap_insert"), &t, Cost {
            latency_cycles: 50, memory_bytes: 0, alloc_count: 0,
            energy_uj: 25, throughput_mops: 80.0, token_count: 4,
        });
        self.register(&CostSubject::expression("hashmap_get"), &t, Cost {
            latency_cycles: 30, memory_bytes: 0, alloc_count: 0,
            energy_uj: 15, throughput_mops: 120.0, token_count: 3,
        });
        self.register(&CostSubject::expression("sort_slice"), &t, Cost {
            latency_cycles: 500, memory_bytes: 0, alloc_count: 0,
            energy_uj: 200, throughput_mops: 10.0, token_count: 3,
        });
        self.register(&CostSubject::expression("clone_deep"), &t, Cost {
            latency_cycles: 300, memory_bytes: 0, alloc_count: 1,
            energy_uj: 150, throughput_mops: 15.0, token_count: 2,
        });
        self.register(&CostSubject::expression("format_string"), &t, Cost {
            latency_cycles: 200, memory_bytes: 64, alloc_count: 1,
            energy_uj: 100, throughput_mops: 20.0, token_count: 4,
        });
    }

    fn seed_aarch64(&mut self) {
        let t = Target::AArch64;

        // AArch64 arithmetic: generally similar latency, lower energy
        self.register(&CostSubject::operation("add_i32"), &t, Cost {
            latency_cycles: 1, memory_bytes: 0, alloc_count: 0,
            energy_uj: 1, throughput_mops: 3200.0, token_count: 1,
        });
        self.register(&CostSubject::operation("add_i64"), &t, Cost {
            latency_cycles: 1, memory_bytes: 0, alloc_count: 0,
            energy_uj: 1, throughput_mops: 3200.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_i32"), &t, Cost {
            latency_cycles: 3, memory_bytes: 0, alloc_count: 0,
            energy_uj: 2, throughput_mops: 1200.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_i64"), &t, Cost {
            latency_cycles: 4, memory_bytes: 0, alloc_count: 0,
            energy_uj: 3, throughput_mops: 1000.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_i32"), &t, Cost {
            latency_cycles: 12, memory_bytes: 0, alloc_count: 0,
            energy_uj: 8, throughput_mops: 350.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_i64"), &t, Cost {
            latency_cycles: 20, memory_bytes: 0, alloc_count: 0,
            energy_uj: 12, throughput_mops: 200.0, token_count: 1,
        });
        self.register(&CostSubject::operation("add_f64"), &t, Cost {
            latency_cycles: 4, memory_bytes: 0, alloc_count: 0,
            energy_uj: 3, throughput_mops: 1600.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_f64"), &t, Cost {
            latency_cycles: 5, memory_bytes: 0, alloc_count: 0,
            energy_uj: 4, throughput_mops: 1300.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_f64"), &t, Cost {
            latency_cycles: 12, memory_bytes: 0, alloc_count: 0,
            energy_uj: 8, throughput_mops: 500.0, token_count: 1,
        });

        // Memory
        self.register(&CostSubject::operation("alloc"), &t, Cost {
            latency_cycles: 120, memory_bytes: 0, alloc_count: 1,
            energy_uj: 35, throughput_mops: 40.0, token_count: 2,
        });
        self.register(&CostSubject::operation("dealloc"), &t, Cost {
            latency_cycles: 90, memory_bytes: 0, alloc_count: 0,
            energy_uj: 25, throughput_mops: 50.0, token_count: 0,
        });
        self.register(&CostSubject::operation("cache_miss"), &t, Cost {
            latency_cycles: 150, memory_bytes: 0, alloc_count: 0,
            energy_uj: 60, throughput_mops: 0.0, token_count: 0,
        });

        // Types
        self.register(&CostSubject::type_of("Vec<T>"), &t, Cost {
            latency_cycles: 120, memory_bytes: 24, alloc_count: 1,
            energy_uj: 35, throughput_mops: 0.0, token_count: 2,
        });
        self.register(&CostSubject::type_of("[T; N]"), &t, Cost {
            latency_cycles: 0, memory_bytes: 0, alloc_count: 0,
            energy_uj: 0, throughput_mops: 0.0, token_count: 3,
        });
        self.register(&CostSubject::type_of("HashMap<K, V>"), &t, Cost {
            latency_cycles: 220, memory_bytes: 48, alloc_count: 1,
            energy_uj: 70, throughput_mops: 0.0, token_count: 3,
        });
        self.register(&CostSubject::type_of("String"), &t, Cost {
            latency_cycles: 120, memory_bytes: 24, alloc_count: 1,
            energy_uj: 35, throughput_mops: 0.0, token_count: 1,
        });
        self.register(&CostSubject::type_of("Box<T>"), &t, Cost {
            latency_cycles: 120, memory_bytes: 8, alloc_count: 1,
            energy_uj: 35, throughput_mops: 0.0, token_count: 2,
        });

        // Expressions
        self.register(&CostSubject::expression("vec_push"), &t, Cost {
            latency_cycles: 6, memory_bytes: 0, alloc_count: 0,
            energy_uj: 2, throughput_mops: 650.0, token_count: 3,
        });
        self.register(&CostSubject::expression("hashmap_insert"), &t, Cost {
            latency_cycles: 60, memory_bytes: 0, alloc_count: 0,
            energy_uj: 18, throughput_mops: 65.0, token_count: 4,
        });
        self.register(&CostSubject::expression("sort_slice"), &t, Cost {
            latency_cycles: 600, memory_bytes: 0, alloc_count: 0,
            energy_uj: 150, throughput_mops: 8.0, token_count: 3,
        });
    }

    fn seed_wasm32(&mut self) {
        let t = Target::Wasm32;

        // WASM: interpreted/JIT, higher overhead
        self.register(&CostSubject::operation("add_i32"), &t, Cost {
            latency_cycles: 1, memory_bytes: 0, alloc_count: 0,
            energy_uj: 2, throughput_mops: 2000.0, token_count: 1,
        });
        self.register(&CostSubject::operation("add_i64"), &t, Cost {
            latency_cycles: 2, memory_bytes: 0, alloc_count: 0,
            energy_uj: 3, throughput_mops: 1500.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_i32"), &t, Cost {
            latency_cycles: 4, memory_bytes: 0, alloc_count: 0,
            energy_uj: 5, throughput_mops: 800.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_i64"), &t, Cost {
            latency_cycles: 8, memory_bytes: 0, alloc_count: 0,
            energy_uj: 10, throughput_mops: 400.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_i32"), &t, Cost {
            latency_cycles: 35, memory_bytes: 0, alloc_count: 0,
            energy_uj: 30, throughput_mops: 100.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_i64"), &t, Cost {
            latency_cycles: 60, memory_bytes: 0, alloc_count: 0,
            energy_uj: 50, throughput_mops: 50.0, token_count: 1,
        });
        self.register(&CostSubject::operation("add_f64"), &t, Cost {
            latency_cycles: 5, memory_bytes: 0, alloc_count: 0,
            energy_uj: 7, throughput_mops: 1000.0, token_count: 1,
        });
        self.register(&CostSubject::operation("mul_f64"), &t, Cost {
            latency_cycles: 8, memory_bytes: 0, alloc_count: 0,
            energy_uj: 10, throughput_mops: 600.0, token_count: 1,
        });
        self.register(&CostSubject::operation("div_f64"), &t, Cost {
            latency_cycles: 25, memory_bytes: 0, alloc_count: 0,
            energy_uj: 20, throughput_mops: 200.0, token_count: 1,
        });

        // Memory
        self.register(&CostSubject::operation("alloc"), &t, Cost {
            latency_cycles: 200, memory_bytes: 0, alloc_count: 1,
            energy_uj: 80, throughput_mops: 25.0, token_count: 2,
        });
        self.register(&CostSubject::operation("dealloc"), &t, Cost {
            latency_cycles: 150, memory_bytes: 0, alloc_count: 0,
            energy_uj: 60, throughput_mops: 30.0, token_count: 0,
        });

        // Types
        self.register(&CostSubject::type_of("Vec<T>"), &t, Cost {
            latency_cycles: 200, memory_bytes: 12, alloc_count: 1,
            energy_uj: 80, throughput_mops: 0.0, token_count: 2,
        });
        self.register(&CostSubject::type_of("[T; N]"), &t, Cost {
            latency_cycles: 0, memory_bytes: 0, alloc_count: 0,
            energy_uj: 0, throughput_mops: 0.0, token_count: 3,
        });
        self.register(&CostSubject::type_of("HashMap<K, V>"), &t, Cost {
            latency_cycles: 350, memory_bytes: 24, alloc_count: 1,
            energy_uj: 150, throughput_mops: 0.0, token_count: 3,
        });
        self.register(&CostSubject::type_of("String"), &t, Cost {
            latency_cycles: 200, memory_bytes: 12, alloc_count: 1,
            energy_uj: 80, throughput_mops: 0.0, token_count: 1,
        });

        // Expressions
        self.register(&CostSubject::expression("vec_push"), &t, Cost {
            latency_cycles: 10, memory_bytes: 0, alloc_count: 0,
            energy_uj: 8, throughput_mops: 400.0, token_count: 3,
        });
        self.register(&CostSubject::expression("hashmap_insert"), &t, Cost {
            latency_cycles: 80, memory_bytes: 0, alloc_count: 0,
            energy_uj: 40, throughput_mops: 40.0, token_count: 4,
        });
        self.register(&CostSubject::expression("sort_slice"), &t, Cost {
            latency_cycles: 1000, memory_bytes: 0, alloc_count: 0,
            energy_uj: 400, throughput_mops: 4.0, token_count: 3,
        });
    }
}

impl Default for CostOracle {
    fn default() -> Self {
        Self::new()
    }
}

// ── Comparison Result ──────────────────────────────────────────────────────

/// Result of comparing costs across targets.
#[derive(Debug, Clone)]
pub struct CostComparison {
    pub subject: CostSubject,
    pub entries: Vec<ComparisonEntry>,
}

#[derive(Debug, Clone)]
pub struct ComparisonEntry {
    pub target: Target,
    pub cost: Option<Cost>,
}

impl CostComparison {
    /// Find the target with the lowest latency.
    pub fn cheapest_by_latency(&self) -> Option<&ComparisonEntry> {
        self.entries.iter()
            .filter(|e| e.cost.is_some())
            .min_by_key(|e| e.cost.as_ref().unwrap().latency_cycles)
    }

    /// Find the target with the lowest energy usage.
    pub fn cheapest_by_energy(&self) -> Option<&ComparisonEntry> {
        self.entries.iter()
            .filter(|e| e.cost.is_some())
            .min_by_key(|e| e.cost.as_ref().unwrap().energy_uj)
    }

    /// Find the target with the lowest memory usage.
    pub fn cheapest_by_memory(&self) -> Option<&ComparisonEntry> {
        self.entries.iter()
            .filter(|e| e.cost.is_some())
            .min_by_key(|e| e.cost.as_ref().unwrap().memory_bytes)
    }

    /// Check if all targets have cost data.
    pub fn is_complete(&self) -> bool {
        self.entries.iter().all(|e| e.cost.is_some())
    }

    /// Format as human-readable text.
    pub fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Cost comparison for: {}\n", self.subject.name()));
        out.push_str("  Target      Latency(cyc)  Memory(B)  Allocs  Energy(uJ)\n");
        out.push_str("  ──────────  ───────────── ─────────  ──────  ──────────\n");
        for entry in &self.entries {
            if let Some(cost) = &entry.cost {
                out.push_str(&format!(
                    "  {:10} {:>13} {:>9} {:>7} {:>10}\n",
                    entry.target.name(),
                    cost.latency_cycles,
                    cost.memory_bytes,
                    cost.alloc_count,
                    cost.energy_uj,
                ));
            } else {
                out.push_str(&format!("  {:10}      (no data)\n", entry.target.name()));
            }
        }
        out
    }

    /// Format as JSON.
    pub fn format_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str(&format!("  \"subject\": \"{}\",\n", self.subject.name()));
        out.push_str("  \"targets\": {\n");
        for (i, entry) in self.entries.iter().enumerate() {
            out.push_str(&format!("    \"{}\": ", entry.target.name()));
            if let Some(cost) = &entry.cost {
                out.push_str(&format!(
                    "{{ \"latency_cycles\": {}, \"memory_bytes\": {}, \"alloc_count\": {}, \"energy_uj\": {}, \"throughput_mops\": {:.1}, \"token_count\": {} }}",
                    cost.latency_cycles, cost.memory_bytes, cost.alloc_count,
                    cost.energy_uj, cost.throughput_mops, cost.token_count,
                ));
            } else {
                out.push_str("null");
            }
            if i + 1 < self.entries.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  }\n");
        out.push_str("}\n");
        out
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn oracle() -> CostOracle {
        CostOracle::new()
    }

    // ── Cost type tests ──

    #[test]
    fn test_cost_zero() {
        let c = Cost::zero();
        assert_eq!(c.latency_cycles, 0);
        assert_eq!(c.memory_bytes, 0);
        assert_eq!(c.alloc_count, 0);
    }

    #[test]
    fn test_cost_add() {
        let a = Cost { latency_cycles: 10, memory_bytes: 8, alloc_count: 1, energy_uj: 5, throughput_mops: 0.0, token_count: 2 };
        let b = Cost { latency_cycles: 20, memory_bytes: 16, alloc_count: 0, energy_uj: 10, throughput_mops: 0.0, token_count: 3 };
        let sum = a.add(&b);
        assert_eq!(sum.latency_cycles, 30);
        assert_eq!(sum.memory_bytes, 24);
        assert_eq!(sum.alloc_count, 1);
        assert_eq!(sum.energy_uj, 15);
        assert_eq!(sum.token_count, 5);
    }

    #[test]
    fn test_cost_scale() {
        let c = Cost { latency_cycles: 10, memory_bytes: 8, alloc_count: 1, energy_uj: 5, throughput_mops: 100.0, token_count: 2 };
        let scaled = c.scale(100);
        assert_eq!(scaled.latency_cycles, 1000);
        assert_eq!(scaled.memory_bytes, 8); // memory doesn't scale
        assert_eq!(scaled.alloc_count, 100);
        assert_eq!(scaled.energy_uj, 500);
    }

    #[test]
    fn test_cost_dominates() {
        let a = Cost { latency_cycles: 5, memory_bytes: 8, alloc_count: 0, energy_uj: 3, throughput_mops: 0.0, token_count: 1 };
        let b = Cost { latency_cycles: 10, memory_bytes: 16, alloc_count: 1, energy_uj: 8, throughput_mops: 0.0, token_count: 2 };
        assert!(a.dominates(&b));
        assert!(!b.dominates(&a));
    }

    // ── Target tests ──

    #[test]
    fn test_target_from_name() {
        assert_eq!(Target::from_name("x86-64"), Target::X86_64);
        assert_eq!(Target::from_name("x86_64"), Target::X86_64);
        assert_eq!(Target::from_name("aarch64"), Target::AArch64);
        assert_eq!(Target::from_name("arm64"), Target::AArch64);
        assert_eq!(Target::from_name("wasm32"), Target::Wasm32);
        assert_eq!(Target::from_name("wasm"), Target::Wasm32);
        assert_eq!(Target::from_name("nvptx"), Target::Custom("nvptx".to_string()));
    }

    #[test]
    fn test_target_display() {
        assert_eq!(Target::X86_64.to_string(), "x86-64");
        assert_eq!(Target::AArch64.to_string(), "aarch64");
        assert_eq!(Target::Wasm32.to_string(), "wasm32");
    }

    #[test]
    fn test_all_standard_targets() {
        let targets = Target::all_standard();
        assert_eq!(targets.len(), 3);
    }

    // ── Oracle query tests ──

    #[test]
    fn test_oracle_seeded() {
        let o = oracle();
        assert!(o.entry_count() > 0);
    }

    #[test]
    fn test_query_add_i32_x86() {
        let o = oracle();
        let cost = o.query(&CostSubject::operation("add_i32"), &Target::X86_64).unwrap();
        assert_eq!(cost.latency_cycles, 1);
    }

    #[test]
    fn test_query_div_i64_x86() {
        let o = oracle();
        let cost = o.query(&CostSubject::operation("div_i64"), &Target::X86_64).unwrap();
        assert_eq!(cost.latency_cycles, 40);
    }

    #[test]
    fn test_query_vec_type_x86() {
        let o = oracle();
        let cost = o.query(&CostSubject::type_of("Vec<T>"), &Target::X86_64).unwrap();
        assert_eq!(cost.alloc_count, 1);
        assert_eq!(cost.memory_bytes, 24);
    }

    #[test]
    fn test_query_array_type_zero_cost() {
        let o = oracle();
        let cost = o.query(&CostSubject::type_of("[T; N]"), &Target::X86_64).unwrap();
        assert_eq!(cost.latency_cycles, 0);
        assert_eq!(cost.alloc_count, 0);
    }

    #[test]
    fn test_query_hashmap_insert() {
        let o = oracle();
        let cost = o.query(&CostSubject::expression("hashmap_insert"), &Target::X86_64).unwrap();
        assert!(cost.latency_cycles > 0);
    }

    #[test]
    fn test_query_nonexistent() {
        let o = oracle();
        assert!(o.query(&CostSubject::operation("quantum_teleport"), &Target::X86_64).is_none());
    }

    #[test]
    fn test_query_aarch64() {
        let o = oracle();
        let cost = o.query(&CostSubject::operation("add_i32"), &Target::AArch64).unwrap();
        assert_eq!(cost.latency_cycles, 1);
    }

    #[test]
    fn test_query_wasm() {
        let o = oracle();
        let cost = o.query(&CostSubject::operation("add_i32"), &Target::Wasm32).unwrap();
        assert_eq!(cost.latency_cycles, 1);
    }

    // ── Comparison tests ──

    #[test]
    fn test_compare_all_targets() {
        let o = oracle();
        let cmp = o.compare_all(&CostSubject::operation("div_i32"));
        assert_eq!(cmp.entries.len(), 3);
        assert!(cmp.is_complete());
    }

    #[test]
    fn test_compare_cheapest_latency() {
        let o = oracle();
        let cmp = o.compare_all(&CostSubject::operation("div_i64"));
        let cheapest = cmp.cheapest_by_latency().unwrap();
        // AArch64 has the lowest div_i64 latency (20 vs 40 vs 60)
        assert_eq!(cheapest.target, Target::AArch64);
    }

    #[test]
    fn test_compare_cheapest_energy() {
        let o = oracle();
        let cmp = o.compare_all(&CostSubject::operation("div_i64"));
        let cheapest = cmp.cheapest_by_energy().unwrap();
        assert_eq!(cheapest.target, Target::AArch64);
    }

    #[test]
    fn test_compare_format_text() {
        let o = oracle();
        let cmp = o.compare_all(&CostSubject::operation("add_i32"));
        let text = cmp.format_text();
        assert!(text.contains("x86-64"));
        assert!(text.contains("aarch64"));
        assert!(text.contains("wasm32"));
    }

    #[test]
    fn test_compare_format_json() {
        let o = oracle();
        let cmp = o.compare_all(&CostSubject::operation("mul_i32"));
        let json = cmp.format_json();
        assert!(json.contains("\"subject\""));
        assert!(json.contains("\"x86-64\""));
    }

    #[test]
    fn test_compare_incomplete() {
        let o = oracle();
        let cmp = o.compare(&CostSubject::operation("add_i32"), &[Target::X86_64, Target::Custom("risc-v".to_string())]);
        assert!(!cmp.is_complete()); // custom target has no data
    }

    // ── Vec vs array cost comparison (proposal example) ──

    #[test]
    fn test_vec_vs_array_cost() {
        let o = oracle();
        let vec_cost = o.query(&CostSubject::type_of("Vec<T>"), &Target::X86_64).unwrap();
        let arr_cost = o.query(&CostSubject::type_of("[T; N]"), &Target::X86_64).unwrap();
        assert!(arr_cost.dominates(vec_cost));
        assert_eq!(arr_cost.alloc_count, 0);
        assert_eq!(vec_cost.alloc_count, 1);
    }

    // ── Oracle customization ──

    #[test]
    fn test_register_custom() {
        let mut o = CostOracle::empty();
        o.register(&CostSubject::operation("custom_op"), &Target::X86_64, Cost {
            latency_cycles: 42, memory_bytes: 0, alloc_count: 0,
            energy_uj: 10, throughput_mops: 100.0, token_count: 1,
        });
        let cost = o.query(&CostSubject::operation("custom_op"), &Target::X86_64).unwrap();
        assert_eq!(cost.latency_cycles, 42);
    }

    #[test]
    fn test_cheapest_target() {
        let o = oracle();
        let (target, _cost) = o.cheapest_target(&CostSubject::operation("div_i64")).unwrap();
        assert_eq!(target, Target::AArch64); // 20 cycles vs 40/60
    }

    #[test]
    fn test_subjects_for_target() {
        let o = oracle();
        let subjects = o.subjects_for_target(&Target::X86_64);
        assert!(subjects.contains(&"add_i32".to_string()));
        assert!(subjects.contains(&"Vec<T>".to_string()));
    }

    // ── Proposal-aligned tests ──

    #[test]
    fn test_proposal_cost_dimensions() {
        // P19: latency, memory, allocation, energy, throughput, token count
        let o = oracle();
        let cost = o.query(&CostSubject::type_of("HashMap<K, V>"), &Target::X86_64).unwrap();
        assert!(cost.latency_cycles > 0);
        assert!(cost.memory_bytes > 0);
        assert!(cost.alloc_count > 0);
        assert!(cost.energy_uj > 0);
        assert!(cost.token_count > 0);
    }

    #[test]
    fn test_wasm_higher_overhead() {
        let o = oracle();
        let x86 = o.query(&CostSubject::operation("div_i32"), &Target::X86_64).unwrap();
        let wasm = o.query(&CostSubject::operation("div_i32"), &Target::Wasm32).unwrap();
        assert!(wasm.latency_cycles > x86.latency_cycles);
    }

    #[test]
    fn test_aarch64_lower_energy() {
        let o = oracle();
        let x86 = o.query(&CostSubject::operation("div_i32"), &Target::X86_64).unwrap();
        let arm = o.query(&CostSubject::operation("div_i32"), &Target::AArch64).unwrap();
        assert!(arm.energy_uj < x86.energy_uj);
    }

    #[test]
    fn test_default_oracle() {
        let o = CostOracle::default();
        assert!(o.entry_count() > 0);
    }
}
