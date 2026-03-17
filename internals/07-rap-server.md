# Chapter 7: RAP Server

RAP (Redox Agent Protocol) is the language-server component that exposes
compiler capabilities to AI agents, IDEs, and external tools over a
JSON-RPC protocol.

---

## 7.1 Design Goals

| Goal               | Rationale                                                |
| ------------------ | -------------------------------------------------------- |
| Agent-first        | Primary consumers are LLM agents, not humans             |
| Streaming          | Agents need partial results during long compilations     |
| Stateless queries  | Each request is self-contained — no session state        |
| Incremental future | Designed so a Salsa-based query engine can be dropped in |
| JSON-RPC           | Universal protocol — any language can connect            |

## 7.2 Transport

RAP runs as a TCP server accepting newline-delimited JSON-RPC 2.0
messages:

```
┌────────────┐        TCP (127.0.0.1:9876)        ┌──────────────┐
│   Agent    │  ──── JSON-RPC newline-delimited ────▶   RAP Server │
│ (LLM/IDE)  │  ◀──── JSON-RPC responses ───────────  (rdx rap)   │
└────────────┘                                     └──────────────┘
```

Starting the server:

```bash
rdx rap --bind 127.0.0.1:9876
```

The prototype implementation (`prototype/src/rap.rs`) binds via
`TcpListener`, reads one line per request, and writes one line per
response:

```rust
pub fn serve(addr: &str) {
    let listener = TcpListener::bind(addr).unwrap_or_else(|e| {
        eprintln!("rap: failed to bind {addr}: {e}");
        std::process::exit(1);
    });
    for stream in listener.incoming() {
        // handle_connection reads JSON-RPC lines
    }
}
```

## 7.3 Protocol Methods

### `language/tokens`

Tokenise source code and return the token stream.

**Request:**
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "language/tokens",
    "params": { "source": "+f add(a: i32, b: i32) -> i32 { a + b }" }
}
```

**Response:**
```json
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "tokens": [
            { "kind": "PubFn", "text": "+f", "line": 1, "col": 1 },
            { "kind": "Ident", "text": "add", "line": 1, "col": 4 },
            { "kind": "LParen", "text": "(", "line": 1, "col": 7 }
        ]
    }
}
```

### `language/parse`

Parse source code and return the AST as JSON.

**Request:**
```json
{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "language/parse",
    "params": { "source": "+f main() { p\"hello\" }" }
}
```

**Response (success):**
```json
{
    "jsonrpc": "2.0",
    "id": 2,
    "result": {
        "ok": true,
        "ast": {
            "items": [
                {
                    "visibility": "Public",
                    "kind": {
                        "Function": {
                            "name": "main",
                            "params": [],
                            "body": { "stmts": [...] }
                        }
                    }
                }
            ]
        }
    }
}
```

**Response (error):**
```json
{
    "jsonrpc": "2.0",
    "id": 2,
    "result": {
        "ok": false,
        "error": {
            "line": 1,
            "col": 12,
            "message": "expected `)`, found `{`"
        }
    }
}
```

### `build/check`

Run lex + parse and return all diagnostics (lexer errors and parse
errors combined).

**Request:**
```json
{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "build/check",
    "params": { "source": "+f bad( {" }
}
```

**Response:**
```json
{
    "jsonrpc": "2.0",
    "id": 3,
    "result": {
        "ok": false,
        "errors": [
            { "line": 1, "col": 8, "message": "expected `)`, found `{`" }
        ]
    }
}
```

## 7.4 Planned Methods

The protocol is designed to grow as the compiler matures:

| Method              | Description                                   | Status        |
| ------------------- | --------------------------------------------- | ------------- |
| `language/tokens`   | Tokenise source                               | ✅ Implemented |
| `language/parse`    | Parse to AST                                  | ✅ Implemented |
| `build/check`       | Lex + parse diagnostics                       | ✅ Implemented |
| `build/full`        | Full pipeline (type check, effects, MLIR)     | Planned       |
| `query/type`        | Query the type of an expression at a position | Planned       |
| `query/effects`     | Query the effect set of a function            | Planned       |
| `query/cost`        | Query the cost oracle for a function          | Planned       |
| `query/completions` | Return code completions at a position         | Planned       |
| `query/hover`       | Return hover information at a position        | Planned       |
| `query/definition`  | Go-to-definition                              | Planned       |
| `query/references`  | Find all references to a symbol               | Planned       |
| `skb/suggest`       | SKB rule suggestions for a code region        | Planned       |
| `skb/explain`       | Explain why a rule fired                      | Planned       |
| `agent/context`     | Return agent-optimised context for a file     | Planned       |
| `agent/refactor`    | Apply an agent-proposed refactoring           | Planned       |

## 7.5 Dispatch Architecture

The dispatcher is a simple pattern-match on the method string:

```rust
fn dispatch(method: &str, params: &serde_json::Value) -> serde_json::Value {
    let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");

    match method {
        "language/tokens" => { /* lex source, return tokens */ }
        "language/parse"  => { /* parse source, return AST */ }
        "build/check"     => { /* lex + parse, return errors */ }
        _ => serde_json::json!({ "error": format!("unknown method: {method}") }),
    }
}
```

The production dispatcher will use a trait-based registration system:

```rust
trait RapMethod {
    const NAME: &'static str;
    type Params: serde::de::DeserializeOwned;
    type Result: serde::Serialize;

    fn execute(ctx: &CompilerCtx, params: Self::Params) -> Self::Result;
}

struct TokensMethod;
impl RapMethod for TokensMethod {
    const NAME: &'static str = "language/tokens";
    type Params = SourceParams;
    type Result = TokensResult;

    fn execute(ctx: &CompilerCtx, params: SourceParams) -> TokensResult {
        let tokens = ctx.query::<Tokens>(params.source);
        TokensResult { tokens }
    }
}
```

## 7.6 VS Code Integration

The `redox-vscode` extension connects to the RAP server as an LSP-like
client:

```
┌──────────────┐       JSON-RPC/TCP       ┌──────────────┐
│  VS Code     │  ────────────────────▶   │  RAP Server  │
│  Extension   │  ◀────────────────────   │  (rdx rap)   │
│              │                          │              │
│  • Syntax    │  language/tokens ──────▶  │  • Lexer     │
│  • Errors    │  build/check ─────────▶  │  • Parser    │
│  • Hover     │  query/type ──────────▶  │  • TypeCheck │
│  • Complete  │  query/completions ───▶  │  • Resolve   │
└──────────────┘                          └──────────────┘
```

The extension provides:
- **Syntax highlighting**: TextMate grammar for `.rdx` files
- **Error underlining**: Maps `build/check` errors to VS Code diagnostics
- **Hover information**: Maps `query/type` to tooltip display
- **Completions**: Maps `query/completions` to VS Code completion items

## 7.7 Agent Interaction Patterns

### Pattern 1: Parse-and-Inspect

An agent sends source to `language/parse`, receives the AST, and reasons
about structure:

```python
import json, socket

def rap_call(method, source):
    s = socket.create_connection(("127.0.0.1", 9876))
    req = json.dumps({"jsonrpc": "2.0", "id": 1,
                       "method": method, "params": {"source": source}})
    s.sendall((req + "\n").encode())
    resp = s.makefile().readline()
    return json.loads(resp)["result"]

ast = rap_call("language/parse", '+f main() { p"hello" }')
```

### Pattern 2: Iterative Fix

An agent sends code, checks for errors, modifies the code, and
rechecks:

```python
code = '+f main() { v x: i32 = "oops" }'
result = rap_call("build/check", code)

while not result["ok"]:
    # Agent fixes code based on errors
    code = agent_fix(code, result["errors"])
    result = rap_call("build/check", code)
```

### Pattern 3: Context Gathering

An agent uses `agent/context` to get a structured summary before making
edits (planned method):

```python
context = rap_call("agent/context", file_source)
# Returns: imports, public API, type signatures, effect annotations
# Agent uses this summary instead of re-parsing the whole file
```

## 7.8 Future: Incremental Compilation

The current prototype recompiles from scratch on each request. The
production RAP will use the Salsa query engine for incremental
computation:

```rust
#[salsa::query_group(CompilerDatabase)]
trait CompilerDb {
    #[salsa::input]
    fn source(&self, file: FileId) -> Arc<String>;

    fn tokens(&self, file: FileId) -> Arc<Vec<Token>>;
    fn ast(&self, file: FileId) -> Arc<Module>;
    fn hir(&self, file: FileId) -> Arc<HirModule>;
    fn types(&self, file: FileId) -> Arc<TypeTable>;
    fn effects(&self, file: FileId) -> Arc<EffectMap>;
}
```

When an agent modifies one file, only the affected queries are
recomputed — other files are served from cache. This makes the edit →
check → fix cycle fast enough for real-time agent interaction.
