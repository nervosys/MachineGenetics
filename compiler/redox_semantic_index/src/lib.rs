//! # Redox Semantic Index
//!
//! A persistent semantic knowledge graph that indexes all symbols, types,
//! traits, impls, and their relationships across crates.  Merges the
//! capabilities of `rustdoc-json-types`, `rust-analyzer`'s `Analysis`, and
//! `rustc_public` into a unified, agent-queryable index.
//!
//! Reference: REDOX_PROPOSAL.md §Phase 1.4 — Semantic Code Index

use std::collections::HashMap;
use std::fmt;

// ===========================================================================
// Identifiers
// ===========================================================================

/// Crate-local unique ID for an indexed item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemId(pub u64);

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "item#{}", self.0)
    }
}

/// Crate identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrateId {
    pub name: String,
    pub version: String,
}

impl fmt::Display for CrateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.name, self.version)
    }
}

// ===========================================================================
// Items — the nodes of the knowledge graph
// ===========================================================================

/// Visibility of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Public,
    Crate,
    Module,
    Private,
}

/// The kind of indexed item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Function,
    Method,
    Struct,
    Enum,
    Union,
    Trait,
    Impl,
    TypeAlias,
    Constant,
    Static,
    Module,
    Macro,
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Union => "union",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::TypeAlias => "type alias",
            Self::Constant => "constant",
            Self::Static => "static",
            Self::Module => "module",
            Self::Macro => "macro",
        };
        write!(f, "{s}")
    }
}

/// A capability / effect that a function may require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    Io,
    Alloc,
    Panic,
    Unsafe,
    Async,
    Ffi,
}

impl Capability {
    pub fn name(self) -> &'static str {
        match self {
            Self::Io => "io",
            Self::Alloc => "alloc",
            Self::Panic => "panic",
            Self::Unsafe => "unsafe",
            Self::Async => "async",
            Self::Ffi => "ffi",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A type signature component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSig {
    /// Rendered type string (e.g. `fn(u32) -> bool`).
    pub rendered: String,
    /// Generic parameters, if any (e.g. `["T: Clone", "'a"]`).
    pub generics: Vec<String>,
    /// Where-clause bounds (e.g. `["T: Send + 'a"]`).
    pub where_bounds: Vec<String>,
}

/// Source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLoc {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.column)
    }
}

/// An indexed item — a node in the knowledge graph.
#[derive(Debug, Clone)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    /// Fully qualified path (e.g. `std::collections::HashMap::insert`).
    pub path: String,
    pub kind: ItemKind,
    pub crate_id: CrateId,
    pub visibility: Visibility,
    pub type_sig: Option<TypeSig>,
    pub capabilities: Vec<Capability>,
    pub doc: Option<String>,
    pub source: Option<SourceLoc>,
}

// ===========================================================================
// Edges — the relationships between items
// ===========================================================================

/// Relationship kinds in the cross-reference graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// Function A calls function B.
    Calls,
    /// Type A implements trait B.
    Implements,
    /// Item A is a child of module/type B.
    ChildOf,
    /// Crate A depends on crate B.
    DependsOn,
    /// Type A contains field of type B.
    ContainsType,
    /// Function A returns type B.
    Returns,
    /// Function A takes parameter of type B.
    TakesParam,
    /// Impl block A is for type B.
    ImplFor,
    /// Re-export: A re-exports B.
    ReExports,
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Calls => "calls",
            Self::Implements => "implements",
            Self::ChildOf => "child_of",
            Self::DependsOn => "depends_on",
            Self::ContainsType => "contains_type",
            Self::Returns => "returns",
            Self::TakesParam => "takes_param",
            Self::ImplFor => "impl_for",
            Self::ReExports => "re_exports",
        };
        write!(f, "{s}")
    }
}

/// A directional edge in the knowledge graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: ItemId,
    pub to: ItemId,
    pub kind: EdgeKind,
}

// ===========================================================================
// SemanticIndex — the knowledge graph
// ===========================================================================

/// Query results returned by the index.
#[derive(Debug, Clone)]
pub struct QueryResult<'a> {
    pub items: Vec<&'a Item>,
}

/// Statistics about the index contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStats {
    pub total_items: usize,
    pub total_edges: usize,
    pub items_by_kind: HashMap<String, usize>,
    pub items_by_crate: HashMap<String, usize>,
    pub capabilities_count: HashMap<String, usize>,
}

/// The semantic knowledge graph.
///
/// In-memory backend. Stores items and edges with multiple indices for
/// fast lookup by name, path, kind, crate, and capability.
#[derive(Debug)]
pub struct SemanticIndex {
    /// All items, keyed by ItemId.
    items: HashMap<ItemId, Item>,
    /// Edges (from → list of edges).
    edges_from: HashMap<ItemId, Vec<Edge>>,
    /// Reverse edges (to → list of edges).
    edges_to: HashMap<ItemId, Vec<Edge>>,

    // --- Secondary indices ---
    /// Name → ItemIds (handles name collisions across crates).
    name_index: HashMap<String, Vec<ItemId>>,
    /// Fully-qualified path → ItemId (unique within a version).
    path_index: HashMap<String, ItemId>,
    /// Kind → ItemIds.
    kind_index: HashMap<ItemKind, Vec<ItemId>>,
    /// CrateId → ItemIds.
    crate_index: HashMap<CrateId, Vec<ItemId>>,
    /// Capability → ItemIds (items that have this capability).
    capability_index: HashMap<Capability, Vec<ItemId>>,

    /// Next auto-incrementing ItemId.
    next_id: u64,
}

impl SemanticIndex {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            edges_from: HashMap::new(),
            edges_to: HashMap::new(),
            name_index: HashMap::new(),
            path_index: HashMap::new(),
            kind_index: HashMap::new(),
            crate_index: HashMap::new(),
            capability_index: HashMap::new(),
            next_id: 1,
        }
    }

    /// Allocate the next unique [`ItemId`].
    pub fn next_item_id(&mut self) -> ItemId {
        let id = ItemId(self.next_id);
        self.next_id += 1;
        id
    }

    // ------------------------------------------------------------------
    // Mutation
    // ------------------------------------------------------------------

    /// Insert an item into the index. Updates all secondary indices.
    pub fn add_item(&mut self, item: Item) {
        let id = item.id;

        // Name index.
        self.name_index
            .entry(item.name.clone())
            .or_default()
            .push(id);

        // Path index.
        self.path_index.insert(item.path.clone(), id);

        // Kind index.
        self.kind_index.entry(item.kind.clone()).or_default().push(id);

        // Crate index.
        self.crate_index.entry(item.crate_id.clone()).or_default().push(id);

        // Capability index.
        for cap in &item.capabilities {
            self.capability_index.entry(*cap).or_default().push(id);
        }

        self.items.insert(id, item);
    }

    /// Add a directional edge between two items.
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges_to
            .entry(edge.to)
            .or_default()
            .push(edge.clone());
        self.edges_from.entry(edge.from).or_default().push(edge);
    }

    /// Convenience: insert an item and return its assigned ID.
    pub fn insert(&mut self, mut builder: Item) -> ItemId {
        if builder.id == ItemId(0) {
            builder.id = self.next_item_id();
        }
        let id = builder.id;
        self.add_item(builder);
        id
    }

    // ------------------------------------------------------------------
    // Lookup — single item
    // ------------------------------------------------------------------

    /// Get an item by its ID.
    pub fn get(&self, id: ItemId) -> Option<&Item> {
        self.items.get(&id)
    }

    /// Get an item by its fully-qualified path.
    pub fn get_by_path(&self, path: &str) -> Option<&Item> {
        self.path_index
            .get(path)
            .and_then(|id| self.items.get(id))
    }

    // ------------------------------------------------------------------
    // Query — multiple items
    // ------------------------------------------------------------------

    /// Find all items with a given name.
    pub fn query_by_name(&self, name: &str) -> Vec<&Item> {
        self.name_index
            .get(name)
            .map(|ids| ids.iter().filter_map(|id| self.items.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find all items of a given kind.
    pub fn query_by_kind(&self, kind: &ItemKind) -> Vec<&Item> {
        self.kind_index
            .get(kind)
            .map(|ids| ids.iter().filter_map(|id| self.items.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find all items in a given crate.
    pub fn query_by_crate(&self, crate_id: &CrateId) -> Vec<&Item> {
        self.crate_index
            .get(crate_id)
            .map(|ids| ids.iter().filter_map(|id| self.items.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find all items that have a given capability.
    pub fn query_by_capability(&self, cap: Capability) -> Vec<&Item> {
        self.capability_index
            .get(&cap)
            .map(|ids| ids.iter().filter_map(|id| self.items.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find all public items in a crate.
    pub fn public_api(&self, crate_id: &CrateId) -> Vec<&Item> {
        self.query_by_crate(crate_id)
            .into_iter()
            .filter(|item| item.visibility == Visibility::Public)
            .collect()
    }

    // ------------------------------------------------------------------
    // Graph traversal
    // ------------------------------------------------------------------

    /// Get all outgoing edges from an item.
    pub fn edges_from(&self, id: ItemId) -> &[Edge] {
        self.edges_from.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all incoming edges to an item.
    pub fn edges_to(&self, id: ItemId) -> &[Edge] {
        self.edges_to.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get callers of a function.
    pub fn callers(&self, id: ItemId) -> Vec<&Item> {
        self.edges_to(id)
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .filter_map(|e| self.items.get(&e.from))
            .collect()
    }

    /// Get callees of a function.
    pub fn callees(&self, id: ItemId) -> Vec<&Item> {
        self.edges_from(id)
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .filter_map(|e| self.items.get(&e.to))
            .collect()
    }

    /// Get all types that implement a trait.
    pub fn implementors(&self, trait_id: ItemId) -> Vec<&Item> {
        self.edges_to(trait_id)
            .iter()
            .filter(|e| e.kind == EdgeKind::Implements)
            .filter_map(|e| self.items.get(&e.from))
            .collect()
    }

    /// Get all traits implemented by a type.
    pub fn traits_of(&self, type_id: ItemId) -> Vec<&Item> {
        self.edges_from(type_id)
            .iter()
            .filter(|e| e.kind == EdgeKind::Implements)
            .filter_map(|e| self.items.get(&e.to))
            .collect()
    }

    /// Get the children of a module or type.
    pub fn children(&self, parent_id: ItemId) -> Vec<&Item> {
        self.edges_to(parent_id)
            .iter()
            .filter(|e| e.kind == EdgeKind::ChildOf)
            .filter_map(|e| self.items.get(&e.from))
            .collect()
    }

    /// Get direct dependencies of a crate item.
    pub fn dependencies(&self, crate_module_id: ItemId) -> Vec<&Item> {
        self.edges_from(crate_module_id)
            .iter()
            .filter(|e| e.kind == EdgeKind::DependsOn)
            .filter_map(|e| self.items.get(&e.to))
            .collect()
    }

    // ------------------------------------------------------------------
    // Statistics
    // ------------------------------------------------------------------

    pub fn total_items(&self) -> usize {
        self.items.len()
    }

    pub fn total_edges(&self) -> usize {
        self.edges_from.values().map(|v| v.len()).sum()
    }

    pub fn statistics(&self) -> IndexStats {
        let mut items_by_kind: HashMap<String, usize> = HashMap::new();
        let mut items_by_crate: HashMap<String, usize> = HashMap::new();
        let mut capabilities_count: HashMap<String, usize> = HashMap::new();

        for item in self.items.values() {
            *items_by_kind
                .entry(format!("{}", item.kind))
                .or_default() += 1;
            *items_by_crate
                .entry(item.crate_id.name.clone())
                .or_default() += 1;
            for cap in &item.capabilities {
                *capabilities_count.entry(cap.name().to_string()).or_default() += 1;
            }
        }

        IndexStats {
            total_items: self.items.len(),
            total_edges: self.total_edges(),
            items_by_kind,
            items_by_crate,
            capabilities_count,
        }
    }
}

impl Default for SemanticIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Builder helpers for ergonomic item construction
// ===========================================================================

/// Fluent builder for creating [`Item`]s.
pub struct ItemBuilder {
    item: Item,
}

impl ItemBuilder {
    pub fn new(name: impl Into<String>, kind: ItemKind, crate_id: CrateId) -> Self {
        let name = name.into();
        let path = name.clone();
        Self {
            item: Item {
                id: ItemId(0), // assigned on insert
                name,
                path,
                kind,
                crate_id,
                visibility: Visibility::Public,
                type_sig: None,
                capabilities: Vec::new(),
                doc: None,
                source: None,
            },
        }
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.item.path = path.into();
        self
    }

    pub fn visibility(mut self, vis: Visibility) -> Self {
        self.item.visibility = vis;
        self
    }

    pub fn type_sig(mut self, sig: TypeSig) -> Self {
        self.item.type_sig = Some(sig);
        self
    }

    pub fn capability(mut self, cap: Capability) -> Self {
        self.item.capabilities.push(cap);
        self
    }

    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.item.doc = Some(doc.into());
        self
    }

    pub fn source(mut self, loc: SourceLoc) -> Self {
        self.item.source = Some(loc);
        self
    }

    pub fn build(self) -> Item {
        self.item
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_crate() -> CrateId {
        CrateId { name: "mylib".into(), version: "1.0.0".into() }
    }

    fn make_fn(idx: &mut SemanticIndex, name: &str, path: &str) -> ItemId {
        let item = ItemBuilder::new(name, ItemKind::Function, test_crate())
            .path(path)
            .build();
        idx.insert(item)
    }

    fn make_struct(idx: &mut SemanticIndex, name: &str, path: &str) -> ItemId {
        let item = ItemBuilder::new(name, ItemKind::Struct, test_crate())
            .path(path)
            .build();
        idx.insert(item)
    }

    fn make_trait(idx: &mut SemanticIndex, name: &str, path: &str) -> ItemId {
        let item = ItemBuilder::new(name, ItemKind::Trait, test_crate())
            .path(path)
            .build();
        idx.insert(item)
    }

    // -- Basic insertion and lookup ----------------------------------------

    #[test]
    fn insert_and_get() {
        let mut idx = SemanticIndex::new();
        let id = make_fn(&mut idx, "foo", "mylib::foo");
        let item = idx.get(id).unwrap();
        assert_eq!(item.name, "foo");
        assert_eq!(item.path, "mylib::foo");
    }

    #[test]
    fn get_by_path() {
        let mut idx = SemanticIndex::new();
        make_fn(&mut idx, "bar", "mylib::bar");
        let item = idx.get_by_path("mylib::bar").unwrap();
        assert_eq!(item.name, "bar");
    }

    #[test]
    fn missing_item_returns_none() {
        let idx = SemanticIndex::new();
        assert!(idx.get(ItemId(999)).is_none());
        assert!(idx.get_by_path("nonexistent").is_none());
    }

    #[test]
    fn auto_incrementing_ids() {
        let mut idx = SemanticIndex::new();
        let id1 = make_fn(&mut idx, "a", "a");
        let id2 = make_fn(&mut idx, "b", "b");
        assert_eq!(id1.0 + 1, id2.0);
    }

    // -- Name index --------------------------------------------------------

    #[test]
    fn query_by_name() {
        let mut idx = SemanticIndex::new();
        make_fn(&mut idx, "process", "mylib::process");
        let other_crate = CrateId { name: "other".into(), version: "2.0.0".into() };
        let item2 = ItemBuilder::new("process", ItemKind::Function, other_crate)
            .path("other::process")
            .build();
        idx.insert(item2);

        let results = idx.query_by_name("process");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_by_name_empty() {
        let idx = SemanticIndex::new();
        assert!(idx.query_by_name("nonexistent").is_empty());
    }

    // -- Kind index --------------------------------------------------------

    #[test]
    fn query_by_kind() {
        let mut idx = SemanticIndex::new();
        make_fn(&mut idx, "f1", "f1");
        make_fn(&mut idx, "f2", "f2");
        make_struct(&mut idx, "S1", "S1");

        let fns = idx.query_by_kind(&ItemKind::Function);
        assert_eq!(fns.len(), 2);
        let structs = idx.query_by_kind(&ItemKind::Struct);
        assert_eq!(structs.len(), 1);
    }

    // -- Crate index -------------------------------------------------------

    #[test]
    fn query_by_crate() {
        let mut idx = SemanticIndex::new();
        make_fn(&mut idx, "f", "mylib::f");
        make_struct(&mut idx, "S", "mylib::S");
        let results = idx.query_by_crate(&test_crate());
        assert_eq!(results.len(), 2);
    }

    // -- Capability index --------------------------------------------------

    #[test]
    fn query_by_capability() {
        let mut idx = SemanticIndex::new();
        let item = ItemBuilder::new("read_file", ItemKind::Function, test_crate())
            .path("mylib::read_file")
            .capability(Capability::Io)
            .capability(Capability::Panic)
            .build();
        idx.insert(item);

        make_fn(&mut idx, "pure", "mylib::pure");

        let io_items = idx.query_by_capability(Capability::Io);
        assert_eq!(io_items.len(), 1);
        assert_eq!(io_items[0].name, "read_file");

        let panic_items = idx.query_by_capability(Capability::Panic);
        assert_eq!(panic_items.len(), 1);

        let alloc_items = idx.query_by_capability(Capability::Alloc);
        assert!(alloc_items.is_empty());
    }

    // -- Public API --------------------------------------------------------

    #[test]
    fn public_api_filters_visibility() {
        let mut idx = SemanticIndex::new();
        let pub_item = ItemBuilder::new("pub_fn", ItemKind::Function, test_crate())
            .path("mylib::pub_fn")
            .visibility(Visibility::Public)
            .build();
        idx.insert(pub_item);

        let priv_item = ItemBuilder::new("priv_fn", ItemKind::Function, test_crate())
            .path("mylib::priv_fn")
            .visibility(Visibility::Private)
            .build();
        idx.insert(priv_item);

        let api = idx.public_api(&test_crate());
        assert_eq!(api.len(), 1);
        assert_eq!(api[0].name, "pub_fn");
    }

    // -- Edges: calls ------------------------------------------------------

    #[test]
    fn callers_and_callees() {
        let mut idx = SemanticIndex::new();
        let a = make_fn(&mut idx, "a", "a");
        let b = make_fn(&mut idx, "b", "b");
        let c = make_fn(&mut idx, "c", "c");

        // a calls b, a calls c
        idx.add_edge(Edge { from: a, to: b, kind: EdgeKind::Calls });
        idx.add_edge(Edge { from: a, to: c, kind: EdgeKind::Calls });

        let callees = idx.callees(a);
        assert_eq!(callees.len(), 2);

        let callers_b = idx.callers(b);
        assert_eq!(callers_b.len(), 1);
        assert_eq!(callers_b[0].name, "a");
    }

    // -- Edges: implements -------------------------------------------------

    #[test]
    fn implementors_and_traits_of() {
        let mut idx = SemanticIndex::new();
        let trait_id = make_trait(&mut idx, "Display", "std::fmt::Display");
        let struct_id = make_struct(&mut idx, "Point", "mylib::Point");

        idx.add_edge(Edge {
            from: struct_id,
            to: trait_id,
            kind: EdgeKind::Implements,
        });

        let impls = idx.implementors(trait_id);
        assert_eq!(impls.len(), 1);
        assert_eq!(impls[0].name, "Point");

        let traits = idx.traits_of(struct_id);
        assert_eq!(traits.len(), 1);
        assert_eq!(traits[0].name, "Display");
    }

    // -- Edges: child_of ---------------------------------------------------

    #[test]
    fn children_of_module() {
        let mut idx = SemanticIndex::new();
        let mod_id = {
            let item = ItemBuilder::new("utils", ItemKind::Module, test_crate())
                .path("mylib::utils")
                .build();
            idx.insert(item)
        };
        let fn_id = make_fn(&mut idx, "helper", "mylib::utils::helper");

        idx.add_edge(Edge { from: fn_id, to: mod_id, kind: EdgeKind::ChildOf });

        let kids = idx.children(mod_id);
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].name, "helper");
    }

    // -- Edges: depends_on -------------------------------------------------

    #[test]
    fn crate_dependencies() {
        let mut idx = SemanticIndex::new();
        let crate_a = {
            let item = ItemBuilder::new("crate_a", ItemKind::Module, test_crate())
                .path("crate_a")
                .build();
            idx.insert(item)
        };
        let dep_crate = CrateId { name: "serde".into(), version: "1.0.0".into() };
        let crate_b = {
            let item = ItemBuilder::new("serde", ItemKind::Module, dep_crate)
                .path("serde")
                .build();
            idx.insert(item)
        };

        idx.add_edge(Edge { from: crate_a, to: crate_b, kind: EdgeKind::DependsOn });

        let deps = idx.dependencies(crate_a);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "serde");
    }

    // -- Edge counts -------------------------------------------------------

    #[test]
    fn edges_from_and_to() {
        let mut idx = SemanticIndex::new();
        let a = make_fn(&mut idx, "a", "a");
        let b = make_fn(&mut idx, "b", "b");

        idx.add_edge(Edge { from: a, to: b, kind: EdgeKind::Calls });

        assert_eq!(idx.edges_from(a).len(), 1);
        assert_eq!(idx.edges_to(b).len(), 1);
        assert!(idx.edges_from(b).is_empty());
        assert!(idx.edges_to(a).is_empty());
    }

    // -- Statistics --------------------------------------------------------

    #[test]
    fn empty_statistics() {
        let idx = SemanticIndex::new();
        let stats = idx.statistics();
        assert_eq!(stats.total_items, 0);
        assert_eq!(stats.total_edges, 0);
    }

    #[test]
    fn statistics_counts() {
        let mut idx = SemanticIndex::new();
        let a = make_fn(&mut idx, "a", "a");
        let b = make_fn(&mut idx, "b", "b");
        make_struct(&mut idx, "S", "S");

        idx.add_edge(Edge { from: a, to: b, kind: EdgeKind::Calls });

        let stats = idx.statistics();
        assert_eq!(stats.total_items, 3);
        assert_eq!(stats.total_edges, 1);
        assert_eq!(stats.items_by_kind["function"], 2);
        assert_eq!(stats.items_by_kind["struct"], 1);
        assert_eq!(stats.items_by_crate["mylib"], 3);
    }

    #[test]
    fn statistics_capabilities() {
        let mut idx = SemanticIndex::new();
        let item = ItemBuilder::new("f", ItemKind::Function, test_crate())
            .path("f")
            .capability(Capability::Io)
            .capability(Capability::Alloc)
            .build();
        idx.insert(item);

        let stats = idx.statistics();
        assert_eq!(stats.capabilities_count["io"], 1);
        assert_eq!(stats.capabilities_count["alloc"], 1);
    }

    // -- Builder -----------------------------------------------------------

    #[test]
    fn item_builder_full() {
        let item = ItemBuilder::new("process", ItemKind::Function, test_crate())
            .path("mylib::process")
            .visibility(Visibility::Crate)
            .type_sig(TypeSig {
                rendered: "fn(u32) -> bool".into(),
                generics: vec![],
                where_bounds: vec![],
            })
            .capability(Capability::Io)
            .doc("Process an item.")
            .source(SourceLoc {
                file: "src/lib.rs".into(),
                line: 42,
                column: 1,
            })
            .build();

        assert_eq!(item.name, "process");
        assert_eq!(item.visibility, Visibility::Crate);
        assert!(item.type_sig.is_some());
        assert_eq!(item.capabilities.len(), 1);
        assert_eq!(item.doc.as_deref(), Some("Process an item."));
        assert_eq!(item.source.as_ref().unwrap().line, 42);
    }

    // -- Display impls -----------------------------------------------------

    #[test]
    fn item_id_display() {
        assert_eq!(format!("{}", ItemId(42)), "item#42");
    }

    #[test]
    fn crate_id_display() {
        let c = CrateId { name: "foo".into(), version: "1.2.3".into() };
        assert_eq!(format!("{c}"), "foo-1.2.3");
    }

    #[test]
    fn edge_kind_display() {
        assert_eq!(format!("{}", EdgeKind::Calls), "calls");
        assert_eq!(format!("{}", EdgeKind::Implements), "implements");
        assert_eq!(format!("{}", EdgeKind::ChildOf), "child_of");
    }

    #[test]
    fn capability_display() {
        assert_eq!(format!("{}", Capability::Io), "io");
        assert_eq!(format!("{}", Capability::Async), "async");
    }

    #[test]
    fn source_loc_display() {
        let loc = SourceLoc { file: "main.rs".into(), line: 10, column: 5 };
        assert_eq!(format!("{loc}"), "main.rs:10:5");
    }

    // -- Default impl ------------------------------------------------------

    #[test]
    fn default_creates_empty_index() {
        let idx = SemanticIndex::default();
        assert_eq!(idx.total_items(), 0);
        assert_eq!(idx.total_edges(), 0);
    }

    // -- Integration: multi-crate graph ------------------------------------

    #[test]
    fn multi_crate_knowledge_graph() {
        let mut idx = SemanticIndex::new();

        // Crate: std
        let std_crate = CrateId { name: "std".into(), version: "1.0.0".into() };
        let display_trait = {
            let item = ItemBuilder::new("Display", ItemKind::Trait, std_crate.clone())
                .path("std::fmt::Display")
                .doc("Format trait for user-facing output.")
                .build();
            idx.insert(item)
        };
        let clone_trait = {
            let item = ItemBuilder::new("Clone", ItemKind::Trait, std_crate.clone())
                .path("std::clone::Clone")
                .build();
            idx.insert(item)
        };

        // Crate: mylib
        let mylib = test_crate();
        let point_struct = {
            let item = ItemBuilder::new("Point", ItemKind::Struct, mylib.clone())
                .path("mylib::Point")
                .type_sig(TypeSig {
                    rendered: "struct Point { x: f64, y: f64 }".into(),
                    generics: vec![],
                    where_bounds: vec![],
                })
                .build();
            idx.insert(item)
        };
        let distance_fn = {
            let item = ItemBuilder::new("distance", ItemKind::Method, mylib.clone())
                .path("mylib::Point::distance")
                .type_sig(TypeSig {
                    rendered: "fn(&self, other: &Point) -> f64".into(),
                    generics: vec![],
                    where_bounds: vec![],
                })
                .build();
            idx.insert(item)
        };

        // Edges
        idx.add_edge(Edge { from: point_struct, to: display_trait, kind: EdgeKind::Implements });
        idx.add_edge(Edge { from: point_struct, to: clone_trait, kind: EdgeKind::Implements });
        idx.add_edge(Edge { from: distance_fn, to: point_struct, kind: EdgeKind::ChildOf });

        // Verify
        assert_eq!(idx.total_items(), 4);
        assert_eq!(idx.total_edges(), 3);

        let traits = idx.traits_of(point_struct);
        assert_eq!(traits.len(), 2);

        let impls = idx.implementors(display_trait);
        assert_eq!(impls.len(), 1);
        assert_eq!(impls[0].name, "Point");

        let children = idx.children(point_struct);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "distance");

        let std_items = idx.query_by_crate(&std_crate);
        assert_eq!(std_items.len(), 2);

        let stats = idx.statistics();
        assert_eq!(stats.items_by_crate["std"], 2);
        assert_eq!(stats.items_by_crate["mylib"], 2);
    }
}
