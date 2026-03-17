# Redox Cookbook

A collection of practical recipes for common programming tasks in Redox.

Each recipe is a **self-contained, copy-paste example** that solves a specific
real-world problem. Recipes are organized by category and ordered from simple to
complex within each section.

## Categories

| File                             | Topics                                                          |
| -------------------------------- | --------------------------------------------------------------- |
| [io.md](io.md)                   | Read/write files, CSV parsing, directory walking, temp files    |
| [http.md](http.md)               | GET/POST requests, REST APIs, file downloads, webhooks          |
| [data.md](data.md)               | JSON processing, sorting, filtering, grouping, transforms       |
| [concurrency.md](concurrency.md) | Parallel tasks, channels, mutexes, rate limiting                |
| [agents.md](agents.md)           | Agent creation, swarms, message passing, consensus              |
| [cli.md](cli.md)                 | Argument parsing, progress bars, colored output, REPL           |
| [errors.md](errors.md)           | Custom errors, error chains, retry logic, fallbacks             |
| [testing.md](testing.md)         | Table-driven tests, mocking effects, property tests, benchmarks |

## How to use

Each recipe follows a consistent format:

```
### Recipe Title

**Problem**: What you want to accomplish.

**Solution**:
<code example>

**Discussion**: Why it works, trade-offs, related recipes.
```

All recipes use the standard library only — no external dependencies unless
explicitly noted.

## Contributing

To add a recipe:

1. Pick the appropriate category file
2. Add your recipe using the format above
3. Keep examples under 50 lines
4. Include effect annotations on all non-pure functions
5. Show expected output where applicable
