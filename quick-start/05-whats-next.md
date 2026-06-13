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

u std.io

+f main() / io {
    p"Enter expression (e.g. 2 + 3):"
    v line = io.stdin().read_line()?
    v parts = line.trim().split(' ').collect[Vec[&s]]()

    ? parts.len() == 3 {
        v a: f64 = parts[0].parse()?
        v op = parts[1]
        v b: f64 = parts[2].parse()?

        v result = ? op {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" => a / b,
            _ => { ep"Unknown operator: {op}"; ret }
        }

        p"{a} {op} {b} = {result}"
    } : {
        ep"Usage: <number> <op> <number>"
    }
}
```

### 2. Explore the Standard Library

```MAGE
u std.fs
u std.collections.{HashMap, BTreeMap}
u std.io.{Read, Write, BufRead}
u std.net.TcpStream
```

See [stdlib/std/](../stdlib/std/) for all available modules.

### 3. Write Tests for Your Code

```MAGE
@test
f test_calculator() {
    assert(calculate("2 + 3") == 5.0)
    assert(calculate("10 / 2") == 5.0)
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
