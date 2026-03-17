/// SKB Query Engine — structured knowledge base queries (P14).
///
/// The SKB stores per-module metadata: specs, effects, capabilities,
/// costs, aliases, deprecation info, and semantic dependencies.
/// Agents query the SKB at compile time and via RAP to make decisions.
///
/// SKB-QL syntax (simplified):
///   SELECT effects FROM std.io.read_file
///   SELECT cost WHERE construct = "Vec::push" AND target = "x86_64"
///   SELECT modules WHERE capability = "network"
///   SELECT spec FROM my_module.my_fn
use serde::{Deserialize, Serialize};

// ── SKB Data Model ───────────────────────────────────────────────────

/// A single SKB entry for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkbEntry {
    /// Fully qualified name: e.g. "std.io.read_file".
    pub fqn: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// Effects declared by this symbol.
    pub effects: Vec<String>,
    /// Required capabilities to invoke.
    pub capabilities: Vec<String>,
    /// Spec block (pre/post conditions), if any.
    pub spec: Option<SpecBlock>,
    /// Whether the symbol is deprecated.
    pub deprecated: Option<String>,
    /// Aliases from the Rust ecosystem.
    pub rust_aliases: Vec<String>,
    /// Tags for semantic search.
    pub tags: Vec<String>,
}

/// Spec block attached to a function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecBlock {
    pub requires: Vec<String>,
    pub ensures: Vec<String>,
}

/// Symbol kind in the knowledge base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Constant,
    TypeAlias,
}

// ── Built-in SKB ─────────────────────────────────────────────────────

fn builtin_skb() -> Vec<SkbEntry> {
    vec![
        // I/O
        entry("std.io.read_file", SymbolKind::Function,
            &["io", "fs"], &["fs.read"],
            Some(spec(&["path.exists()"], &["ret.is_ok() => ret.unwrap().len() >= 0"])),
            &["fs::read_to_string"], &["io", "file", "read"]),
        entry("std.io.write_file", SymbolKind::Function,
            &["io", "fs"], &["fs.write"],
            Some(spec(&["path.parent().exists()"], &["path.exists()"])),
            &["fs::write"], &["io", "file", "write"]),
        entry("std.io.stdin", SymbolKind::Function,
            &["io"], &[],
            None,
            &["io::stdin"], &["io", "input"]),
        entry("std.io.stdout", SymbolKind::Function,
            &["io"], &[],
            None,
            &["io::stdout"], &["io", "output"]),
        // Net
        entry("std.net.TcpStream", SymbolKind::Struct,
            &["io", "net"], &["network"],
            None,
            &["net::TcpStream"], &["network", "tcp", "stream"]),
        entry("std.net.listen", SymbolKind::Function,
            &["io", "net"], &["network"],
            Some(spec(&["port > 0", "port < 65536"], &["ret.is_ok() => listener.is_bound()"])),
            &["TcpListener::bind"], &["network", "tcp", "listen", "server"]),
        // Collections
        entry("std.collections.Vec", SymbolKind::Struct,
            &[], &[],
            None,
            &["Vec"], &["collection", "array", "vector"]),
        entry("std.collections.HashMap", SymbolKind::Struct,
            &[], &[],
            None,
            &["HashMap"], &["collection", "map", "hash"]),
        // Agent primitives
        entry("std.agent.Agent", SymbolKind::Trait,
            &[], &[],
            None,
            &[], &["agent", "trait", "handle"]),
        entry("std.agent.Swarm", SymbolKind::Struct,
            &["concurrency"], &[],
            None,
            &[], &["agent", "swarm", "multi-agent"]),
        entry("std.agent.Bus", SymbolKind::Struct,
            &["concurrency"], &[],
            None,
            &[], &["agent", "bus", "pubsub", "messaging"]),
        entry("std.agent.Memory", SymbolKind::Struct,
            &["io"], &[],
            None,
            &[], &["agent", "memory", "persist", "state"]),
        entry("std.agent.Lease", SymbolKind::Struct,
            &[], &[],
            None,
            &[], &["agent", "capability", "lease", "rbac"]),
        // Sync
        entry("std.sync.Mutex", SymbolKind::Struct,
            &["concurrency"], &[],
            None,
            &["Mutex"], &["sync", "mutex", "lock"]),
        entry("std.sync.RwLock", SymbolKind::Struct,
            &["concurrency"], &[],
            None,
            &["RwLock"], &["sync", "rwlock", "read-write"]),
        entry("std.sync.Channel", SymbolKind::Struct,
            &["concurrency"], &[],
            None,
            &["mpsc::channel"], &["sync", "channel", "mpsc"]),
    ]
}

fn entry(fqn: &str, kind: SymbolKind, effects: &[&str], caps: &[&str], spec: Option<SpecBlock>, aliases: &[&str], tags: &[&str]) -> SkbEntry {
    SkbEntry {
        fqn: fqn.into(),
        kind,
        effects: effects.iter().map(|s| s.to_string()).collect(),
        capabilities: caps.iter().map(|s| s.to_string()).collect(),
        spec,
        deprecated: None,
        rust_aliases: aliases.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

fn spec(requires: &[&str], ensures: &[&str]) -> SpecBlock {
    SpecBlock {
        requires: requires.iter().map(|s| s.to_string()).collect(),
        ensures: ensures.iter().map(|s| s.to_string()).collect(),
    }
}

// ── Query Interface ──────────────────────────────────────────────────

/// Result of an SKB query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub matches: Vec<SkbEntry>,
    pub query_text: String,
}

/// Query the SKB by fully qualified name (exact or prefix match).
pub fn query_by_fqn(fqn: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.fqn == fqn || e.fqn.starts_with(&format!("{fqn}.")))
        .collect();
    QueryResult { matches, query_text: format!("fqn = {fqn}") }
}

/// Query the SKB by effect name: find all symbols that declare a given effect.
pub fn query_by_effect(effect: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.effects.iter().any(|eff| eff == effect))
        .collect();
    QueryResult { matches, query_text: format!("effect = {effect}") }
}

/// Query by required capability: find symbols that require a given capability.
pub fn query_by_capability(cap: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.capabilities.iter().any(|c| c == cap))
        .collect();
    QueryResult { matches, query_text: format!("capability = {cap}") }
}

/// Query by tag (semantic search).
pub fn query_by_tag(tag: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.tags.iter().any(|t| t == tag))
        .collect();
    QueryResult { matches, query_text: format!("tag = {tag}") }
}

/// Query by Rust alias: find the Redox equivalent of a Rust symbol.
pub fn query_by_rust_alias(alias: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.rust_aliases.iter().any(|a| a.contains(alias)))
        .collect();
    QueryResult { matches, query_text: format!("rust_alias contains {alias}") }
}

/// Lookup the spec block for a symbol.
pub fn query_spec(fqn: &str) -> Option<SpecBlock> {
    builtin_skb().into_iter().find(|e| e.fqn == fqn).and_then(|e| e.spec)
}

/// List all symbols in a module (prefix match).
pub fn query_module(module_prefix: &str) -> QueryResult {
    let matches: Vec<_> = builtin_skb()
        .into_iter()
        .filter(|e| e.fqn.starts_with(module_prefix))
        .collect();
    QueryResult { matches, query_text: format!("module = {module_prefix}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_exact_fqn() {
        let result = query_by_fqn("std.io.read_file");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].fqn, "std.io.read_file");
    }

    #[test]
    fn query_module_prefix() {
        let result = query_module("std.io");
        assert!(result.matches.len() >= 2);
    }

    #[test]
    fn query_io_effect() {
        let result = query_by_effect("io");
        assert!(result.matches.len() >= 3);
    }

    #[test]
    fn query_network_capability() {
        let result = query_by_capability("network");
        assert!(result.matches.len() >= 1);
    }

    #[test]
    fn query_rust_alias() {
        let result = query_by_rust_alias("HashMap");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].fqn, "std.collections.HashMap");
    }

    #[test]
    fn query_spec_read_file() {
        let spec = query_spec("std.io.read_file").unwrap();
        assert!(!spec.requires.is_empty());
        assert!(!spec.ensures.is_empty());
    }

    #[test]
    fn query_agent_primitives() {
        let result = query_by_tag("agent");
        assert!(result.matches.len() >= 4);
    }

    #[test]
    fn query_nonexistent() {
        let result = query_by_fqn("nonexistent.module");
        assert!(result.matches.is_empty());
    }
}
