# Step 5: What's Next?

You've installed MAGE, written your first program, learned the syntax,
and know how to build, run, and test. Here's where to go from here.

---

## Learn More

| Resource                   | Description                                                  | Link                               |
| -------------------------- | ------------------------------------------------------------ | ---------------------------------- |
| **The MAGE Book**         | Comprehensive language guide covering every feature in depth | [book/](../book/README.md)         |
| **Cookbook**               | 65+ copy-paste recipes for common tasks                      | [cookbook/](../cookbook/README.md) |
| **Language Specification** | Formal syntax and semantics reference                        | [MAGE_SPEC.md](../MAGE_SPEC.md)  |
| **Standard Library**       | Module reference for `std.*`                                 | [stdlib/](../stdlib/std/)          |

## For Specific Audiences

| You are...                 | Read this                                                                   |
| -------------------------- | --------------------------------------------------------------------------- |
| An **AI agent** developer  | [Agent Guide](../agent-guide/README.md) — patterns for agents writing MAGE |
| Coming from **Rust**       | [Migration Guide](../migration-guide/README.md) — Rust → MAGE translation  |
| A **compiler contributor** | [Internals Guide](../internals/README.md) — compiler architecture           |

## Try These Next

### 1. Build a Small Project

```MAGE
// A simple CLI calculator
//
// `io` is a capability namespace, in scope everywhere — there is nothing to
// import. `words` splits on whitespace; `?=` is match, and `?` / `:` is
// if / else.

+f main() / io {
    p"Enter expression (e.g. 2 + 3):"
    v line = io.read_line()
    v parts = words(line)

    ? len(parts) == 3 {
        v a = 2.0
        v op = parts[1]
        v b = 3.0

        v result = ?= op {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" => a / b,
            _ => 0.0,
        }

        p"{a} {op} {b} = {result}"
    } : {
        ep"usage: <number> <op> <number>"
    }
}
```

### 2. Explore the Standard Vocabulary

There is no module system and nothing to import. The 31-word standard
vocabulary and the capability namespaces are in scope in every file:

```
map filter fold reduce sum len count sort reverse zip freq first last any all
find take range keys values flatten group scan contains split join chars words
lines upper lower

io fs net env time rng process alloc panic ffi async agent llm gpu npu json kb
db mem thread
```

`use` parses for source compatibility and imports nothing — the checker warns
when it sees one.

### 3. Write Tests for Your Code

```MAGE
f calculate(a: f64, op: str, b: f64) -> f64 {
    ?= op {
        "+" => a + b,
        "/" => a / b,
        _ => 0.0,
    }
}

// A test is a function marked `@test`. Its value is what the runner checks —
// there is no `assert!` macro.
@test
f test_calculator() -> bool {
    calculate(2.0, "+", 3.0) == 5.0 && calculate(10.0, "/", 2.0) == 5.0
}
```

### 4. Set Up Your Editor

Install the VS Code extension for the best experience:
- Syntax highlighting
- Error highlighting as you type
- Hover information and completions

## Community

- [GitHub Repository](https://github.com/nervosys/MAGE) — source code,
  issues, and discussions
- [MAGE_PROPOSAL.md](../MAGE_PROPOSAL.md) — the original language
  design proposal

---

**Welcome to MAGE. Happy coding!**
