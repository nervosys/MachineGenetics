# Redox Example Projects

Complete, self-contained example projects demonstrating Redox language features
and ecosystem patterns. Each project has a `Forge.toml` manifest and can be
built with `rdx build`.

## Examples

| Project                                       | Description                      | Key Features                                          |
| --------------------------------------------- | -------------------------------- | ----------------------------------------------------- |
| [hello-world](hello-world/)                   | Minimal Redox program            | Entry point, printing, variables                      |
| [data-structures](data-structures/)           | Structs, enums, generics, traits | Type definitions, impl blocks, pattern matching       |
| [http-client](http-client/)                   | Async HTTP client                | Effects, async/await, error handling, JSON            |
| [cli-tool](cli-tool/)                         | Command-line grep utility        | File I/O, iterators, argument parsing                 |
| [agent-swarm](agent-swarm/)                   | Multi-agent task coordination    | Agent primitives, swarm, consensus, leases            |
| [effects-showcase](effects-showcase/)         | Effect system demonstrations     | Effect declarations, handlers, composition            |
| [autonomous-pipeline](autonomous-pipeline/)   | AI code generation pipeline      | Task decomposition, contracts, cost oracle, pipelines |
| [swarm-code-review](swarm-code-review/)       | Multi-agent code review          | Scatter/gather swarm, consensus voting, memory        |
| [safe-plugin-host](safe-plugin-host/)         | Capability-based plugin sandbox  | Sandboxing, resource limits, audit trails             |
| [live-compiler](live-compiler/)               | Hot-reload dev server            | Hot patching, self-healing, rollback, versioning      |
| [multilang-bindings](multilang-bindings/)     | Cross-language FFI bridge        | FFI targets (C/Python/WASM), type mapping, codegen    |
| [cost-aware-optimizer](cost-aware-optimizer/) | Cost-model compilation optimizer | Multi-objective scoring, calibration, budget pruning  |

## Running an Example

```bash
cd examples/hello-world
rdx run
```

## Transpiling to Rust

Any example can be back-transpiled to Rust with:

```bash
rdx2rs src/main.rdx --output rs/
```

## Project Structure

Each example follows the standard Redox project layout:

```
example-name/
├── Forge.toml          # Project manifest
└── src/
    └── main.rdx        # Entry point (or lib.rdx for libraries)
```
