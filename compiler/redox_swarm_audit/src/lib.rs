//! # Redox Swarm Audit Log
//!
//! Append-only, cryptographically signed (SHA-256) operation history
//! with agent ID attribution for swarm coordination auditing.
//!
//! Core concepts:
//! - **AuditEntry**: A single auditable operation with agent attribution
//! - **AuditChain**: SHA-256 hash chain guaranteeing tamper-evidence
//! - **AuditLog**: The main log with append, query, and verify operations
//! - **AuditQuery**: Flexible query builder for filtering entries
//!
//! Security properties (§9.7.2 of REDOX_PROPOSAL.md):
//! - Every semantic op cryptographically signed
//! - Every sandbox execution logged with agent ID
//! - Append-only: entries cannot be modified or deleted
//! - Hash chain: each entry includes hash of previous entry
//! - Deterministic replay: full operation history preserved
//!
//! (ROADMAP Step 55)

use std::collections::BTreeMap;
use std::fmt;

// ── SHA-256 (minimal, no external deps) ─────────────────────────────────────

/// A SHA-256 hash digest (32 bytes).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Sha256Digest(pub [u8; 32]);

impl Sha256Digest {
    /// The zero digest, used as the "previous hash" for the genesis entry.
    pub fn zero() -> Self {
        Sha256Digest([0u8; 32])
    }

    /// Format as lowercase hex string.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Parse from a 64-character hex string.
    pub fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 64 {
            return None;
        }
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Sha256Digest(bytes))
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sha256({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Compute SHA-256 of arbitrary data.
///
/// This is a self-contained implementation — no external crate dependencies.
/// Based on FIPS 180-4.
pub fn sha256(data: &[u8]) -> Sha256Digest {
    // Initial hash values (first 32 bits of fractional parts of square roots of first 8 primes)
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    // Round constants (first 32 bits of fractional parts of cube roots of first 64 primes)
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    // Pre-processing: pad message
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit (64-byte) block
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut digest = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        digest[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    Sha256Digest(digest)
}

// ── Timestamps ──────────────────────────────────────────────────────────────

/// Monotonic logical timestamp for audit entries.
pub type AuditTimestamp = u64;

// ── Agent ID ────────────────────────────────────────────────────────────────

/// Agent identifier for attribution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new(id: &str) -> Self {
        AgentId(id.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Operation Categories ────────────────────────────────────────────────────

/// Category of auditable operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OpCategory {
    /// Semantic code operation (add, remove, rename, modify).
    SemanticOp,
    /// Lease acquisition or release.
    LeaseOp,
    /// Consensus vote (propose, accept, reject).
    ConsensusOp,
    /// Task assignment or completion.
    TaskOp,
    /// Sandbox execution of agent-generated code.
    SandboxExec,
    /// Branch / merge / snapshot VCS operation.
    VcsOp,
    /// Agent lifecycle (join, leave, health-check).
    AgentLifecycle,
    /// Security event (capability violation, sandbox termination).
    SecurityEvent,
    /// Custom category for extensibility.
    Custom(String),
}

impl fmt::Display for OpCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpCategory::SemanticOp => write!(f, "semantic_op"),
            OpCategory::LeaseOp => write!(f, "lease_op"),
            OpCategory::ConsensusOp => write!(f, "consensus_op"),
            OpCategory::TaskOp => write!(f, "task_op"),
            OpCategory::SandboxExec => write!(f, "sandbox_exec"),
            OpCategory::VcsOp => write!(f, "vcs_op"),
            OpCategory::AgentLifecycle => write!(f, "agent_lifecycle"),
            OpCategory::SecurityEvent => write!(f, "security_event"),
            OpCategory::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

// ── Severity ────────────────────────────────────────────────────────────────

/// Severity level for audit entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Informational — normal operations.
    Info,
    /// Warning — unusual but non-critical.
    Warning,
    /// Error — operation failed or was rejected.
    Error,
    /// Critical — security violation or data integrity issue.
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "INFO"),
            Severity::Warning => write!(f, "WARN"),
            Severity::Error => write!(f, "ERROR"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}

// ── Audit Entry ─────────────────────────────────────────────────────────────

/// Unique identifier for an audit entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId(pub u64);

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit-{}", self.0)
    }
}

/// A single entry in the audit log — immutable once appended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// Unique sequential identifier.
    pub id: EntryId,
    /// Logical timestamp.
    pub timestamp: AuditTimestamp,
    /// Agent that performed the operation.
    pub agent_id: AgentId,
    /// Category of the operation.
    pub category: OpCategory,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable description of the operation.
    pub description: String,
    /// Optional structured metadata (key-value pairs).
    pub metadata: BTreeMap<String, String>,
    /// SHA-256 hash of the previous entry (genesis uses zero hash).
    pub prev_hash: Sha256Digest,
    /// SHA-256 hash of this entry's content (includes prev_hash for chaining).
    pub hash: Sha256Digest,
}

impl AuditEntry {
    /// Compute the canonical byte representation for hashing.
    fn canonical_bytes(
        id: u64,
        timestamp: AuditTimestamp,
        agent_id: &str,
        category: &str,
        severity: &str,
        description: &str,
        metadata: &BTreeMap<String, String>,
        prev_hash: &Sha256Digest,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&timestamp.to_le_bytes());
        buf.extend_from_slice(agent_id.as_bytes());
        buf.push(0); // separator
        buf.extend_from_slice(category.as_bytes());
        buf.push(0);
        buf.extend_from_slice(severity.as_bytes());
        buf.push(0);
        buf.extend_from_slice(description.as_bytes());
        buf.push(0);
        // Metadata in deterministic order (BTreeMap is sorted)
        for (k, v) in metadata {
            buf.extend_from_slice(k.as_bytes());
            buf.push(b'=');
            buf.extend_from_slice(v.as_bytes());
            buf.push(0);
        }
        // Chain to previous entry
        buf.extend_from_slice(&prev_hash.0);
        buf
    }

    /// Verify that this entry's hash matches its content.
    pub fn verify_hash(&self) -> bool {
        let bytes = Self::canonical_bytes(
            self.id.0,
            self.timestamp,
            self.agent_id.as_str(),
            &self.category.to_string(),
            &self.severity.to_string(),
            &self.description,
            &self.metadata,
            &self.prev_hash,
        );
        sha256(&bytes) == self.hash
    }
}

impl fmt::Display for AuditEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} | {} | {} | {} | {}",
            self.id,
            self.timestamp,
            self.severity,
            self.agent_id,
            self.category,
            self.description,
        )
    }
}

// ── Audit Log ───────────────────────────────────────────────────────────────

/// Errors from audit log operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    /// Hash chain is broken at the given entry index.
    ChainBroken { index: usize, entry_id: EntryId },
    /// Entry hash does not match its content.
    HashMismatch { entry_id: EntryId },
    /// Log is empty (no entries to verify or query).
    EmptyLog,
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditError::ChainBroken { index, entry_id } => {
                write!(f, "hash chain broken at index {index} ({entry_id})")
            }
            AuditError::HashMismatch { entry_id } => {
                write!(f, "hash mismatch for entry {entry_id}")
            }
            AuditError::EmptyLog => write!(f, "audit log is empty"),
        }
    }
}

/// Append-only, SHA-256-chained audit log for swarm operations.
///
/// Guarantees:
/// - Entries are immutable once appended.
/// - Each entry's hash covers its content + the previous entry's hash.
/// - The chain can be verified end-to-end for tamper detection.
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    next_id: u64,
    next_timestamp: AuditTimestamp,
}

impl AuditLog {
    /// Create a new empty audit log.
    pub fn new() -> Self {
        AuditLog {
            entries: Vec::new(),
            next_id: 1,
            next_timestamp: 1,
        }
    }

    /// Append a new entry to the log. Returns the entry's ID.
    pub fn append(
        &mut self,
        agent_id: AgentId,
        category: OpCategory,
        severity: Severity,
        description: String,
        metadata: BTreeMap<String, String>,
    ) -> EntryId {
        let id = self.next_id;
        let timestamp = self.next_timestamp;
        let prev_hash = self
            .entries
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(Sha256Digest::zero);

        let cat_str = category.to_string();
        let sev_str = severity.to_string();
        let bytes = AuditEntry::canonical_bytes(
            id,
            timestamp,
            agent_id.as_str(),
            &cat_str,
            &sev_str,
            &description,
            &metadata,
            &prev_hash,
        );
        let hash = sha256(&bytes);

        let entry = AuditEntry {
            id: EntryId(id),
            timestamp,
            agent_id,
            category,
            severity,
            description,
            metadata,
            prev_hash,
            hash,
        };

        self.entries.push(entry);
        self.next_id += 1;
        self.next_timestamp += 1;
        EntryId(id)
    }

    /// Convenience: append an info-level entry with no metadata.
    pub fn append_info(
        &mut self,
        agent_id: AgentId,
        category: OpCategory,
        description: String,
    ) -> EntryId {
        self.append(agent_id, category, Severity::Info, description, BTreeMap::new())
    }

    /// Number of entries in the log.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get an entry by its sequential index (0-based).
    pub fn get(&self, index: usize) -> Option<&AuditEntry> {
        self.entries.get(index)
    }

    /// Get an entry by its EntryId.
    pub fn get_by_id(&self, id: &EntryId) -> Option<&AuditEntry> {
        self.entries.iter().find(|e| e.id == *id)
    }

    /// Get the last entry in the log.
    pub fn last(&self) -> Option<&AuditEntry> {
        self.entries.last()
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &AuditEntry> {
        self.entries.iter()
    }

    /// Get the current chain head hash.
    pub fn head_hash(&self) -> Sha256Digest {
        self.entries
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(Sha256Digest::zero)
    }

    // ── Verification ─────────────────────────────────────────────────────

    /// Verify the integrity of the entire hash chain.
    ///
    /// Checks:
    /// 1. Each entry's hash matches its content.
    /// 2. Each entry's prev_hash matches the previous entry's hash.
    /// 3. The first entry's prev_hash is the zero hash.
    pub fn verify_chain(&self) -> Result<(), AuditError> {
        if self.entries.is_empty() {
            return Ok(());
        }

        // Check genesis entry
        if self.entries[0].prev_hash != Sha256Digest::zero() {
            return Err(AuditError::ChainBroken {
                index: 0,
                entry_id: self.entries[0].id.clone(),
            });
        }

        for (i, entry) in self.entries.iter().enumerate() {
            // Verify entry's own hash
            if !entry.verify_hash() {
                return Err(AuditError::HashMismatch {
                    entry_id: entry.id.clone(),
                });
            }

            // Verify chain linkage (skip genesis)
            if i > 0 && entry.prev_hash != self.entries[i - 1].hash {
                return Err(AuditError::ChainBroken {
                    index: i,
                    entry_id: entry.id.clone(),
                });
            }
        }

        Ok(())
    }

    /// Verify a range of entries [start..end) for chain integrity.
    pub fn verify_range(&self, start: usize, end: usize) -> Result<(), AuditError> {
        let end = end.min(self.entries.len());
        if start >= end {
            return Ok(());
        }

        for i in start..end {
            let entry = &self.entries[i];
            if !entry.verify_hash() {
                return Err(AuditError::HashMismatch {
                    entry_id: entry.id.clone(),
                });
            }
            if i > 0 && entry.prev_hash != self.entries[i - 1].hash {
                return Err(AuditError::ChainBroken {
                    index: i,
                    entry_id: entry.id.clone(),
                });
            }
        }

        Ok(())
    }

    // ── Queries ──────────────────────────────────────────────────────────

    /// Get all entries for a specific agent.
    pub fn entries_by_agent(&self, agent_id: &AgentId) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.agent_id == *agent_id).collect()
    }

    /// Get all entries of a specific category.
    pub fn entries_by_category(&self, category: &OpCategory) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.category == *category).collect()
    }

    /// Get all entries at or above a severity threshold.
    pub fn entries_by_severity(&self, min_severity: Severity) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.severity >= min_severity).collect()
    }

    /// Get entries in a timestamp range [from..to].
    pub fn entries_in_range(&self, from: AuditTimestamp, to: AuditTimestamp) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= from && e.timestamp <= to)
            .collect()
    }

    /// Count entries per agent.
    pub fn agent_summary(&self) -> BTreeMap<AgentId, usize> {
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            *counts.entry(entry.agent_id.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Count entries per category.
    pub fn category_summary(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            *counts.entry(entry.category.to_string()).or_insert(0) += 1;
        }
        counts
    }

    /// Count entries per severity.
    pub fn severity_summary(&self) -> BTreeMap<Severity, usize> {
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            *counts.entry(entry.severity).or_insert(0) += 1;
        }
        counts
    }

    /// Get the most recent N entries.
    pub fn recent(&self, n: usize) -> &[AuditEntry] {
        let start = self.entries.len().saturating_sub(n);
        &self.entries[start..]
    }

    /// Get security events (severity >= Warning).
    pub fn security_events(&self) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.category == OpCategory::SecurityEvent || e.severity >= Severity::Warning
            })
            .collect()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

// ── Audit Query Builder ─────────────────────────────────────────────────────

/// Flexible query builder for filtering audit entries.
pub struct AuditQuery<'a> {
    log: &'a AuditLog,
    agent_filter: Option<AgentId>,
    category_filter: Option<OpCategory>,
    severity_filter: Option<Severity>,
    time_from: Option<AuditTimestamp>,
    time_to: Option<AuditTimestamp>,
    description_contains: Option<String>,
    limit: Option<usize>,
}

impl<'a> AuditQuery<'a> {
    /// Create a new query against the given log.
    pub fn new(log: &'a AuditLog) -> Self {
        AuditQuery {
            log,
            agent_filter: None,
            category_filter: None,
            severity_filter: None,
            time_from: None,
            time_to: None,
            description_contains: None,
            limit: None,
        }
    }

    /// Filter by agent.
    pub fn agent(mut self, agent_id: AgentId) -> Self {
        self.agent_filter = Some(agent_id);
        self
    }

    /// Filter by category.
    pub fn category(mut self, category: OpCategory) -> Self {
        self.category_filter = Some(category);
        self
    }

    /// Filter by minimum severity.
    pub fn min_severity(mut self, severity: Severity) -> Self {
        self.severity_filter = Some(severity);
        self
    }

    /// Filter by timestamp range (inclusive start).
    pub fn from_time(mut self, from: AuditTimestamp) -> Self {
        self.time_from = Some(from);
        self
    }

    /// Filter by timestamp range (inclusive end).
    pub fn to_time(mut self, to: AuditTimestamp) -> Self {
        self.time_to = Some(to);
        self
    }

    /// Filter entries whose description contains the given substring.
    pub fn description_contains(mut self, needle: String) -> Self {
        self.description_contains = Some(needle);
        self
    }

    /// Limit the number of results.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Execute the query, returning matching entries.
    pub fn execute(&self) -> Vec<&'a AuditEntry> {
        let mut results: Vec<&AuditEntry> = self
            .log
            .iter()
            .filter(|e| {
                if let Some(ref agent) = self.agent_filter {
                    if e.agent_id != *agent {
                        return false;
                    }
                }
                if let Some(ref cat) = self.category_filter {
                    if e.category != *cat {
                        return false;
                    }
                }
                if let Some(ref sev) = self.severity_filter {
                    if e.severity < *sev {
                        return false;
                    }
                }
                if let Some(from) = self.time_from {
                    if e.timestamp < from {
                        return false;
                    }
                }
                if let Some(to) = self.time_to {
                    if e.timestamp > to {
                        return false;
                    }
                }
                if let Some(ref needle) = self.description_contains {
                    if !e.description.contains(needle.as_str()) {
                        return false;
                    }
                }
                true
            })
            .collect();

        if let Some(limit) = self.limit {
            results.truncate(limit);
        }

        results
    }

    /// Count matching entries without collecting.
    pub fn count(&self) -> usize {
        self.execute().len()
    }
}

// ── Structured Log Export ───────────────────────────────────────────────────

/// Format for structured log export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Structured JSON-like format (one entry per line).
    Structured,
    /// Compact single-line format.
    Compact,
}

/// Export audit entries to a structured string representation.
pub fn export_entries(entries: &[&AuditEntry], format: LogFormat) -> String {
    let mut output = String::new();
    for entry in entries {
        match format {
            LogFormat::Structured => {
                output.push_str(&format!(
                    "{{id:{},ts:{},agent:\"{}\",cat:\"{}\",sev:\"{}\",desc:\"{}\",hash:\"{}\",prev:\"{}\"}}\n",
                    entry.id.0,
                    entry.timestamp,
                    entry.agent_id,
                    entry.category,
                    entry.severity,
                    entry.description,
                    &entry.hash.to_hex()[..16],
                    &entry.prev_hash.to_hex()[..16],
                ));
            }
            LogFormat::Compact => {
                output.push_str(&format!(
                    "{} {} {} {}: {}\n",
                    entry.timestamp, entry.severity, entry.agent_id, entry.category, entry.description,
                ));
            }
        }
    }
    output
}

// ── Replay Support ──────────────────────────────────────────────────────────

/// A replay cursor for walking through audit entries sequentially.
pub struct ReplayCursor<'a> {
    log: &'a AuditLog,
    position: usize,
}

impl<'a> ReplayCursor<'a> {
    /// Create a cursor starting at the beginning of the log.
    pub fn new(log: &'a AuditLog) -> Self {
        ReplayCursor { log, position: 0 }
    }

    /// Create a cursor starting at a specific position.
    pub fn from_position(log: &'a AuditLog, position: usize) -> Self {
        ReplayCursor {
            log,
            position: position.min(log.len()),
        }
    }

    /// Current position (0-based index).
    pub fn position(&self) -> usize {
        self.position
    }

    /// Whether there are more entries to replay.
    pub fn has_next(&self) -> bool {
        self.position < self.log.len()
    }

    /// Get the next entry and advance the cursor.
    pub fn next(&mut self) -> Option<&'a AuditEntry> {
        if self.position < self.log.len() {
            let entry = &self.log.entries[self.position];
            self.position += 1;
            Some(entry)
        } else {
            None
        }
    }

    /// Peek at the next entry without advancing.
    pub fn peek(&self) -> Option<&'a AuditEntry> {
        self.log.entries.get(self.position)
    }

    /// Skip entries that don't match a predicate, stopping at the first match.
    pub fn skip_until(&mut self, predicate: impl Fn(&AuditEntry) -> bool) -> Option<&'a AuditEntry> {
        while self.position < self.log.len() {
            if predicate(&self.log.entries[self.position]) {
                return Some(&self.log.entries[self.position]);
            }
            self.position += 1;
        }
        None
    }

    /// Collect all remaining entries for a specific agent.
    pub fn remaining_for_agent(&mut self, agent_id: &AgentId) -> Vec<&'a AuditEntry> {
        let mut results = Vec::new();
        while self.position < self.log.len() {
            let entry = &self.log.entries[self.position];
            self.position += 1;
            if entry.agent_id == *agent_id {
                results.push(entry);
            }
        }
        results
    }

    /// Reset the cursor to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(name: &str) -> AgentId {
        AgentId::new(name)
    }

    // ── SHA-256 ──────────────────────────────────────────────────────────

    #[test]
    fn sha256_empty_input() {
        let digest = sha256(b"");
        assert_eq!(
            digest.to_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_abc() {
        let digest = sha256(b"abc");
        assert_eq!(
            digest.to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_longer_input() {
        let digest = sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
        assert_eq!(
            digest.to_hex(),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn sha256_digest_hex_roundtrip() {
        let digest = sha256(b"test data");
        let hex = digest.to_hex();
        let parsed = Sha256Digest::from_hex(&hex).unwrap();
        assert_eq!(digest, parsed);
    }

    #[test]
    fn sha256_digest_from_hex_invalid() {
        assert!(Sha256Digest::from_hex("tooshort").is_none());
        assert!(Sha256Digest::from_hex("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz").is_none());
    }

    #[test]
    fn sha256_zero_digest() {
        let zero = Sha256Digest::zero();
        assert_eq!(zero.to_hex(), "0000000000000000000000000000000000000000000000000000000000000000");
    }

    // ── Audit Entry ──────────────────────────────────────────────────────

    #[test]
    fn entry_verify_hash_valid() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "added function foo".to_string());
        assert!(log.get(0).unwrap().verify_hash());
    }

    #[test]
    fn entry_display() {
        let mut log = AuditLog::new();
        log.append_info(agent("synth-1"), OpCategory::TaskOp, "completed task T-42".to_string());
        let entry = log.get(0).unwrap();
        let display = format!("{entry}");
        assert!(display.contains("synth-1"));
        assert!(display.contains("task_op"));
        assert!(display.contains("completed task T-42"));
    }

    // ── Append & Basic Queries ───────────────────────────────────────────

    #[test]
    fn append_single_entry() {
        let mut log = AuditLog::new();
        let id = log.append_info(agent("a1"), OpCategory::SemanticOp, "test op".to_string());
        assert_eq!(id, EntryId(1));
        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());
    }

    #[test]
    fn append_multiple_entries() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "op 1".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "op 2".to_string());
        log.append_info(agent("a1"), OpCategory::TaskOp, "op 3".to_string());
        assert_eq!(log.len(), 3);

        let last = log.last().unwrap();
        assert_eq!(last.id, EntryId(3));
        assert_eq!(last.agent_id, agent("a1"));
    }

    #[test]
    fn get_by_id() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "first".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "second".to_string());

        let entry = log.get_by_id(&EntryId(2)).unwrap();
        assert_eq!(entry.description, "second");
        assert!(log.get_by_id(&EntryId(99)).is_none());
    }

    #[test]
    fn head_hash_changes_on_append() {
        let mut log = AuditLog::new();
        let h0 = log.head_hash();
        assert_eq!(h0, Sha256Digest::zero());

        log.append_info(agent("a1"), OpCategory::SemanticOp, "op 1".to_string());
        let h1 = log.head_hash();
        assert_ne!(h1, h0);

        log.append_info(agent("a2"), OpCategory::LeaseOp, "op 2".to_string());
        let h2 = log.head_hash();
        assert_ne!(h2, h1);
    }

    #[test]
    fn append_with_metadata() {
        let mut log = AuditLog::new();
        let mut meta = BTreeMap::new();
        meta.insert("region".to_string(), "module::parser".to_string());
        meta.insert("lease_id".to_string(), "L-42".to_string());
        log.append(
            agent("synth-3"),
            OpCategory::LeaseOp,
            Severity::Info,
            "acquired lease".to_string(),
            meta,
        );
        let entry = log.get(0).unwrap();
        assert_eq!(entry.metadata.get("region").unwrap(), "module::parser");
        assert_eq!(entry.metadata.get("lease_id").unwrap(), "L-42");
        assert!(entry.verify_hash());
    }

    // ── Hash Chain Verification ──────────────────────────────────────────

    #[test]
    fn verify_empty_chain() {
        let log = AuditLog::new();
        assert!(log.verify_chain().is_ok());
    }

    #[test]
    fn verify_single_entry_chain() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "op".to_string());
        assert!(log.verify_chain().is_ok());
    }

    #[test]
    fn verify_multi_entry_chain() {
        let mut log = AuditLog::new();
        for i in 0..20 {
            log.append_info(
                agent(&format!("agent-{}", i % 5)),
                OpCategory::SemanticOp,
                format!("operation {i}"),
            );
        }
        assert!(log.verify_chain().is_ok());
    }

    #[test]
    fn verify_chain_linkage() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "first".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "second".to_string());
        log.append_info(agent("a3"), OpCategory::TaskOp, "third".to_string());

        // Each entry's prev_hash should match the previous entry's hash
        assert_eq!(log.get(0).unwrap().prev_hash, Sha256Digest::zero());
        assert_eq!(log.get(1).unwrap().prev_hash, log.get(0).unwrap().hash);
        assert_eq!(log.get(2).unwrap().prev_hash, log.get(1).unwrap().hash);
    }

    #[test]
    fn verify_range() {
        let mut log = AuditLog::new();
        for i in 0..10 {
            log.append_info(agent("a1"), OpCategory::SemanticOp, format!("op {i}"));
        }
        assert!(log.verify_range(3, 7).is_ok());
        assert!(log.verify_range(0, 10).is_ok());
        assert!(log.verify_range(5, 5).is_ok()); // empty range
    }

    // ── Agent Query ──────────────────────────────────────────────────────

    #[test]
    fn entries_by_agent() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "op1".to_string());
        log.append_info(agent("a2"), OpCategory::SemanticOp, "op2".to_string());
        log.append_info(agent("a1"), OpCategory::TaskOp, "op3".to_string());
        log.append_info(agent("a3"), OpCategory::LeaseOp, "op4".to_string());
        log.append_info(agent("a1"), OpCategory::VcsOp, "op5".to_string());

        let a1_entries = log.entries_by_agent(&agent("a1"));
        assert_eq!(a1_entries.len(), 3);
        assert_eq!(a1_entries[0].description, "op1");
        assert_eq!(a1_entries[1].description, "op3");
        assert_eq!(a1_entries[2].description, "op5");
    }

    #[test]
    fn entries_by_category() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "s1".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "l1".to_string());
        log.append_info(agent("a1"), OpCategory::SemanticOp, "s2".to_string());

        let semantic = log.entries_by_category(&OpCategory::SemanticOp);
        assert_eq!(semantic.len(), 2);
    }

    #[test]
    fn entries_by_severity() {
        let mut log = AuditLog::new();
        log.append(agent("a1"), OpCategory::SecurityEvent, Severity::Critical, "breach".to_string(), BTreeMap::new());
        log.append(agent("a2"), OpCategory::TaskOp, Severity::Info, "ok".to_string(), BTreeMap::new());
        log.append(agent("a3"), OpCategory::LeaseOp, Severity::Warning, "timeout".to_string(), BTreeMap::new());
        log.append(agent("a1"), OpCategory::SecurityEvent, Severity::Error, "violation".to_string(), BTreeMap::new());

        let warnings_up = log.entries_by_severity(Severity::Warning);
        assert_eq!(warnings_up.len(), 3); // Critical, Warning, Error

        let errors_up = log.entries_by_severity(Severity::Error);
        assert_eq!(errors_up.len(), 2); // Critical, Error

        let critical_only = log.entries_by_severity(Severity::Critical);
        assert_eq!(critical_only.len(), 1);
    }

    #[test]
    fn entries_in_range() {
        let mut log = AuditLog::new();
        for _ in 0..10 {
            log.append_info(agent("a1"), OpCategory::SemanticOp, "op".to_string());
        }
        let range = log.entries_in_range(3, 7);
        assert_eq!(range.len(), 5); // timestamps 3,4,5,6,7
    }

    // ── Summaries ────────────────────────────────────────────────────────

    #[test]
    fn agent_summary() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "op".to_string());
        log.append_info(agent("a2"), OpCategory::SemanticOp, "op".to_string());
        log.append_info(agent("a1"), OpCategory::TaskOp, "op".to_string());

        let summary = log.agent_summary();
        assert_eq!(*summary.get(&agent("a1")).unwrap(), 2);
        assert_eq!(*summary.get(&agent("a2")).unwrap(), 1);
    }

    #[test]
    fn category_summary() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "s".to_string());
        log.append_info(agent("a1"), OpCategory::LeaseOp, "l".to_string());
        log.append_info(agent("a1"), OpCategory::SemanticOp, "s".to_string());

        let summary = log.category_summary();
        assert_eq!(*summary.get("semantic_op").unwrap(), 2);
        assert_eq!(*summary.get("lease_op").unwrap(), 1);
    }

    #[test]
    fn severity_summary() {
        let mut log = AuditLog::new();
        log.append(agent("a1"), OpCategory::TaskOp, Severity::Info, "i".to_string(), BTreeMap::new());
        log.append(agent("a1"), OpCategory::TaskOp, Severity::Warning, "w".to_string(), BTreeMap::new());
        log.append(agent("a1"), OpCategory::TaskOp, Severity::Info, "i".to_string(), BTreeMap::new());

        let summary = log.severity_summary();
        assert_eq!(*summary.get(&Severity::Info).unwrap(), 2);
        assert_eq!(*summary.get(&Severity::Warning).unwrap(), 1);
    }

    #[test]
    fn recent_entries() {
        let mut log = AuditLog::new();
        for i in 0..10 {
            log.append_info(agent("a1"), OpCategory::SemanticOp, format!("op-{i}"));
        }
        let recent = log.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].description, "op-7");
        assert_eq!(recent[2].description, "op-9");
    }

    #[test]
    fn security_events() {
        let mut log = AuditLog::new();
        log.append(agent("a1"), OpCategory::SemanticOp, Severity::Info, "normal".to_string(), BTreeMap::new());
        log.append(agent("a2"), OpCategory::SecurityEvent, Severity::Critical, "breach".to_string(), BTreeMap::new());
        log.append(agent("a3"), OpCategory::TaskOp, Severity::Warning, "slow".to_string(), BTreeMap::new());

        let events = log.security_events();
        assert_eq!(events.len(), 2); // SecurityEvent + Warning
    }

    // ── Query Builder ────────────────────────────────────────────────────

    #[test]
    fn query_by_agent() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "op1".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "op2".to_string());
        log.append_info(agent("a1"), OpCategory::TaskOp, "op3".to_string());

        let results = AuditQuery::new(&log).agent(agent("a1")).execute();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_combined_filters() {
        let mut log = AuditLog::new();
        log.append(agent("a1"), OpCategory::SemanticOp, Severity::Info, "code change".to_string(), BTreeMap::new());
        log.append(agent("a1"), OpCategory::SecurityEvent, Severity::Error, "violation".to_string(), BTreeMap::new());
        log.append(agent("a2"), OpCategory::SecurityEvent, Severity::Critical, "breach".to_string(), BTreeMap::new());
        log.append(agent("a1"), OpCategory::SecurityEvent, Severity::Warning, "suspicious".to_string(), BTreeMap::new());

        let results = AuditQuery::new(&log)
            .agent(agent("a1"))
            .category(OpCategory::SecurityEvent)
            .execute();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].description, "violation");
        assert_eq!(results[1].description, "suspicious");
    }

    #[test]
    fn query_with_time_range() {
        let mut log = AuditLog::new();
        for i in 0..10 {
            log.append_info(agent("a1"), OpCategory::SemanticOp, format!("op-{i}"));
        }

        let results = AuditQuery::new(&log)
            .from_time(3)
            .to_time(5)
            .execute();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn query_description_contains() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "added function foo".to_string());
        log.append_info(agent("a1"), OpCategory::SemanticOp, "removed function bar".to_string());
        log.append_info(agent("a1"), OpCategory::SemanticOp, "added module baz".to_string());

        let results = AuditQuery::new(&log)
            .description_contains("added".to_string())
            .execute();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn query_with_limit() {
        let mut log = AuditLog::new();
        for i in 0..20 {
            log.append_info(agent("a1"), OpCategory::SemanticOp, format!("op-{i}"));
        }

        let results = AuditQuery::new(&log).limit(5).execute();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn query_count() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "op1".to_string());
        log.append_info(agent("a2"), OpCategory::SemanticOp, "op2".to_string());
        log.append_info(agent("a1"), OpCategory::SemanticOp, "op3".to_string());

        let count = AuditQuery::new(&log).agent(agent("a1")).count();
        assert_eq!(count, 2);
    }

    // ── Export ────────────────────────────────────────────────────────────

    #[test]
    fn export_structured() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "test op".to_string());

        let entries: Vec<&AuditEntry> = log.iter().collect();
        let output = export_entries(&entries, LogFormat::Structured);
        assert!(output.contains("agent:\"a1\""));
        assert!(output.contains("cat:\"semantic_op\""));
        assert!(output.contains("desc:\"test op\""));
    }

    #[test]
    fn export_compact() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::TaskOp, "did things".to_string());

        let entries: Vec<&AuditEntry> = log.iter().collect();
        let output = export_entries(&entries, LogFormat::Compact);
        assert!(output.contains("INFO"));
        assert!(output.contains("a1"));
        assert!(output.contains("did things"));
    }

    // ── Replay Cursor ────────────────────────────────────────────────────

    #[test]
    fn replay_cursor_basic() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "first".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "second".to_string());
        log.append_info(agent("a1"), OpCategory::TaskOp, "third".to_string());

        let mut cursor = ReplayCursor::new(&log);
        assert_eq!(cursor.position(), 0);
        assert!(cursor.has_next());

        let e1 = cursor.next().unwrap();
        assert_eq!(e1.description, "first");
        let e2 = cursor.next().unwrap();
        assert_eq!(e2.description, "second");
        let e3 = cursor.next().unwrap();
        assert_eq!(e3.description, "third");
        assert!(!cursor.has_next());
        assert!(cursor.next().is_none());
    }

    #[test]
    fn replay_cursor_peek() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "only".to_string());

        let cursor = ReplayCursor::new(&log);
        let peeked = cursor.peek().unwrap();
        assert_eq!(peeked.description, "only");
        // peek doesn't advance
        assert_eq!(cursor.position(), 0);
    }

    #[test]
    fn replay_cursor_from_position() {
        let mut log = AuditLog::new();
        for i in 0..5 {
            log.append_info(agent("a1"), OpCategory::SemanticOp, format!("op-{i}"));
        }

        let mut cursor = ReplayCursor::from_position(&log, 3);
        assert_eq!(cursor.position(), 3);
        let entry = cursor.next().unwrap();
        assert_eq!(entry.description, "op-3");
    }

    #[test]
    fn replay_cursor_skip_until() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "skip me".to_string());
        log.append_info(agent("a1"), OpCategory::LeaseOp, "skip me too".to_string());
        log.append(agent("a2"), OpCategory::SecurityEvent, Severity::Critical, "found it".to_string(), BTreeMap::new());
        log.append_info(agent("a1"), OpCategory::TaskOp, "after".to_string());

        let mut cursor = ReplayCursor::new(&log);
        let found = cursor.skip_until(|e| e.severity == Severity::Critical);
        assert!(found.is_some());
        assert_eq!(found.unwrap().description, "found it");
        assert_eq!(cursor.position(), 2); // stopped at index 2
    }

    #[test]
    fn replay_cursor_remaining_for_agent() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "a1-first".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "a2-only".to_string());
        log.append_info(agent("a1"), OpCategory::TaskOp, "a1-second".to_string());
        log.append_info(agent("a3"), OpCategory::VcsOp, "a3-only".to_string());
        log.append_info(agent("a1"), OpCategory::SemanticOp, "a1-third".to_string());

        let mut cursor = ReplayCursor::new(&log);
        let a1_entries = cursor.remaining_for_agent(&agent("a1"));
        assert_eq!(a1_entries.len(), 3);
        assert_eq!(a1_entries[0].description, "a1-first");
        assert_eq!(a1_entries[1].description, "a1-second");
        assert_eq!(a1_entries[2].description, "a1-third");
    }

    #[test]
    fn replay_cursor_reset() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "op".to_string());

        let mut cursor = ReplayCursor::new(&log);
        cursor.next();
        assert!(!cursor.has_next());
        cursor.reset();
        assert!(cursor.has_next());
        assert_eq!(cursor.position(), 0);
    }

    // ── Op Category ──────────────────────────────────────────────────────

    #[test]
    fn op_category_display() {
        assert_eq!(OpCategory::SemanticOp.to_string(), "semantic_op");
        assert_eq!(OpCategory::SecurityEvent.to_string(), "security_event");
        assert_eq!(OpCategory::Custom("my_cat".to_string()).to_string(), "custom:my_cat");
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }

    // ── Default ──────────────────────────────────────────────────────────

    #[test]
    fn default_log_is_empty() {
        let log = AuditLog::default();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    // ── Chain Tamper Detection ───────────────────────────────────────────

    #[test]
    fn tampered_entry_detected() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "legit".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "also legit".to_string());

        // Tamper with the first entry's description
        log.entries[0].description = "tampered!".to_string();

        let result = log.verify_chain();
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::HashMismatch { entry_id } => assert_eq!(entry_id, EntryId(1)),
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }

    #[test]
    fn broken_chain_detected() {
        let mut log = AuditLog::new();
        log.append_info(agent("a1"), OpCategory::SemanticOp, "first".to_string());
        log.append_info(agent("a2"), OpCategory::LeaseOp, "second".to_string());
        log.append_info(agent("a3"), OpCategory::TaskOp, "third".to_string());

        // Break the chain by zeroing the second entry's prev_hash
        // but keep its own hash valid for its (now wrong) content
        log.entries[1].prev_hash = Sha256Digest::zero();
        // Recompute entry 1's hash with the wrong prev_hash
        let bytes = AuditEntry::canonical_bytes(
            log.entries[1].id.0,
            log.entries[1].timestamp,
            log.entries[1].agent_id.as_str(),
            &log.entries[1].category.to_string(),
            &log.entries[1].severity.to_string(),
            &log.entries[1].description,
            &log.entries[1].metadata,
            &log.entries[1].prev_hash,
        );
        log.entries[1].hash = sha256(&bytes);

        let result = log.verify_chain();
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::ChainBroken { index, .. } => assert_eq!(index, 1),
            other => panic!("expected ChainBroken, got {other:?}"),
        }
    }
}
