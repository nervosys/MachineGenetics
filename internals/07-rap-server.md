# Chapter 7: RAP Server

RAP (MAGE Agent Protocol) is the language-server component that exposes
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
│ (LLM/IDE)  │  ◀──── JSON-RPC responses ───────────  (mg rap)   │
└────────────┘                                     └──────────────┘
```

Starting the server:

```bash
mg rap --bind 127.0.0.1:9876
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

## 7.4 The method surface

**Corrected 2026-08-25.** This section was a "Planned Methods" table listing
three implemented methods and eleven planned ones. **38 methods are published
and dispatch** — the count is pinned, and a test exercises every one against
its published parameter list. None of the eleven exists under the name given,
and most of the capabilities they described shipped under a different one:

| §7.4 planned | What was actually built |
|---|---|
| `query/cost` | `cost/query`, `cost/compare` |
| `query/effects` | `effects/infer`, `effects/check` |
| `skb/suggest`, `skb/explain` | `skb/query`, `skb/rules`, `skb/spec` |
| `build/full` | `build/heal`, `build/recover`, `pipeline/recover-and-encode` |
| `query/type`, `query/completions`, `query/hover`, `query/definition`, `query/references` | nothing — these are the editor-service methods, and §7.6's VS Code integration is the part that did not happen |
| `agent/context` | nothing under that name; `format/agent` and `nl/*` cover some of the intent |

The naming convention that won is `namespace/verb` grouped by *subsystem*
(`cost/query`) rather than `query/noun` grouped by *operation* (`query/cost`).
That is worth knowing before adding a method.

The full published surface, generated from `MAGE_ONTOLOGY.json`'s
`rap_methods` section — which CI checks against a fresh `--emit-ontology`, so
this table cannot drift from the binary without the ontology drifting first:

| Method | Params | Summary |
|---|---|---|
| `abl/decode` | `abl_hex` | Agentic Binary Language bytes (hex) -> decompiled per-item view |
| `abl/encode` | `source` | Source -> Agentic Binary Language bytes (hex) |
| `abl/run` | `source` | Source -> encode -> CpuBackend dispatch |
| `attribute/compress` | `name` | Compress attributes back to shorthand |
| `attribute/expand` | `name` | Expand attribute shorthand |
| `build/check` | `source` | Lex + parse + report diagnostics |
| `build/heal` | `source` | Generate fix candidates for diagnostics |
| `build/recover` | `source` | Run the 5-stage recovery pipeline; return final source |
| `capability/check` | `source` | List capabilities required by source |
| `cost/compare` | `a`, `b`, `target` | Compare costs of two constructs |
| `cost/query` | `construct`, `target`, `opt` | Per-construct cost estimate |
| `doc/query` | `fqn` | Lookup documentation by FQN |
| `effects/check` | `source` | Check declared effects against inferred |
| `effects/infer` | `source` | Infer effects of each function |
| `elision/apply` | `source` | Apply elision rules to compact source |
| `format/agent` | `source` | Format source in agent-canonical sigil mode |
| `format/human` | `source` | Format source in human-readable keyword mode |
| `grammar/list` | — | List grammar extensions |
| `heal/graph` | `source` | Heal-pipeline diagnostic graph |
| `language/parse` | `source` | Parse source to AST (JSON) |
| `language/tokens` | `source` | Tokenize source |
| `lint/check` | `source` | Run lints on source |
| `manifest/generate` | `source`, `crate_name`, `version` | Generate capability manifest for a module |
| `nl/explain` | `source` | Explain source in natural language |
| `nl/generate` | `prompt` | Generate MAGE from a natural-language prompt |
| `nl/query` | `prompt` | General NL query against the SKB |
| `nl/refactor` | `source` | Refactor source via natural-language request |
| `ontology/full` | — | Return this complete ontology |
| `ontology/section` | `section` | Return one named section of the ontology |
| `pipeline/recover-and-encode` | `source` | Recover then encode Agentic Binary Language in one call |
| `sandbox/policy` | `source`, `agent` | Lookup sandbox policy by name |
| `skb/query` | `by`, `value` | Query structured knowledge base |
| `skb/rules` | `domain` | List SKB rules |
| `skb/spec` | `fqn` | Lookup spec block for a symbol |
| `token/report` | `source` | Per-construct token cost report for source |
| `verify/contracts` | `fqn`, `requires`, `ensures`, `declared_effects`, `used_effects` | Verify function contracts (req/ens/inv) |
| `verify/module` | `source` | Verify entire module |

Three of these are documented in detail in §7.3 above. The rest are single-turn
request/response in the same shape.

## 7.5 Dispatch Architecture

The dispatcher is a pattern-match on the method string, behind a gate that
separates *protocol* failure from *program* failure:

```rust
fn dispatch_checked(method: &str, params: &Value) -> Result<Value, RpcError> {
    if !METHODS.contains(&method) {
        return Err(RpcError::method_not_found(method));  // -32601
    }
    Ok(dispatch(method, params))
}

fn dispatch(method: &str, params: &serde_json::Value) -> serde_json::Value {
    let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("");

    match method {
        "language/tokens" => { /* lex source, return tokens */ }
        "language/parse"  => { /* parse source, return AST */ }
        "build/check"     => { /* lex + parse, return errors */ }
        _ => /* unreachable from the wire; guards METHODS-vs-arms drift */
    }
}
```

That split is the one thing to carry away from this section. **A MAGE program
that fails to check is a successful call.** `language/parse` answering
`{"ok": false, "error": {"line": 3, ...}}` inside `result` is correct: the
server was asked a question and answered it. What belongs in JSON-RPC's `error`
member is only the case where no call happened at all:

| Condition | Code | Member |
|---|---:|---|
| Frame is not JSON | -32700 | `error` |
| No string `method` | -32600 | `error` |
| Method not in `METHODS` | -32601 | `error` |
| Method ran; program was bad | — | `result`, with `ok: false` |

Until 2026-08-19 the first three also came back as `result`, so a client doing
the one thing JSON-RPC guarantees — checking for the `error` member — read a
typo'd method name as success and only noticed when it indexed the object it
got back. `rap/methods` remains the discovery call, and the -32601 response
carries a `data.hint` pointing at it.

A malformed frame is now **answered** rather than closing the connection. The
parse error used to propagate out of `handle_connection`, so a client that sent
one bad line lost every later request on that socket with no explanation.

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

## 7.6 Editor integration

> **Decided 2026-09-02: RAP is an agent protocol, and this repository will not
> pretend it is an editor one.** The question left open here was whether to
> write an LSP shim over RAP or to accept the mismatch. Accepted, on the
> evidence in §7.1: the design goals describe an agent protocol, and the method
> list is agent-shaped — `build/heal`, `nl/generate`, `abl/encode`,
> `abl/run`. An LSP server is a different thing with a different lifecycle:
> `initialize`, `textDocument` synchronisation, incremental sync, push diagnostics,
> position encodings. Writing one is a project, and calling RAP one was the
> defect.
>
> So the editor configurations now do what they can do, and say what they
> cannot. Tree-sitter highlighting works in all four. Formatting works as of
> today, because `mage-parse --fmt-compact -` reads stdin and every editor's
> format hook wants exactly that — no protocol required (item 25). What is
> gone is the language-server registration, in Neovim and Helix, along with
> eight invented settings for capabilities RAP has no methods for.
>
> If someone does want an LSP later, the honest shape is a separate binary that
> speaks LSP on stdio and calls RAP over TCP — a translator, not a rename. It
> would need the five `query/*` methods §7.4 lists as never-implemented.


**Corrected 2026-08-25.** This section described a `MAGE-vscode` extension
speaking to the RAP server, with a diagram routing hover through `query/type`
and completions through `query/completions`. **The extension does not exist** —
there is no `MAGE-vscode/` directory and no VS Code file is tracked anywhere in
the repository — and neither of those two methods exists either (§7.4).

What ships is in `editors/`:

| Editor | What it is | Talks to RAP? |
|---|---|---|
| Neovim | `lua/mage.lua`: filetype detection, tree-sitter, and an LSP registration | **No — see below** |
| Helix | `languages.toml`: language config and highlight queries | No |
| Zed | `extension.json` and highlights | No |
| tree-sitter | `tree-sitter-mage/`: the grammar the above share | n/a |

**The Neovim LSP registration cannot work, for three independent reasons.** It
registers RAP as a custom `lspconfig` server with `cmd = { 'rap' }`, and:

1. **RAP is not LSP.** `rap.rs` contains **zero** occurrences of `initialize`
   or `textDocument` — it speaks `language/parse`, not the LSP handshake, so a
   client would fail before sending anything useful.
2. **There is no `rap` binary.** The server is `mage-parse --rap`; no
   `[[bin]]` is named `rap`.
3. **RAP is TCP, not stdio.** It binds a `TcpListener` (§7.2); `lspconfig`'s
   `cmd` spawns a process and speaks over stdin/stdout.

The settings block it registers — `completion.autoimport`, `inlayHints.typeHints`,
`diagnostics.skb` — names capabilities RAP has no methods for. Syntax
highlighting via tree-sitter works in all four editors; nothing else does.

Making it work means either an LSP shim translating `textDocument/*` to RAP
methods, or accepting that RAP is an agent protocol rather than an editor one —
which is what §7.1's design goals actually describe. That is a decision, and
nobody has made it.

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
    resp = json.loads(s.makefile().readline())
    # A protocol failure has no `result` member at all. Reaching straight
    # for ["result"] raises KeyError with nothing to say why; the server
    # put the reason, and a discovery hint, in `error`.
    if "error" in resp:
        e = resp["error"]
        raise RuntimeError(f'{e["code"]}: {e["message"]} {e.get("data", "")}')
    return resp["result"]

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
