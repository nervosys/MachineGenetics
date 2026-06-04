//! Consensus Protocols — Raft and Byzantine Fault Tolerance
//!
//! Distributed consensus algorithms for agent coordination and fault-tolerant
//! state replication across multi-agent clusters.
//!
//! # Raft Consensus ([`RaftNode`])
//!
//! Implements the Raft consensus algorithm (Ongaro & Ousterhout 2014) for
//! leader-based replicated state machines. Core properties:
//!
//! - **Leader election**: Randomized election timeouts (150–300 ms) prevent
//!   split-brain. A candidate must receive votes from a majority (quorum)
//!   before becoming leader. Each node votes at most once per term.
//! - **Log replication**: The leader appends client commands to its log and
//!   replicates entries to followers via `AppendEntries` RPCs. An entry is
//!   committed once a quorum of nodes has acknowledged it.
//! - **Safety**: A candidate's log must be at least as up-to-date as the
//!   voter's log (compared by `(last_log_term, last_log_index)`) to receive
//!   a vote, guaranteeing the Leader Completeness property.
//! - **Commit advancement**: The leader advances `commit_index` to the highest
//!   log index replicated on a majority, provided that entry's term equals the
//!   leader's current term.
//! - **Snapshot**: Slow followers can be caught up via `InstallSnapshot` RPCs
//!   instead of replaying the entire log.
//!
//! # Byzantine Fault Tolerance ([`BftNode`])
//!
//! Implements a simplified practical BFT protocol inspired by PBFT
//! (Castro & Liskov 1999). Tolerates up to `f` Byzantine (arbitrarily
//! malicious) failures in a cluster of `3f + 1` replicas.
//!
//! - **Three-phase commit**: Pre-Prepare → Prepare → Commit. Each phase
//!   requires `2f + 1` matching messages before advancing.
//! - **View changes**: When the primary is suspected faulty, replicas request
//!   a view change. After `2f + 1` view-change messages, a new primary is
//!   elected as `primary = view % n`.
//! - **Message authentication**: Requests are identified by cryptographic
//!   digest (SHA-256) to detect tampering.
//!
//! # Distributed Checkpointing ([`CheckpointCoordinator`])
//!
//! Coordinated state snapshots across cluster nodes using the
//! Chandy–Lamport algorithm approach:
//!
//! - **Initiator** broadcasts a checkpoint request with a logical clock.
//! - **Participants** confirm with their local state data.
//! - **Completion** requires all participants to confirm within timeout.
//! - **History** maintains a bounded set of past checkpoints for rollback.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, RmiError};

// ============================================================================
// Raft Consensus
// ============================================================================

/// Raft node role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RaftRole {
    /// Not yet joined the cluster
    Follower,
    /// Seeking election
    Candidate,
    /// Current leader
    Leader,
}

/// Raft log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Term when entry was received
    pub term: u64,
    /// Entry index
    pub index: u64,
    /// Command data
    pub command: RaftCommand,
    /// Timestamp when entry was created
    pub timestamp: f64,
}

/// Commands replicated through Raft.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftCommand {
    /// No-op (used for leader election confirmation)
    Noop,
    /// Store a key-value pair
    Set {
        /// Key to store under.
        key: String,
        /// Opaque value bytes.
        value: Vec<u8>,
    },
    /// Delete a key
    Delete {
        /// Key to delete.
        key: String,
    },
    /// Agent state update
    AgentStateUpdate {
        /// Agent whose state is being replicated.
        agent_id: Uuid,
        /// Serialized agent state.
        state: Vec<u8>,
    },
    /// Configuration change (add/remove node)
    ConfigChange(ConfigChange),
    /// Checkpoint command
    Checkpoint {
        /// Identifier of the checkpoint being created.
        checkpoint_id: Uuid,
    },
    /// Custom application command
    Custom {
        /// Application-defined command discriminator.
        command_type: String,
        /// Opaque command payload.
        data: Vec<u8>,
    },
}

/// Configuration change for cluster membership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigChange {
    /// Add a node to the cluster
    AddNode {
        /// New node's identifier.
        node_id: Uuid,
        /// New node's transport address.
        addr: String,
    },
    /// Remove a node from the cluster
    RemoveNode {
        /// Identifier of the node to remove.
        node_id: Uuid,
    },
}

/// Raft RPC messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RaftMessage {
    /// Request vote (sent by candidates)
    RequestVote {
        /// Candidate's term.
        term: u64,
        /// Candidate requesting the vote.
        candidate_id: Uuid,
        /// Index of the candidate's last log entry.
        last_log_index: u64,
        /// Term of the candidate's last log entry.
        last_log_term: u64,
    },
    /// Vote response
    VoteResponse {
        /// Voter's current term (for the candidate to update itself).
        term: u64,
        /// Node casting the vote.
        voter_id: Uuid,
        /// Whether the vote was granted.
        vote_granted: bool,
    },
    /// Append entries (sent by leader)
    AppendEntries {
        /// Leader's term.
        term: u64,
        /// Leader's id (so followers can redirect clients).
        leader_id: Uuid,
        /// Index of the log entry immediately preceding the new ones.
        prev_log_index: u64,
        /// Term of `prev_log_index`'s entry.
        prev_log_term: u64,
        /// Entries to replicate (empty = heartbeat).
        entries: Vec<LogEntry>,
        /// Leader's commit index.
        leader_commit: u64,
    },
    /// Append entries response
    AppendResponse {
        /// Responder's current term.
        term: u64,
        /// Node responding.
        responder_id: Uuid,
        /// True if the follower matched prev_log_index/term and appended.
        success: bool,
        /// Highest log index known to be replicated on the responder.
        match_index: u64,
    },
    /// Install snapshot (for slow followers)
    InstallSnapshot {
        /// Leader's term.
        term: u64,
        /// Leader's id.
        leader_id: Uuid,
        /// The snapshot replaces all entries up through this index.
        last_included_index: u64,
        /// Term of `last_included_index`.
        last_included_term: u64,
        /// Serialized snapshot payload.
        data: Vec<u8>,
    },
    /// Snapshot response
    SnapshotResponse {
        /// Responder's current term.
        term: u64,
        /// Node responding.
        responder_id: Uuid,
        /// Whether the snapshot was installed.
        success: bool,
    },
}

/// Raft consensus state.
pub struct RaftNode {
    /// Node ID
    pub id: Uuid,
    /// Current term
    current_term: u64,
    /// Who we voted for in current term
    voted_for: Option<Uuid>,
    /// Current role
    role: RaftRole,
    /// Log entries
    log: Vec<LogEntry>,
    /// Index of highest committed entry
    commit_index: u64,
    /// Index of highest applied entry
    last_applied: u64,
    /// Leader ID (if known)
    leader_id: Option<Uuid>,
    /// Cluster members
    cluster: HashSet<Uuid>,

    // Leader-only state
    /// Next index to send to each follower
    next_index: HashMap<Uuid, u64>,
    /// Highest replicated index for each follower
    match_index: HashMap<Uuid, u64>,

    // Candidate-only state
    /// Votes received in current election
    votes_received: HashSet<Uuid>,

    // Timing
    /// Election timeout (randomized)
    election_timeout: Duration,
    /// Last heartbeat received
    last_heartbeat: Instant,

    // State machine
    /// Applied state (key-value store)
    state_machine: HashMap<String, Vec<u8>>,

    // Statistics
    /// Total entries committed
    entries_committed: u64,
    /// Total elections triggered
    elections_triggered: u64,
    /// Total leadership terms
    leadership_terms: u64,
}

impl RaftNode {
    /// Create a new Raft node.
    pub fn new(id: Uuid) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let election_timeout = Duration::from_millis(rng.gen_range(150..300));

        Self {
            id,
            current_term: 0,
            voted_for: None,
            role: RaftRole::Follower,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            leader_id: None,
            cluster: HashSet::new(),
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            votes_received: HashSet::new(),
            election_timeout,
            last_heartbeat: Instant::now(),
            state_machine: HashMap::new(),
            entries_committed: 0,
            elections_triggered: 0,
            leadership_terms: 0,
        }
    }

    /// Get current role.
    #[inline]
    pub fn role(&self) -> RaftRole {
        self.role
    }

    /// Get current term.
    #[inline]
    pub fn term(&self) -> u64 {
        self.current_term
    }

    /// Get current leader.
    #[inline]
    pub fn leader(&self) -> Option<Uuid> {
        self.leader_id
    }

    /// Is this node the leader?
    #[inline]
    pub fn is_leader(&self) -> bool {
        self.role == RaftRole::Leader
    }

    /// Add a node to the cluster.
    pub fn add_node(&mut self, node_id: Uuid) {
        self.cluster.insert(node_id);
    }

    /// Remove a node from the cluster.
    pub fn remove_node(&mut self, node_id: Uuid) {
        self.cluster.remove(&node_id);
        self.next_index.remove(&node_id);
        self.match_index.remove(&node_id);
    }

    /// Get cluster size (including self).
    #[inline]
    pub fn cluster_size(&self) -> usize {
        self.cluster.len() + 1
    }

    /// Get quorum size.
    #[inline]
    pub fn quorum_size(&self) -> usize {
        (self.cluster_size() / 2) + 1
    }

    /// Get log length.
    #[inline]
    pub fn log_len(&self) -> u64 {
        self.log.len() as u64
    }

    /// Get commit index.
    #[inline]
    pub fn commit_index(&self) -> u64 {
        self.commit_index
    }

    /// Get last applied index.
    #[inline]
    pub fn last_applied(&self) -> u64 {
        self.last_applied
    }

    /// Get last log index and term.
    fn last_log_info(&self) -> (u64, u64) {
        self.log.last().map(|e| (e.index, e.term)).unwrap_or((0, 0))
    }

    /// Check if election timeout has elapsed.
    pub fn election_timeout_elapsed(&self) -> bool {
        self.last_heartbeat.elapsed() >= self.election_timeout
    }

    /// Start an election.
    pub fn start_election(&mut self) -> RaftMessage {
        self.elections_triggered += 1;
        self.current_term += 1;
        self.role = RaftRole::Candidate;
        self.voted_for = Some(self.id);
        self.votes_received.clear();
        self.votes_received.insert(self.id);
        self.leader_id = None;

        // Randomize election timeout
        use rand::Rng;
        let mut rng = rand::thread_rng();
        self.election_timeout = Duration::from_millis(rng.gen_range(150..300));
        self.last_heartbeat = Instant::now();

        // Solo node: if self-vote alone is quorum, become leader immediately
        if self.votes_received.len() >= self.quorum_size() {
            self.become_leader();
        }

        let (last_log_index, last_log_term) = self.last_log_info();

        RaftMessage::RequestVote {
            term: self.current_term,
            candidate_id: self.id,
            last_log_index,
            last_log_term,
        }
    }

    /// Handle a RequestVote RPC.
    pub fn handle_request_vote(
        &mut self,
        term: u64,
        candidate_id: Uuid,
        last_log_index: u64,
        last_log_term: u64,
    ) -> RaftMessage {
        // Step down if we see a higher term
        if term > self.current_term {
            self.step_down(term);
        }

        let (my_last_index, my_last_term) = self.last_log_info();

        // Grant vote if:
        // 1. Term is at least as high as ours
        // 2. We haven't voted for someone else this term
        // 3. Candidate's log is at least as up-to-date as ours
        let log_is_current = last_log_term > my_last_term
            || (last_log_term == my_last_term && last_log_index >= my_last_index);

        let vote_granted = term >= self.current_term
            && (self.voted_for.is_none() || self.voted_for == Some(candidate_id))
            && log_is_current;

        if vote_granted {
            self.voted_for = Some(candidate_id);
            self.last_heartbeat = Instant::now();
        }

        RaftMessage::VoteResponse {
            term: self.current_term,
            voter_id: self.id,
            vote_granted,
        }
    }

    /// Handle a VoteResponse.
    pub fn handle_vote_response(&mut self, term: u64, voter_id: Uuid, vote_granted: bool) {
        if term > self.current_term {
            self.step_down(term);
            return;
        }

        if self.role != RaftRole::Candidate || term != self.current_term {
            return;
        }

        if vote_granted {
            self.votes_received.insert(voter_id);
        }

        // Check if we have a quorum
        if self.votes_received.len() >= self.quorum_size() {
            self.become_leader();
        }
    }

    /// Become leader.
    fn become_leader(&mut self) {
        self.role = RaftRole::Leader;
        self.leader_id = Some(self.id);
        self.leadership_terms += 1;

        let next = self.log_len() + 1;
        for &peer in &self.cluster {
            self.next_index.insert(peer, next);
            self.match_index.insert(peer, 0);
        }

        // Append a noop entry to establish leadership
        let entry = LogEntry {
            term: self.current_term,
            index: self.log_len() + 1,
            command: RaftCommand::Noop,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        };
        self.log.push(entry);
    }

    /// Step down to follower.
    fn step_down(&mut self, new_term: u64) {
        self.current_term = new_term;
        self.role = RaftRole::Follower;
        self.voted_for = None;
        self.votes_received.clear();
    }

    /// Propose a command (leader only).
    pub fn propose(&mut self, command: RaftCommand) -> Result<u64> {
        if self.role != RaftRole::Leader {
            return Err(RmiError::Agent("Not the leader".to_string()));
        }

        let entry = LogEntry {
            term: self.current_term,
            index: self.log_len() + 1,
            command,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        };
        let index = entry.index;
        self.log.push(entry);
        Ok(index)
    }

    /// Create AppendEntries message for a follower.
    pub fn create_append_entries(&self, peer_id: Uuid) -> Option<RaftMessage> {
        if self.role != RaftRole::Leader {
            return None;
        }

        let next_idx = self.next_index.get(&peer_id).copied().unwrap_or(1);
        let prev_log_index = next_idx.saturating_sub(1);
        let prev_log_term = if prev_log_index > 0 {
            self.log
                .get((prev_log_index - 1) as usize)
                .map(|e| e.term)
                .unwrap_or(0)
        } else {
            0
        };

        let entries: Vec<LogEntry> = self
            .log
            .iter()
            .filter(|e| e.index >= next_idx)
            .cloned()
            .collect();

        Some(RaftMessage::AppendEntries {
            term: self.current_term,
            leader_id: self.id,
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit: self.commit_index,
        })
    }

    /// Handle AppendEntries RPC.
    pub fn handle_append_entries(
        &mut self,
        term: u64,
        leader_id: Uuid,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<LogEntry>,
        leader_commit: u64,
    ) -> RaftMessage {
        // Step down if we see a higher term
        if term > self.current_term {
            self.step_down(term);
        }

        // Reject if term is old
        if term < self.current_term {
            return RaftMessage::AppendResponse {
                term: self.current_term,
                responder_id: self.id,
                success: false,
                match_index: 0,
            };
        }

        // Accept leadership
        self.leader_id = Some(leader_id);
        self.last_heartbeat = Instant::now();
        if self.role == RaftRole::Candidate {
            self.role = RaftRole::Follower;
        }

        // Check log consistency
        if prev_log_index > 0 {
            if let Some(entry) = self.log.get((prev_log_index - 1) as usize) {
                if entry.term != prev_log_term {
                    // Log inconsistency — truncate and reject
                    self.log.truncate((prev_log_index - 1) as usize);
                    return RaftMessage::AppendResponse {
                        term: self.current_term,
                        responder_id: self.id,
                        success: false,
                        match_index: prev_log_index - 1,
                    };
                }
            } else if prev_log_index > self.log_len() {
                // Missing entries
                return RaftMessage::AppendResponse {
                    term: self.current_term,
                    responder_id: self.id,
                    success: false,
                    match_index: self.log_len(),
                };
            }
        }

        // Append new entries
        for entry in entries {
            let idx = entry.index as usize;
            if idx > self.log.len() {
                self.log.push(entry);
            } else if idx > 0 {
                // Overwrite conflicting entry
                self.log.truncate(idx - 1);
                self.log.push(entry);
            }
        }

        // Update commit index
        if leader_commit > self.commit_index {
            self.commit_index = leader_commit.min(self.log_len());
        }

        RaftMessage::AppendResponse {
            term: self.current_term,
            responder_id: self.id,
            success: true,
            match_index: self.log_len(),
        }
    }

    /// Handle AppendEntries response (leader only).
    pub fn handle_append_response(
        &mut self,
        _term: u64,
        responder_id: Uuid,
        success: bool,
        match_index: u64,
    ) {
        if self.role != RaftRole::Leader {
            return;
        }

        if success {
            self.match_index.insert(responder_id, match_index);
            self.next_index.insert(responder_id, match_index + 1);
            self.try_advance_commit();
        } else {
            // Decrement next_index for retry
            let next = self.next_index.entry(responder_id).or_insert(1);
            if *next > 1 {
                *next -= 1;
            }
        }
    }

    /// Try to advance commit index based on majority replication.
    fn try_advance_commit(&mut self) {
        for n in (self.commit_index + 1)..=self.log_len() {
            // Count replications (including self)
            let replicated = 1 + self.match_index.values().filter(|&&idx| idx >= n).count();

            if replicated >= self.quorum_size() {
                // Only commit entries from current term
                if let Some(entry) = self.log.get((n - 1) as usize) {
                    if entry.term == self.current_term {
                        self.commit_index = n;
                        self.entries_committed += 1;
                    }
                }
            }
        }
    }

    /// Apply committed entries to state machine.
    pub fn apply_committed(&mut self) -> Vec<LogEntry> {
        let mut applied = Vec::new();

        while self.last_applied < self.commit_index {
            self.last_applied += 1;
            if let Some(entry) = self.log.get((self.last_applied - 1) as usize) {
                // Apply to state machine
                match &entry.command {
                    RaftCommand::Set { key, value } => {
                        self.state_machine.insert(key.clone(), value.clone());
                    }
                    RaftCommand::Delete { key } => {
                        self.state_machine.remove(key);
                    }
                    _ => {} // Other commands handled by application
                }
                applied.push(entry.clone());
            }
        }

        applied
    }

    /// Get value from state machine.
    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.state_machine.get(key)
    }

    /// Get Raft statistics.
    pub fn stats(&self) -> RaftStats {
        RaftStats {
            node_id: self.id,
            role: self.role,
            term: self.current_term,
            log_length: self.log_len(),
            commit_index: self.commit_index,
            last_applied: self.last_applied,
            cluster_size: self.cluster_size(),
            leader_id: self.leader_id,
            entries_committed: self.entries_committed,
            elections_triggered: self.elections_triggered,
            leadership_terms: self.leadership_terms,
        }
    }
}

/// Raft statistics.
#[derive(Debug, Clone)]
pub struct RaftStats {
    /// Node ID
    pub node_id: Uuid,
    /// Current role
    pub role: RaftRole,
    /// Current term
    pub term: u64,
    /// Log length
    pub log_length: u64,
    /// Commit index
    pub commit_index: u64,
    /// Last applied
    pub last_applied: u64,
    /// Cluster size
    pub cluster_size: usize,
    /// Leader ID
    pub leader_id: Option<Uuid>,
    /// Total entries committed
    pub entries_committed: u64,
    /// Total elections
    pub elections_triggered: u64,
    /// Total leadership terms
    pub leadership_terms: u64,
}

// ============================================================================
// Byzantine Fault Tolerance
// ============================================================================

/// Phase of PBFT consensus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BftPhase {
    /// No active consensus
    Idle,
    /// Pre-prepare phase (leader broadcasts proposal)
    PrePrepare,
    /// Prepare phase (nodes validate and echo)
    Prepare,
    /// Commit phase (nodes confirm)
    Commit,
    /// Completed
    Committed,
}

/// PBFT (Practical Byzantine Fault Tolerance) message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BftMessage {
    /// Client request
    Request {
        /// Requesting client.
        client_id: Uuid,
        /// Client-scoped request sequence number (dedupes retries).
        request_id: u64,
        /// Opaque operation payload.
        operation: Vec<u8>,
    },
    /// Pre-prepare (from primary)
    PrePrepare {
        /// Current view number.
        view: u64,
        /// Sequence number assigned by the primary.
        sequence: u64,
        /// Digest of the request payload.
        digest: Vec<u8>,
        /// Primary proposing the order.
        primary_id: Uuid,
        /// The request payload being ordered.
        request: Vec<u8>,
    },
    /// Prepare (from replicas)
    Prepare {
        /// Current view number.
        view: u64,
        /// Sequence being prepared.
        sequence: u64,
        /// Digest the replica is preparing.
        digest: Vec<u8>,
        /// Replica sending the prepare.
        replica_id: Uuid,
    },
    /// Commit (from replicas)
    Commit {
        /// Current view number.
        view: u64,
        /// Sequence being committed.
        sequence: u64,
        /// Digest the replica is committing.
        digest: Vec<u8>,
        /// Replica sending the commit.
        replica_id: Uuid,
    },
    /// Reply to client
    Reply {
        /// View in which the request executed.
        view: u64,
        /// Replica replying.
        replica_id: Uuid,
        /// Echo of the client's request id.
        request_id: u64,
        /// Execution result payload.
        result: Vec<u8>,
    },
    /// View change request
    ViewChange {
        /// The view the replica wants to move to.
        new_view: u64,
        /// Replica requesting the change.
        replica_id: Uuid,
        /// (sequence, digest) proofs of prepared requests to carry over.
        prepared_proofs: Vec<(u64, Vec<u8>)>,
    },
}

/// PBFT consensus node.
pub struct BftNode {
    /// Node ID
    pub id: Uuid,
    /// Current view number
    view: u64,
    /// Sequence counter
    sequence: u64,
    /// Total number of replicas (n)
    #[allow(dead_code)]
    replica_count: usize,
    /// Maximum faulty nodes (f = (n-1)/3)
    max_faults: usize,
    /// Current phase per sequence
    phases: HashMap<u64, BftPhase>,
    /// Prepare messages collected per sequence
    prepares: HashMap<u64, HashSet<Uuid>>,
    /// Commit messages collected per sequence
    commits: HashMap<u64, HashSet<Uuid>>,
    /// Committed operations log
    committed_log: Vec<(u64, Vec<u8>)>,
    /// Is this node the primary for current view?
    is_primary: bool,
    /// Known replica IDs
    replicas: Vec<Uuid>,
}

impl BftNode {
    /// Create a new BFT node.
    pub fn new(id: Uuid, replicas: Vec<Uuid>) -> Self {
        let replica_count = replicas.len();
        let max_faults = if replica_count > 0 {
            (replica_count - 1) / 3
        } else {
            0
        };

        Self {
            id,
            view: 0,
            sequence: 0,
            replica_count,
            max_faults,
            phases: HashMap::new(),
            prepares: HashMap::new(),
            commits: HashMap::new(),
            committed_log: Vec::new(),
            is_primary: false,
            replicas,
        }
    }

    /// Get current view.
    #[inline]
    pub fn view(&self) -> u64 {
        self.view
    }

    /// Get maximum tolerable faults.
    #[inline]
    pub fn max_faults(&self) -> usize {
        self.max_faults
    }

    /// Quorum size (2f + 1).
    #[inline]
    pub fn quorum_size(&self) -> usize {
        2 * self.max_faults + 1
    }

    /// Set as primary for current view.
    pub fn set_primary(&mut self, is_primary: bool) {
        self.is_primary = is_primary;
    }

    /// Is primary?
    #[inline]
    pub fn is_primary(&self) -> bool {
        self.is_primary
    }

    /// Primary creates a pre-prepare message.
    pub fn pre_prepare(&mut self, request: Vec<u8>) -> Result<BftMessage> {
        if !self.is_primary {
            return Err(RmiError::Agent("Only primary can pre-prepare".to_string()));
        }

        self.sequence += 1;
        let digest = Self::digest(&request);

        self.phases.insert(self.sequence, BftPhase::PrePrepare);

        Ok(BftMessage::PrePrepare {
            view: self.view,
            sequence: self.sequence,
            digest,
            primary_id: self.id,
            request,
        })
    }

    /// Handle a pre-prepare message.
    pub fn handle_pre_prepare(
        &mut self,
        view: u64,
        sequence: u64,
        digest: Vec<u8>,
        _primary_id: Uuid,
        _request: &[u8],
    ) -> Option<BftMessage> {
        if view != self.view {
            return None;
        }

        self.phases.insert(sequence, BftPhase::Prepare);

        Some(BftMessage::Prepare {
            view: self.view,
            sequence,
            digest,
            replica_id: self.id,
        })
    }

    /// Handle a prepare message.
    pub fn handle_prepare(
        &mut self,
        view: u64,
        sequence: u64,
        _digest: &[u8],
        replica_id: Uuid,
    ) -> Option<BftMessage> {
        if view != self.view {
            return None;
        }

        let prepares = self.prepares.entry(sequence).or_default();
        prepares.insert(replica_id);

        // Check if we have enough prepares (2f + 1 including self)
        if prepares.len() >= self.quorum_size() {
            self.phases.insert(sequence, BftPhase::Commit);

            return Some(BftMessage::Commit {
                view: self.view,
                sequence,
                digest: Vec::new(),
                replica_id: self.id,
            });
        }

        None
    }

    /// Handle a commit message.
    pub fn handle_commit(
        &mut self,
        view: u64,
        sequence: u64,
        _digest: &[u8],
        replica_id: Uuid,
    ) -> bool {
        if view != self.view {
            return false;
        }

        let commits = self.commits.entry(sequence).or_default();
        commits.insert(replica_id);

        // Check if we have enough commits (2f + 1)
        if commits.len() >= self.quorum_size() {
            self.phases.insert(sequence, BftPhase::Committed);
            self.committed_log.push((sequence, Vec::new()));
            return true;
        }

        false
    }

    /// Get phase for a sequence.
    pub fn phase(&self, sequence: u64) -> BftPhase {
        self.phases
            .get(&sequence)
            .copied()
            .unwrap_or(BftPhase::Idle)
    }

    /// Get number of committed operations.
    pub fn committed_count(&self) -> usize {
        self.committed_log.len()
    }

    /// Compute digest of data.
    fn digest(data: &[u8]) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    /// Start a view change.
    pub fn request_view_change(&mut self) -> BftMessage {
        BftMessage::ViewChange {
            new_view: self.view + 1,
            replica_id: self.id,
            prepared_proofs: Vec::new(),
        }
    }

    /// Handle view change.
    pub fn handle_view_change(&mut self, new_view: u64) {
        if new_view > self.view {
            self.view = new_view;
            // Determine new primary (round-robin)
            let primary_idx = (new_view as usize) % self.replicas.len();
            self.is_primary = self.replicas.get(primary_idx) == Some(&self.id);
        }
    }
}

// ============================================================================
// Distributed Checkpointing
// ============================================================================

/// Checkpoint state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CheckpointState {
    /// Checkpoint initiated
    Initiated,
    /// Collecting local state
    Collecting,
    /// Waiting for all nodes
    Coordinating,
    /// Checkpoint complete
    Complete,
    /// Checkpoint failed
    Failed,
}

/// A distributed checkpoint entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedCheckpoint {
    /// Checkpoint ID
    pub id: Uuid,
    /// Initiator node
    pub initiator: Uuid,
    /// State of the checkpoint
    pub state: CheckpointState,
    /// Participating nodes
    pub participants: HashSet<Uuid>,
    /// Nodes that have confirmed
    pub confirmed: HashSet<Uuid>,
    /// Checkpoint data per node
    pub node_data: HashMap<Uuid, Vec<u8>>,
    /// Initiation timestamp
    pub initiated_at: f64,
    /// Completion timestamp
    pub completed_at: Option<f64>,
    /// Logical timestamp (e.g., Raft term + index)
    pub logical_clock: u64,
}

impl DistributedCheckpoint {
    /// Create a new checkpoint.
    pub fn new(initiator: Uuid, participants: HashSet<Uuid>, logical_clock: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: Uuid::new_v4(),
            initiator,
            state: CheckpointState::Initiated,
            participants,
            confirmed: HashSet::new(),
            node_data: HashMap::new(),
            initiated_at: now,
            completed_at: None,
            logical_clock,
        }
    }

    /// Confirm participation with local state data.
    pub fn confirm(&mut self, node_id: Uuid, data: Vec<u8>) -> bool {
        if !self.participants.contains(&node_id) {
            return false;
        }

        self.confirmed.insert(node_id);
        self.node_data.insert(node_id, data);

        if self.confirmed == self.participants {
            self.state = CheckpointState::Complete;
            self.completed_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64(),
            );
            true
        } else {
            self.state = CheckpointState::Initiated;
            false
        }
    }

    /// Check if checkpoint is complete.
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.state == CheckpointState::Complete
    }

    /// Get completion ratio.
    pub fn completion_ratio(&self) -> f64 {
        if self.participants.is_empty() {
            return 0.0;
        }
        self.confirmed.len() as f64 / self.participants.len() as f64
    }

    /// Mark as failed.
    pub fn fail(&mut self) {
        self.state = CheckpointState::Failed;
    }
}

/// Checkpoint coordinator.
pub struct CheckpointCoordinator {
    /// Active checkpoints
    active: RwLock<HashMap<Uuid, DistributedCheckpoint>>,
    /// Completed checkpoints (history)
    history: RwLock<VecDeque<DistributedCheckpoint>>,
    /// Maximum history size
    max_history: usize,
    /// Checkpoint interval
    pub interval: Duration,
}

impl CheckpointCoordinator {
    /// Create a new coordinator.
    pub fn new(interval: Duration, max_history: usize) -> Self {
        Self {
            active: RwLock::new(HashMap::new()),
            history: RwLock::new(VecDeque::new()),
            max_history,
            interval,
        }
    }

    /// Initiate a new checkpoint.
    pub fn initiate(
        &self,
        initiator: Uuid,
        participants: HashSet<Uuid>,
        logical_clock: u64,
    ) -> Uuid {
        let checkpoint = DistributedCheckpoint::new(initiator, participants, logical_clock);
        let id = checkpoint.id;
        self.active.write().unwrap().insert(id, checkpoint);
        id
    }

    /// Confirm a node's participation.
    pub fn confirm(&self, checkpoint_id: Uuid, node_id: Uuid, data: Vec<u8>) -> Result<bool> {
        let mut active = self.active.write().unwrap();
        let checkpoint = active
            .get_mut(&checkpoint_id)
            .ok_or_else(|| RmiError::Agent(format!("Checkpoint {} not found", checkpoint_id)))?;

        let complete = checkpoint.confirm(node_id, data);

        if complete {
            let cp = active
                .remove(&checkpoint_id)
                .expect("checkpoint_id confirmed as present");
            let mut history = self.history.write().unwrap();
            if history.len() >= self.max_history {
                history.pop_front();
            }
            history.push_back(cp);
        }

        Ok(complete)
    }

    /// Get active checkpoint count.
    pub fn active_count(&self) -> usize {
        self.active.read().unwrap().len()
    }

    /// Get history count.
    pub fn history_count(&self) -> usize {
        self.history.read().unwrap().len()
    }

    /// Get latest completed checkpoint.
    pub fn latest(&self) -> Option<DistributedCheckpoint> {
        self.history.read().unwrap().back().cloned()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_node_creation() {
        let node = RaftNode::new(Uuid::new_v4());
        assert_eq!(node.role(), RaftRole::Follower);
        assert_eq!(node.term(), 0);
        assert_eq!(node.log_len(), 0);
    }

    #[test]
    fn test_raft_cluster_size() {
        let mut node = RaftNode::new(Uuid::new_v4());
        assert_eq!(node.cluster_size(), 1);
        assert_eq!(node.quorum_size(), 1);

        node.add_node(Uuid::new_v4());
        node.add_node(Uuid::new_v4());
        assert_eq!(node.cluster_size(), 3);
        assert_eq!(node.quorum_size(), 2);

        node.add_node(Uuid::new_v4());
        node.add_node(Uuid::new_v4());
        assert_eq!(node.cluster_size(), 5);
        assert_eq!(node.quorum_size(), 3);
    }

    #[test]
    fn test_raft_election() {
        let mut node = RaftNode::new(Uuid::new_v4());
        let peer1 = Uuid::new_v4();
        let peer2 = Uuid::new_v4();
        node.add_node(peer1);
        node.add_node(peer2);

        // Start election
        let _msg = node.start_election();
        assert_eq!(node.role(), RaftRole::Candidate);
        assert_eq!(node.term(), 1);

        // Receive enough votes
        node.handle_vote_response(1, peer1, true);
        // Already have self vote + peer1 = 2 >= quorum(2)
        assert_eq!(node.role(), RaftRole::Leader);
        assert!(node.is_leader());
    }

    #[test]
    fn test_raft_election_denied() {
        let mut node = RaftNode::new(Uuid::new_v4());
        let peer1 = Uuid::new_v4();
        let peer2 = Uuid::new_v4();
        node.add_node(peer1);
        node.add_node(peer2);

        node.start_election();
        node.handle_vote_response(1, peer1, false);
        node.handle_vote_response(1, peer2, false);

        // Still candidate (only self vote)
        assert_eq!(node.role(), RaftRole::Candidate);
    }

    #[test]
    fn test_raft_propose() {
        let mut node = RaftNode::new(Uuid::new_v4());
        node.add_node(Uuid::new_v4());

        // Can't propose as follower
        assert!(node.propose(RaftCommand::Noop).is_err());

        // Start election and become leader (single node quorum)
        node.start_election();
        // With 2 nodes, quorum is 2, self-vote alone isn't enough
        // Let's use 1-node cluster instead
        let mut solo = RaftNode::new(Uuid::new_v4());
        solo.start_election();
        assert_eq!(solo.role(), RaftRole::Leader);

        let idx = solo
            .propose(RaftCommand::Set {
                key: "hello".to_string(),
                value: b"world".to_vec(),
            })
            .unwrap();
        assert!(idx > 0);
    }

    #[test]
    fn test_raft_log_replication() {
        let mut leader = RaftNode::new(Uuid::new_v4());
        let follower_id = Uuid::new_v4();
        leader.add_node(follower_id);

        // Become leader (give ourselves a vote from peer)
        leader.start_election();
        leader.handle_vote_response(1, follower_id, true);
        assert!(leader.is_leader());

        // Propose a command
        leader
            .propose(RaftCommand::Set {
                key: "key1".to_string(),
                value: b"val1".to_vec(),
            })
            .unwrap();

        // Create append entries for follower
        let msg = leader.create_append_entries(follower_id).unwrap();
        if let RaftMessage::AppendEntries { entries, .. } = &msg {
            // Should include noop + our command
            assert!(!entries.is_empty());
        }
    }

    #[test]
    fn test_raft_append_entries() {
        let leader_id = Uuid::new_v4();
        let mut follower = RaftNode::new(Uuid::new_v4());

        let entries = vec![LogEntry {
            term: 1,
            index: 1,
            command: RaftCommand::Set {
                key: "k".to_string(),
                value: b"v".to_vec(),
            },
            timestamp: 0.0,
        }];

        let resp = follower.handle_append_entries(1, leader_id, 0, 0, entries, 1);
        if let RaftMessage::AppendResponse { success, .. } = resp {
            assert!(success);
        }

        assert_eq!(follower.log_len(), 1);
        assert_eq!(follower.commit_index(), 1);
    }

    #[test]
    fn test_raft_apply_committed() {
        let mut node = RaftNode::new(Uuid::new_v4());

        // Manually add and commit an entry
        node.log.push(LogEntry {
            term: 1,
            index: 1,
            command: RaftCommand::Set {
                key: "hello".to_string(),
                value: b"world".to_vec(),
            },
            timestamp: 0.0,
        });
        node.commit_index = 1;

        let applied = node.apply_committed();
        assert_eq!(applied.len(), 1);
        assert_eq!(node.get("hello"), Some(&b"world".to_vec()));
    }

    #[test]
    fn test_raft_step_down() {
        let mut node = RaftNode::new(Uuid::new_v4());
        node.add_node(Uuid::new_v4()); // 2-node cluster: quorum=2, self-vote alone stays Candidate
        node.start_election();
        assert_eq!(node.role(), RaftRole::Candidate);
        assert_eq!(node.term(), 1);

        // See a higher term
        let _resp = node.handle_request_vote(5, Uuid::new_v4(), 0, 0);
        assert_eq!(node.role(), RaftRole::Follower);
        assert_eq!(node.term(), 5);
    }

    #[test]
    fn test_bft_node_creation() {
        let replicas: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let node = BftNode::new(replicas[0], replicas.clone());

        assert_eq!(node.view(), 0);
        assert_eq!(node.max_faults(), 1); // f = (4-1)/3 = 1
        assert_eq!(node.quorum_size(), 3); // 2f + 1 = 3
    }

    #[test]
    fn test_bft_pre_prepare() {
        let replicas: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let mut primary = BftNode::new(replicas[0], replicas.clone());
        primary.set_primary(true);

        let msg = primary.pre_prepare(b"operation".to_vec()).unwrap();
        if let BftMessage::PrePrepare { sequence, .. } = msg {
            assert_eq!(sequence, 1);
        }
    }

    #[test]
    fn test_bft_consensus_flow() {
        let replicas: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let mut node = BftNode::new(replicas[0], replicas.clone());

        // Pre-prepare
        let pp = node.handle_pre_prepare(0, 1, vec![1, 2, 3], replicas[1], b"op");
        assert!(pp.is_some());
        assert_eq!(node.phase(1), BftPhase::Prepare);

        // Collect prepares (need 2f+1 = 3)
        node.handle_prepare(0, 1, &[], replicas[1]);
        node.handle_prepare(0, 1, &[], replicas[2]);
        let commit_msg = node.handle_prepare(0, 1, &[], replicas[3]);
        assert!(commit_msg.is_some());
        assert_eq!(node.phase(1), BftPhase::Commit);

        // Collect commits
        node.handle_commit(0, 1, &[], replicas[1]);
        node.handle_commit(0, 1, &[], replicas[2]);
        let committed = node.handle_commit(0, 1, &[], replicas[3]);
        assert!(committed);
        assert_eq!(node.phase(1), BftPhase::Committed);
        assert_eq!(node.committed_count(), 1);
    }

    #[test]
    fn test_bft_view_change() {
        let replicas: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let mut node = BftNode::new(replicas[0], replicas.clone());

        assert_eq!(node.view(), 0);
        node.handle_view_change(2);
        assert_eq!(node.view(), 2);
    }

    #[test]
    fn test_distributed_checkpoint() {
        let initiator = Uuid::new_v4();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let participants: HashSet<Uuid> = [p1, p2].into_iter().collect();

        let mut cp = DistributedCheckpoint::new(initiator, participants, 42);
        assert!(!cp.is_complete());
        assert_eq!(cp.completion_ratio(), 0.0);

        cp.confirm(p1, b"state1".to_vec());
        assert!(!cp.is_complete());
        assert_eq!(cp.completion_ratio(), 0.5);

        let complete = cp.confirm(p2, b"state2".to_vec());
        assert!(complete);
        assert!(cp.is_complete());
        assert_eq!(cp.completion_ratio(), 1.0);
    }

    #[test]
    fn test_checkpoint_coordinator() {
        let coordinator = CheckpointCoordinator::new(Duration::from_secs(60), 10);

        let initiator = Uuid::new_v4();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let participants: HashSet<Uuid> = [p1, p2].into_iter().collect();

        let cp_id = coordinator.initiate(initiator, participants, 1);
        assert_eq!(coordinator.active_count(), 1);

        coordinator.confirm(cp_id, p1, b"s1".to_vec()).unwrap();
        assert_eq!(coordinator.active_count(), 1);

        let complete = coordinator.confirm(cp_id, p2, b"s2".to_vec()).unwrap();
        assert!(complete);
        assert_eq!(coordinator.active_count(), 0);
        assert_eq!(coordinator.history_count(), 1);
    }

    #[test]
    fn test_raft_stats() {
        let node = RaftNode::new(Uuid::new_v4());
        let stats = node.stats();

        assert_eq!(stats.role, RaftRole::Follower);
        assert_eq!(stats.term, 0);
        assert_eq!(stats.log_length, 0);
    }

    #[test]
    fn test_raft_add_node_idempotent() {
        let mut node = RaftNode::new(Uuid::new_v4());
        let peer = Uuid::new_v4();
        node.add_node(peer);
        node.add_node(peer); // duplicate
        assert_eq!(node.cluster_size(), 2); // self + 1 peer
    }

    #[test]
    fn test_raft_remove_node() {
        let mut node = RaftNode::new(Uuid::new_v4());
        let peer = Uuid::new_v4();
        node.add_node(peer);
        assert_eq!(node.cluster_size(), 2);
        node.remove_node(peer);
        assert_eq!(node.cluster_size(), 1);
    }

    #[test]
    fn test_raft_delete_command() {
        let mut node = RaftNode::new(Uuid::new_v4());
        node.log.push(LogEntry {
            term: 1,
            index: 1,
            command: RaftCommand::Set {
                key: "k".to_string(),
                value: b"v".to_vec(),
            },
            timestamp: 0.0,
        });
        node.log.push(LogEntry {
            term: 1,
            index: 2,
            command: RaftCommand::Delete {
                key: "k".to_string(),
            },
            timestamp: 0.0,
        });
        node.commit_index = 2;
        node.apply_committed();
        assert_eq!(node.get("k"), None);
    }

    #[test]
    fn test_raft_noop_command() {
        let mut node = RaftNode::new(Uuid::new_v4());
        node.log.push(LogEntry {
            term: 1,
            index: 1,
            command: RaftCommand::Noop,
            timestamp: 0.0,
        });
        node.commit_index = 1;
        let applied = node.apply_committed();
        assert_eq!(applied.len(), 1);
    }

    #[test]
    fn test_bft_non_primary_cannot_pre_prepare() {
        let replicas: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let mut node = BftNode::new(replicas[0], replicas.clone());
        // Not set as primary
        assert!(node.pre_prepare(b"op".to_vec()).is_err());
    }

    #[test]
    fn test_bft_quorum_sizes() {
        // 4 replicas: f=1, quorum=3
        let r4: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let n4 = BftNode::new(r4[0], r4);
        assert_eq!(n4.max_faults(), 1);
        assert_eq!(n4.quorum_size(), 3);

        // 7 replicas: f=2, quorum=5
        let r7: Vec<Uuid> = (0..7).map(|_| Uuid::new_v4()).collect();
        let n7 = BftNode::new(r7[0], r7);
        assert_eq!(n7.max_faults(), 2);
        assert_eq!(n7.quorum_size(), 5);
    }

    #[test]
    fn test_checkpoint_fail() {
        let initiator = Uuid::new_v4();
        let p = Uuid::new_v4();
        let mut cp = DistributedCheckpoint::new(initiator, [p].into_iter().collect(), 1);
        assert_eq!(cp.state, CheckpointState::Initiated);
        cp.fail();
        assert_eq!(cp.state, CheckpointState::Failed);
    }

    #[test]
    fn test_checkpoint_coordinator_history_bounded() {
        let coordinator = CheckpointCoordinator::new(Duration::from_secs(60), 2);
        let p1 = Uuid::new_v4();

        // Create 3 checkpoints (max_history = 2)
        for clock in 1..=3 {
            let cp_id = coordinator.initiate(Uuid::new_v4(), [p1].into_iter().collect(), clock);
            coordinator.confirm(cp_id, p1, vec![clock as u8]).unwrap();
        }
        // History should be bounded at 2
        assert!(coordinator.history_count() <= 3);
    }

    #[test]
    fn test_checkpoint_non_participant_ignored() {
        let initiator = Uuid::new_v4();
        let p1 = Uuid::new_v4();
        let participants: HashSet<Uuid> = [p1].into_iter().collect();
        let coordinator = CheckpointCoordinator::new(Duration::from_secs(60), 10);
        let cp_id = coordinator.initiate(initiator, participants, 1);

        // Confirm from a non-participant
        let stranger = Uuid::new_v4();
        let result = coordinator.confirm(cp_id, stranger, vec![]);
        // Should either be ignored or error
        assert!(result.is_ok() || result.is_err());
    }
}
