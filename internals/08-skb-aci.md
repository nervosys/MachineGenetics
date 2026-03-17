# Chapter 8: SKB Engine & Agentic Compiler Intelligence

The Safety Knowledge Base (SKB) externalises safety rules that Rust
encodes as borrow-checker/type-system invariants. Rather than requiring
the programmer to encode these rules in syntax, the compiler's SKB
engine applies them automatically. On top of the SKB sits the Agentic
Compiler Intelligence (ACI) — an AI-powered subsystem that learns from
the codebase, bug history, and swarm sessions to provide dynamic
warnings, intelligent debugging, and performance advice.

---

## 8.1 SKB Architecture

```
┌───────────────────────────────────────────┐
│                SKB Engine                 │
├───────────────────────────────────────────┤
│  ┌─────────┐ ┌──────────┐ ┌───────────┐  │
│  │ Rule DB │ │ Matcher  │ │ Fixer     │  │
│  │ (JSON)  │ │ (HIR/MIR)│ │(templates)│  │
│  └────┬────┘ └────┬─────┘ └─────┬─────┘  │
│       └───────────┼─────────────┘         │
│            ┌──────▼──────┐                │
│            │  Diagnostic │                │
│            │   Emitter   │                │
│            └──────┬──────┘                │
│                   ▼                       │
│          RAP / Compiler Output            │
└───────────────────────────────────────────┘
```

### Rule Database

Rules live in `skb/rules/` as JSON files organised by category:

```
skb/
├── manifest.json          # lists all rule files
├── rule-schema.json       # JSON Schema for rules
└── rules/
    ├── ownership.json     # OWN-xxxx rules
    ├── borrowing.json     # BR-xxxx rules
    ├── lifetimes.json     # LT-xxxx rules
    ├── type_safety.json   # TS-xxxx rules
    ├── concurrency.json   # CON-xxxx rules
    └── ffi.json           # FFI-xxxx rules
```

### Rule Schema

Every rule conforms to the schema in `skb/rule-schema.json`:

```json
{
    "id": "OWN-0042",
    "database": "ownership",
    "version": "1.0.0",
    "severity": "error",
    "category": "use-after-move",
    "description": "Value used after ownership transfer",
    "rationale": "Accessing a moved-out value reads freed or invalid memory",
    "scope": "function",
    "pattern": "let $x = ...; move($x); use($x)",
    "context": "!Copy trait on type of $x",
    "fix_template": {
        "action": "clone_before_move",
        "template": "let $x_copy = $x.clone(); move($x); use($x_copy)"
    }
}
```

Key fields:

| Field | Purpose |
|-------|---------|
| `id` | Unique identifier, pattern `[A-Z]{2,4}-[0-9]{3,4}` |
| `database` | Category: ownership, borrow, lifetime, type_safety, concurrency, ffi |
| `severity` | error, warning, info, hint |
| `scope` | Matching scope: function, module, crate, global |
| `pattern` | Structural pattern to match against HIR |
| `fix_template` | Automated fix that an agent can apply |

### Rule Matching

The matcher walks the HIR looking for patterns that match rule
constraints:

```rust
pub struct RuleMatch {
    pub rule_id: String,
    pub severity: Severity,
    pub span: Span,
    pub message: String,
    pub fix: Option<SuggestedFix>,
}

pub fn check_rules(hir: &HirModule, rules: &[Rule]) -> Vec<RuleMatch> {
    let mut matches = Vec::new();
    for rule in rules {
        for item in &hir.items {
            if let Some(m) = rule.pattern.try_match(item) {
                if rule.context_holds(item, &m) {
                    matches.push(RuleMatch {
                        rule_id: rule.id.clone(),
                        severity: rule.severity,
                        span: m.span,
                        message: rule.description.clone(),
                        fix: rule.fix_template.as_ref()
                            .map(|t| t.instantiate(&m)),
                    });
                }
            }
        }
    }
    matches
}
```

## 8.2 How SKB Replaces Borrow-Checker Syntax

In Rust, the programmer must annotate lifetimes, borrow modes, and
ownership transfers explicitly. In Redox, the SKB engine applies these
same rules silently:

| Rust Syntax | SKB Rule |
|-------------|----------|
| `fn foo<'a>(x: &'a str) -> &'a str` | LT-0001: Return reference must live as long as input |
| `fn bar(x: &mut Vec<i32>)` | BR-0012: Mutable borrow implies exclusive access |
| `let y = x; /* x moved */` | OWN-0042: Use after move is forbidden |
| `Arc::new(data)` | CON-0008: Arc provides thread-safe shared ownership |
| `unsafe { ptr::read(p) }` | FFI-0023: Raw pointer dereference requires validation |

The agent never writes `<'a>` or `&mut` annotations. Instead, the
compiler applies SKB rules during the type-check/effect-check phase,
reports violations as diagnostics, and provides automated fixes.

## 8.3 Agent Interaction with SKB

Agents query SKB rules through RAP:

### Query: Get Rules for a Function

```json
{
    "method": "skb/suggest",
    "params": { "source": "...", "func": "process_data" }
}
```

Response returns applicable rules and any violations.

### Query: Explain a Rule

```json
{
    "method": "skb/explain",
    "params": { "rule_id": "OWN-0042" }
}
```

Response includes the rule's description, rationale, examples, and fix
template.

### Auto-Fix Workflow

1. Agent writes Redox code (no safety annotations)
2. Compiler runs SKB matcher on the HIR
3. SKB engine returns violations with `fix_template`
4. Agent applies the fix template automatically
5. Recheck — no violations → code is safe

This replaces the "fight the borrow checker" cycle with a structured
query-fix protocol.

## 8.4 Agentic Compiler Intelligence (ACI)

ACI sits on top of the SKB and static analysis, adding AI-powered
capabilities. It comprises five subsystems:

```
┌─────────────────────────────────────────────────┐
│        Agentic Compiler Intelligence (ACI)      │
├─────────────────────────────────────────────────┤
│  ┌────────────┐ ┌───────────┐ ┌─────────────┐  │
│  │  Dynamic   │ │ Intelligent│ │  Performance │  │
│  │  Warning   │ │  Debugging │ │   Advisor    │  │
│  │  Engine    │ │   Engine   │ │   Engine     │  │
│  └──────┬─────┘ └─────┬─────┘ └──────┬──────┘  │
│         └────────┼─────────┴────────┘           │
│         ┌────────▼──────────────────┐           │
│         │   Codebase Model (LLM)   │           │
│         │ (learned from project,   │           │
│         │  swarm history, SKB)     │           │
│         └────────┬─────────────────┘           │
│  ┌───────────────▼────────────────────────┐    │
│  │  Swarm Coordination Intelligence      │    │
│  │  (conflict prediction, decomposition)  │    │
│  └────────────────────────────────────────┘    │
├─────────────────────────────────────────────────┤
│ Queryable via RAP: rap.query("aci.*", ...)      │
└─────────────────────────────────────────────────┘
```

### 8.4.1 Dynamic Warning Engine

Unlike static lints, dynamic warnings learn from the project's own bug
history:

```rust
v warnings = rap.query("aci.warnings", func_id)
// Returns warnings like:
// DynamicWarning {
//     id: "DW-1847",
//     message: "Pattern similar to bug #423 (off-by-one in range)",
//     confidence: 0.87,
//     source: WarningSource::ProjectHistory,
//     fix: SuggestedFix::AdjustRange { ... },
// }
```

**Learning sources:**

- **Project bug history**: Patterns from past bugs and fixes in the
  semantic VCS
- **Swarm session history**: Which agent changes caused regressions
- **SKB violation frequency**: Which rules are most commonly violated
  in this codebase
- **Cross-project patterns**: Anonymized aggregate patterns from the
  ecosystem (opt-in)

### 8.4.2 Intelligent Debugging Engine

When a runtime failure occurs, the debugging engine performs causal
reasoning:

```rust
v diagnosis = rap.query("aci.debug", FailureReport {
    symptom: "SIGSEGV at matrix_multiply:47",
    stack_trace: [...],
    input_sample: [...],
})
// Returns:
// Diagnosis {
//     root_cause: "Uninitialized memory read: buffer allocated at
//                  line 23 without zero-fill before GPU dispatch",
//     causal_chain: [
//         CausalStep { location: "alloc.rs:23", event: "Alloc without init" },
//         CausalStep { location: "dispatch.rs:31", event: "GPU dispatch" },
//         CausalStep { location: "kernel.rs:47", event: "Read uninit memory" },
//     ],
//     fix: SuggestedFix::InsertInit { location: "alloc.rs:24" },
//     related_skb_rule: "MEM-017",
// }
```

### 8.4.3 Performance Advisor Engine

The performance advisor uses MLIR cost models and profiling data:

```rust
v advice = rap.query("aci.perf", module_id)
// Returns:
// PerfAdvice {
//     target_func: "image_resize",
//     suggestion: "Add @pa(4) — estimated 3.2x speedup from autotuning",
//     estimated_improvement: 3.2,
//     confidence: 0.81,
//     evidence: "5 similar functions benefited from autotuning",
// }
```

### 8.4.4 Codebase Model

A fine-tuned LLM trained on the project's source code, SKB rules, and
swarm history. It powers all three engines above by providing:

- Pattern recognition across the codebase
- Similarity matching for bug patterns
- Natural language explanations of diagnostics
- Code generation for fix templates

### 8.4.5 Swarm Coordination Intelligence

Learns which swarm configurations and decomposition strategies work best:

```rust
v advice = rap.query("aci.swarm", task)
// SwarmAdvice {
//     recommended_swarm_size: 12,
//     recommended_decomposition: DecompositionStrategy::ModuleLevel,
//     predicted_conflicts: [
//         ConflictPrediction {
//             region_a: "auth::session",
//             region_b: "auth::token",
//             probability: 0.73,
//             mitigation: "Assign both to same synthesizer",
//         },
//     ],
// }
```

## 8.5 SKB + ACI Integration Points

### Compiler Pipeline Integration

```
Source → Lex → Parse → Resolve → TypeCheck → EffectCheck
                                      │           │
                                      ▼           ▼
                                   ┌──────────────────┐
                                   │   SKB Matcher    │
                                   │  (rule patterns  │
                                   │   against HIR)   │
                                   └────────┬─────────┘
                                            ▼
                                   ┌──────────────────┐
                                   │   ACI Engines    │
                                   │ (dynamic warn,   │
                                   │  debug, perf)    │
                                   └────────┬─────────┘
                                            ▼
                                    Diagnostics + Fixes
                                            │
                        ┌───────────────────┼─────────────────┐
                        ▼                   ▼                 ▼
                   Compiler Output     RAP Response     IDE Underlining
```

### RAP Query Routing

| Query Prefix | Target |
|-------------|--------|
| `skb.*` | SKB engine (rule matching, explanation, rule listing) |
| `aci.warnings` | Dynamic Warning Engine |
| `aci.debug` | Intelligent Debugging Engine |
| `aci.perf` | Performance Advisor Engine |
| `aci.swarm` | Swarm Coordination Intelligence |
| `aci.model` | Codebase Model (direct embedding queries) |

## 8.6 Creating Custom SKB Rules

Projects can extend the SKB with project-specific rules:

```json
{
    "id": "PROJ-0001",
    "database": "type_safety",
    "version": "1.0.0",
    "severity": "warning",
    "category": "api-contract",
    "description": "HTTP handler must return Result, not raw value",
    "rationale": "Unwrapped returns cause 500 errors on failures",
    "scope": "function",
    "pattern": "fn $handler(...) -> $T where has_attr($handler, 'route')",
    "context": "!is_result($T)",
    "fix_template": {
        "action": "wrap_result",
        "template": "fn $handler(...) -> R[$T, AppError]"
    }
}
```

Place custom rules in `skb/rules/` and add them to `skb/manifest.json`.
The compiler loads all rules at startup.

## 8.7 Testing SKB Rules

### Unit Tests

Each rule should have positive (fires) and negative (doesn't fire) test
cases:

```rust
#[test]
fn own_0042_detects_use_after_move() {
    let source = "f test() { v x = [1, 2, 3]~; v y = x; p\"{x}\" }";
    let diagnostics = check(source);
    assert!(diagnostics.iter().any(|d| d.rule_id == "OWN-0042"));
}

#[test]
fn own_0042_allows_copy_types() {
    let source = "f test() { v x: i32 = 42; v y = x; p\"{x}\" }";
    let diagnostics = check(source);
    assert!(diagnostics.iter().all(|d| d.rule_id != "OWN-0042"));
}
```

### Coverage

The `rdx skb coverage` command reports which rules have test cases:

```bash
rdx skb coverage
# OWN-0042  ✓ 3 positive, 2 negative
# BR-0012   ✓ 2 positive, 1 negative
# LT-0001   ✗ no tests
# ...
```

### Rule Validation

The `rdx skb validate` command checks all rules against the schema:

```bash
rdx skb validate
# ✓ 51 rules validated against rule-schema.json
# ✗ PROJ-0001: missing required field "rationale"
```
