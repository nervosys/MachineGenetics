// Redox Consensus Protocol Engine
//
// Implements the propose → vote → resolve → integrate cycle
// for swarm consensus on shared interface changes (§7.6 of REDOX_PROPOSAL.md).
//
// Supports configurable quorum rules: majority, supermajority, unanimous,
// weighted, and custom threshold.
//
// (ROADMAP Step 51)

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ── Proposal ───────────────────────────────────────────────────────────────

/// Unique proposal identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProposalId(pub String);

impl ProposalId {
    pub fn new(id: &str) -> Self {
        ProposalId(id.to_string())
    }
}

impl fmt::Display for ProposalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A change proposal submitted by an agent.
#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: ProposalId,
    pub author: String,
    pub description: String,
    pub change_kind: ChangeKind,
    pub affected_regions: Vec<String>,
    pub status: ProposalStatus,
}

impl Proposal {
    pub fn new(id: &str, author: &str, description: &str, kind: ChangeKind) -> Self {
        Proposal {
            id: ProposalId::new(id),
            author: author.to_string(),
            description: description.to_string(),
            change_kind: kind,
            affected_regions: Vec::new(),
            status: ProposalStatus::Open,
        }
    }

    pub fn with_region(mut self, region: &str) -> Self {
        self.affected_regions.push(region.to_string());
        self
    }
}

/// What kind of change is being proposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// Modify a type signature.
    ModifySignature,
    /// Modify a trait definition.
    ModifyTrait,
    /// Modify a public API.
    ModifyPublicApi,
    /// Rename a symbol.
    Rename,
    /// Add a new public item.
    AddPublicItem,
    /// Remove a public item.
    RemovePublicItem,
    /// Restructure module layout.
    Restructure,
    /// Other.
    Other(String),
}

impl fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeKind::ModifySignature => write!(f, "modify_signature"),
            ChangeKind::ModifyTrait => write!(f, "modify_trait"),
            ChangeKind::ModifyPublicApi => write!(f, "modify_public_api"),
            ChangeKind::Rename => write!(f, "rename"),
            ChangeKind::AddPublicItem => write!(f, "add_public_item"),
            ChangeKind::RemovePublicItem => write!(f, "remove_public_item"),
            ChangeKind::Restructure => write!(f, "restructure"),
            ChangeKind::Other(s) => write!(f, "other:{s}"),
        }
    }
}

/// Lifecycle status of a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalStatus {
    Open,
    Voting,
    Accepted,
    Rejected(String),
    Integrated,
    Withdrawn,
}

impl fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProposalStatus::Open => write!(f, "open"),
            ProposalStatus::Voting => write!(f, "voting"),
            ProposalStatus::Accepted => write!(f, "accepted"),
            ProposalStatus::Rejected(r) => write!(f, "rejected: {r}"),
            ProposalStatus::Integrated => write!(f, "integrated"),
            ProposalStatus::Withdrawn => write!(f, "withdrawn"),
        }
    }
}

// ── Voting ─────────────────────────────────────────────────────────────────

/// A vote cast by an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vote {
    pub voter: String,
    pub decision: VoteDecision,
    pub reason: Option<String>,
}

impl Vote {
    pub fn approve(voter: &str) -> Self {
        Vote { voter: voter.to_string(), decision: VoteDecision::Approve, reason: None }
    }

    pub fn reject(voter: &str, reason: &str) -> Self {
        Vote {
            voter: voter.to_string(),
            decision: VoteDecision::Reject,
            reason: Some(reason.to_string()),
        }
    }

    pub fn abstain(voter: &str) -> Self {
        Vote { voter: voter.to_string(), decision: VoteDecision::Abstain, reason: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteDecision {
    Approve,
    Reject,
    Abstain,
}

// ── Quorum Rules ───────────────────────────────────────────────────────────

/// Configurable quorum rule.
#[derive(Debug, Clone, PartialEq)]
pub enum QuorumRule {
    /// Simple majority (> 50%).
    Majority,
    /// Supermajority (>= 2/3).
    Supermajority,
    /// All must approve.
    Unanimous,
    /// Custom threshold (0.0..=1.0).
    Threshold(f64),
    /// Weighted voting with per-voter weights.
    Weighted {
        weights: BTreeMap<String, f64>,
        threshold: f64,
    },
}

impl QuorumRule {
    /// Check whether the quorum is reached given votes and eligible voters.
    pub fn is_met(&self, votes: &[Vote], eligible_voters: &[String]) -> QuorumResult {
        let total = eligible_voters.len();
        if total == 0 {
            return QuorumResult {
                met: false,
                approve_count: 0,
                reject_count: 0,
                abstain_count: 0,
                total_eligible: 0,
                threshold_needed: 0.0,
                achieved_ratio: 0.0,
            };
        }

        let approve_count = votes.iter()
            .filter(|v| v.decision == VoteDecision::Approve)
            .count();
        let reject_count = votes.iter()
            .filter(|v| v.decision == VoteDecision::Reject)
            .count();
        let abstain_count = votes.iter()
            .filter(|v| v.decision == VoteDecision::Abstain)
            .count();

        let (met, threshold_needed, achieved_ratio) = match self {
            QuorumRule::Majority => {
                let threshold = 0.5;
                let ratio = approve_count as f64 / total as f64;
                (ratio > threshold, threshold, ratio)
            }
            QuorumRule::Supermajority => {
                let threshold = 2.0 / 3.0;
                let ratio = approve_count as f64 / total as f64;
                (ratio >= threshold, threshold, ratio)
            }
            QuorumRule::Unanimous => {
                let ratio = approve_count as f64 / total as f64;
                (approve_count == total, 1.0, ratio)
            }
            QuorumRule::Threshold(t) => {
                let ratio = approve_count as f64 / total as f64;
                (ratio >= *t, *t, ratio)
            }
            QuorumRule::Weighted { weights, threshold } => {
                let total_weight: f64 = eligible_voters.iter()
                    .map(|v| weights.get(v).copied().unwrap_or(1.0))
                    .sum();
                let approve_weight: f64 = votes.iter()
                    .filter(|v| v.decision == VoteDecision::Approve)
                    .map(|v| weights.get(&v.voter).copied().unwrap_or(1.0))
                    .sum();
                let ratio = if total_weight > 0.0 {
                    approve_weight / total_weight
                } else {
                    0.0
                };
                (ratio >= *threshold, *threshold, ratio)
            }
        };

        QuorumResult {
            met,
            approve_count,
            reject_count,
            abstain_count,
            total_eligible: total,
            threshold_needed,
            achieved_ratio,
        }
    }
}

impl fmt::Display for QuorumRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuorumRule::Majority => write!(f, "majority (>50%)"),
            QuorumRule::Supermajority => write!(f, "supermajority (>=2/3)"),
            QuorumRule::Unanimous => write!(f, "unanimous"),
            QuorumRule::Threshold(t) => write!(f, "threshold ({:.0}%)", t * 100.0),
            QuorumRule::Weighted { threshold, .. } => {
                write!(f, "weighted ({:.0}%)", threshold * 100.0)
            }
        }
    }
}

/// Result of a quorum check.
#[derive(Debug, Clone)]
pub struct QuorumResult {
    pub met: bool,
    pub approve_count: usize,
    pub reject_count: usize,
    pub abstain_count: usize,
    pub total_eligible: usize,
    pub threshold_needed: f64,
    pub achieved_ratio: f64,
}

// ── Consensus Engine ───────────────────────────────────────────────────────

/// The consensus protocol engine.
///
/// Lifecycle: propose → vote → resolve → integrate
pub struct ConsensusEngine {
    proposals: BTreeMap<ProposalId, Proposal>,
    votes: BTreeMap<ProposalId, Vec<Vote>>,
    eligible_voters: BTreeSet<String>,
    quorum_rule: QuorumRule,
    history: Vec<ConsensusEvent>,
}

/// An event in the consensus history.
#[derive(Debug, Clone)]
pub struct ConsensusEvent {
    pub proposal_id: ProposalId,
    pub event_kind: EventKind,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    Proposed,
    VoteCast(VoteDecision),
    Resolved(bool),
    Integrated,
    Withdrawn,
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventKind::Proposed => write!(f, "proposed"),
            EventKind::VoteCast(d) => match d {
                VoteDecision::Approve => write!(f, "vote:approve"),
                VoteDecision::Reject => write!(f, "vote:reject"),
                VoteDecision::Abstain => write!(f, "vote:abstain"),
            },
            EventKind::Resolved(accepted) => {
                if *accepted { write!(f, "resolved:accepted") }
                else { write!(f, "resolved:rejected") }
            }
            EventKind::Integrated => write!(f, "integrated"),
            EventKind::Withdrawn => write!(f, "withdrawn"),
        }
    }
}

/// Errors from consensus operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsensusError {
    ProposalNotFound(String),
    ProposalNotOpen(String),
    ProposalNotVoting(String),
    ProposalNotAccepted(String),
    VoterNotEligible(String),
    DuplicateVote(String),
    AlreadyResolved(String),
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConsensusError::ProposalNotFound(id) => write!(f, "proposal not found: {id}"),
            ConsensusError::ProposalNotOpen(id) => write!(f, "proposal not open: {id}"),
            ConsensusError::ProposalNotVoting(id) => write!(f, "proposal not in voting: {id}"),
            ConsensusError::ProposalNotAccepted(id) => write!(f, "proposal not accepted: {id}"),
            ConsensusError::VoterNotEligible(v) => write!(f, "voter not eligible: {v}"),
            ConsensusError::DuplicateVote(v) => write!(f, "duplicate vote from: {v}"),
            ConsensusError::AlreadyResolved(id) => write!(f, "already resolved: {id}"),
        }
    }
}

impl ConsensusEngine {
    /// Create a new engine with the given quorum rule.
    pub fn new(quorum_rule: QuorumRule) -> Self {
        ConsensusEngine {
            proposals: BTreeMap::new(),
            votes: BTreeMap::new(),
            eligible_voters: BTreeSet::new(),
            quorum_rule,
            history: Vec::new(),
        }
    }

    /// Register an eligible voter.
    pub fn register_voter(&mut self, voter: &str) {
        self.eligible_voters.insert(voter.to_string());
    }

    /// Remove an eligible voter.
    pub fn unregister_voter(&mut self, voter: &str) {
        self.eligible_voters.remove(voter);
    }

    /// Get the current quorum rule.
    pub fn quorum_rule(&self) -> &QuorumRule {
        &self.quorum_rule
    }

    /// Change the quorum rule.
    pub fn set_quorum_rule(&mut self, rule: QuorumRule) {
        self.quorum_rule = rule;
    }

    /// Number of eligible voters.
    pub fn voter_count(&self) -> usize {
        self.eligible_voters.len()
    }

    // ── Phase 1: Propose ──

    /// Submit a proposal. Transitions to Voting.
    pub fn propose(&mut self, mut proposal: Proposal) -> Result<ProposalId, ConsensusError> {
        let id = proposal.id.clone();
        proposal.status = ProposalStatus::Voting;
        self.votes.insert(id.clone(), Vec::new());
        self.history.push(ConsensusEvent {
            proposal_id: id.clone(),
            event_kind: EventKind::Proposed,
            actor: proposal.author.clone(),
        });
        self.proposals.insert(id.clone(), proposal);
        Ok(id)
    }

    // ── Phase 2: Vote ──

    /// Cast a vote on a proposal.
    pub fn vote(
        &mut self,
        proposal_id: &ProposalId,
        vote: Vote,
    ) -> Result<(), ConsensusError> {
        // Check proposal exists and is in voting.
        let proposal = self.proposals.get(proposal_id)
            .ok_or_else(|| ConsensusError::ProposalNotFound(proposal_id.0.clone()))?;
        if proposal.status != ProposalStatus::Voting {
            return Err(ConsensusError::ProposalNotVoting(proposal_id.0.clone()));
        }

        // Check voter is eligible.
        if !self.eligible_voters.contains(&vote.voter) {
            return Err(ConsensusError::VoterNotEligible(vote.voter.clone()));
        }

        // Check for duplicate votes.
        let votes = self.votes.entry(proposal_id.clone()).or_default();
        if votes.iter().any(|v| v.voter == vote.voter) {
            return Err(ConsensusError::DuplicateVote(vote.voter.clone()));
        }

        self.history.push(ConsensusEvent {
            proposal_id: proposal_id.clone(),
            event_kind: EventKind::VoteCast(vote.decision.clone()),
            actor: vote.voter.clone(),
        });
        votes.push(vote);
        Ok(())
    }

    // ── Phase 3: Resolve ──

    /// Resolve a proposal by checking quorum.
    /// Returns `true` if accepted, `false` if rejected.
    pub fn resolve(&mut self, proposal_id: &ProposalId) -> Result<bool, ConsensusError> {
        let proposal = self.proposals.get(proposal_id)
            .ok_or_else(|| ConsensusError::ProposalNotFound(proposal_id.0.clone()))?;
        if proposal.status != ProposalStatus::Voting {
            return Err(ConsensusError::ProposalNotVoting(proposal_id.0.clone()));
        }

        let eligible: Vec<String> = self.eligible_voters.iter().cloned().collect();
        let votes = self.votes.get(proposal_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let result = self.quorum_rule.is_met(votes, &eligible);
        let accepted = result.met;

        let proposal = self.proposals.get_mut(proposal_id).unwrap();
        if accepted {
            proposal.status = ProposalStatus::Accepted;
        } else {
            proposal.status = ProposalStatus::Rejected(
                format!("quorum not met: {:.1}% < {:.1}%",
                    result.achieved_ratio * 100.0,
                    result.threshold_needed * 100.0)
            );
        }

        self.history.push(ConsensusEvent {
            proposal_id: proposal_id.clone(),
            event_kind: EventKind::Resolved(accepted),
            actor: "engine".to_string(),
        });

        Ok(accepted)
    }

    // ── Phase 4: Integrate ──

    /// Mark a proposal as integrated (applied to codebase).
    pub fn integrate(&mut self, proposal_id: &ProposalId) -> Result<(), ConsensusError> {
        let proposal = self.proposals.get_mut(proposal_id)
            .ok_or_else(|| ConsensusError::ProposalNotFound(proposal_id.0.clone()))?;
        if proposal.status != ProposalStatus::Accepted {
            return Err(ConsensusError::ProposalNotAccepted(proposal_id.0.clone()));
        }
        proposal.status = ProposalStatus::Integrated;
        self.history.push(ConsensusEvent {
            proposal_id: proposal_id.clone(),
            event_kind: EventKind::Integrated,
            actor: "engine".to_string(),
        });
        Ok(())
    }

    // ── Withdraw ──

    /// Withdraw a proposal (author only, before resolution).
    pub fn withdraw(
        &mut self,
        proposal_id: &ProposalId,
        actor: &str,
    ) -> Result<(), ConsensusError> {
        let proposal = self.proposals.get_mut(proposal_id)
            .ok_or_else(|| ConsensusError::ProposalNotFound(proposal_id.0.clone()))?;
        match &proposal.status {
            ProposalStatus::Open | ProposalStatus::Voting => {}
            _ => return Err(ConsensusError::AlreadyResolved(proposal_id.0.clone())),
        }
        proposal.status = ProposalStatus::Withdrawn;
        self.history.push(ConsensusEvent {
            proposal_id: proposal_id.clone(),
            event_kind: EventKind::Withdrawn,
            actor: actor.to_string(),
        });
        Ok(())
    }

    // ── Queries ──

    /// Get a proposal by ID.
    pub fn get_proposal(&self, id: &ProposalId) -> Option<&Proposal> {
        self.proposals.get(id)
    }

    /// Get votes for a proposal.
    pub fn get_votes(&self, id: &ProposalId) -> &[Vote] {
        self.votes.get(id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check quorum status for a proposal without resolving it.
    pub fn check_quorum(&self, proposal_id: &ProposalId) -> Option<QuorumResult> {
        let votes = self.votes.get(proposal_id)?;
        let eligible: Vec<String> = self.eligible_voters.iter().cloned().collect();
        Some(self.quorum_rule.is_met(votes, &eligible))
    }

    /// All proposals.
    pub fn proposals(&self) -> impl Iterator<Item = &Proposal> {
        self.proposals.values()
    }

    /// Event history.
    pub fn history(&self) -> &[ConsensusEvent] {
        &self.history
    }

    /// All open/voting proposals.
    pub fn active_proposals(&self) -> Vec<&Proposal> {
        self.proposals.values()
            .filter(|p| matches!(p.status, ProposalStatus::Open | ProposalStatus::Voting))
            .collect()
    }

    /// Number of proposals by status.
    pub fn count_by_status(&self, status: &ProposalStatus) -> usize {
        self.proposals.values()
            .filter(|p| {
                // Compare discriminant only.
                std::mem::discriminant(&p.status) == std::mem::discriminant(status)
            })
            .count()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_engine() -> ConsensusEngine {
        let mut engine = ConsensusEngine::new(QuorumRule::Majority);
        engine.register_voter("alice");
        engine.register_voter("bob");
        engine.register_voter("carol");
        engine
    }

    fn make_proposal() -> Proposal {
        Proposal::new("p1", "alice", "change return type", ChangeKind::ModifySignature)
            .with_region("module::foo")
    }

    // ── ProposalId ──

    #[test]
    fn proposal_id_display() {
        let id = ProposalId::new("abc");
        assert_eq!(format!("{id}"), "abc");
    }

    // ── ChangeKind ──

    #[test]
    fn change_kind_display() {
        assert_eq!(format!("{}", ChangeKind::ModifySignature), "modify_signature");
        assert_eq!(format!("{}", ChangeKind::Rename), "rename");
        assert_eq!(format!("{}", ChangeKind::Other("x".to_string())), "other:x");
    }

    // ── ProposalStatus ──

    #[test]
    fn proposal_status_display() {
        assert_eq!(format!("{}", ProposalStatus::Open), "open");
        assert_eq!(format!("{}", ProposalStatus::Voting), "voting");
        assert_eq!(format!("{}", ProposalStatus::Integrated), "integrated");
    }

    // ── Vote ──

    #[test]
    fn vote_constructors() {
        let a = Vote::approve("alice");
        assert_eq!(a.decision, VoteDecision::Approve);
        let r = Vote::reject("bob", "bad idea");
        assert_eq!(r.decision, VoteDecision::Reject);
        assert_eq!(r.reason.as_deref(), Some("bad idea"));
        let ab = Vote::abstain("carol");
        assert_eq!(ab.decision, VoteDecision::Abstain);
    }

    // ── QuorumRule ──

    #[test]
    fn quorum_majority() {
        let rule = QuorumRule::Majority;
        let voters = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        // 2 out of 3 approve => >50%
        let votes = vec![Vote::approve("a"), Vote::approve("b")];
        assert!(rule.is_met(&votes, &voters).met);

        // 1 out of 3 approve => ~33%, not > 50%
        let votes = vec![Vote::approve("a"), Vote::reject("b", "no")];
        assert!(!rule.is_met(&votes, &voters).met);
    }

    #[test]
    fn quorum_supermajority() {
        let rule = QuorumRule::Supermajority;
        let voters = vec!["a".into(), "b".into(), "c".into()];

        // 2/3 = 66.7% >= 66.7%
        let votes = vec![Vote::approve("a"), Vote::approve("b")];
        assert!(rule.is_met(&votes, &voters).met);

        // 1/3 = 33.3% < 66.7%
        let votes = vec![Vote::approve("a")];
        assert!(!rule.is_met(&votes, &voters).met);
    }

    #[test]
    fn quorum_unanimous() {
        let rule = QuorumRule::Unanimous;
        let voters = vec!["a".into(), "b".into()];

        let votes = vec![Vote::approve("a"), Vote::approve("b")];
        assert!(rule.is_met(&votes, &voters).met);

        let votes = vec![Vote::approve("a")];
        assert!(!rule.is_met(&votes, &voters).met);
    }

    #[test]
    fn quorum_threshold() {
        let rule = QuorumRule::Threshold(0.75);
        let voters: Vec<String> = (0..4).map(|i| format!("v{i}")).collect();

        // 3/4 = 75% >= 75%
        let votes = vec![Vote::approve("v0"), Vote::approve("v1"), Vote::approve("v2")];
        assert!(rule.is_met(&votes, &voters).met);

        // 2/4 = 50% < 75%
        let votes = vec![Vote::approve("v0"), Vote::approve("v1")];
        assert!(!rule.is_met(&votes, &voters).met);
    }

    #[test]
    fn quorum_weighted() {
        let mut weights = BTreeMap::new();
        weights.insert("lead".to_string(), 3.0);
        weights.insert("dev1".to_string(), 1.0);
        weights.insert("dev2".to_string(), 1.0);
        let rule = QuorumRule::Weighted { weights, threshold: 0.6 };
        let voters = vec!["lead".into(), "dev1".into(), "dev2".into()];

        // lead approves: 3/5 = 60% >= 60%
        let votes = vec![Vote::approve("lead")];
        assert!(rule.is_met(&votes, &voters).met);

        // dev1 approves: 1/5 = 20% < 60%
        let votes = vec![Vote::approve("dev1")];
        assert!(!rule.is_met(&votes, &voters).met);
    }

    #[test]
    fn quorum_empty_voters() {
        let rule = QuorumRule::Majority;
        let result = rule.is_met(&[], &[]);
        assert!(!result.met);
    }

    #[test]
    fn quorum_rule_display() {
        assert_eq!(format!("{}", QuorumRule::Majority), "majority (>50%)");
        assert_eq!(format!("{}", QuorumRule::Unanimous), "unanimous");
    }

    // ── ConsensusEngine: Full Cycle ──

    #[test]
    fn full_propose_vote_resolve_integrate_cycle() {
        let mut engine = setup_engine();
        let proposal = make_proposal();

        // Phase 1: propose.
        let id = engine.propose(proposal).unwrap();
        assert_eq!(engine.get_proposal(&id).unwrap().status, ProposalStatus::Voting);

        // Phase 2: vote.
        engine.vote(&id, Vote::approve("alice")).unwrap();
        engine.vote(&id, Vote::approve("bob")).unwrap();
        engine.vote(&id, Vote::reject("carol", "not convinced")).unwrap();

        // Phase 3: resolve (2/3 > 50% => accepted).
        let accepted = engine.resolve(&id).unwrap();
        assert!(accepted);
        assert_eq!(engine.get_proposal(&id).unwrap().status, ProposalStatus::Accepted);

        // Phase 4: integrate.
        engine.integrate(&id).unwrap();
        assert_eq!(engine.get_proposal(&id).unwrap().status, ProposalStatus::Integrated);
    }

    #[test]
    fn proposal_rejected() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();

        engine.vote(&id, Vote::approve("alice")).unwrap();
        engine.vote(&id, Vote::reject("bob", "no")).unwrap();
        engine.vote(&id, Vote::reject("carol", "no")).unwrap();

        let accepted = engine.resolve(&id).unwrap();
        assert!(!accepted);
        assert!(matches!(
            engine.get_proposal(&id).unwrap().status,
            ProposalStatus::Rejected(_)
        ));
    }

    #[test]
    fn integrate_rejected_fails() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();

        engine.vote(&id, Vote::reject("alice", "no")).unwrap();
        engine.resolve(&id).unwrap();

        let err = engine.integrate(&id).unwrap_err();
        assert!(matches!(err, ConsensusError::ProposalNotAccepted(_)));
    }

    // ── Error Cases ──

    #[test]
    fn vote_on_nonexistent_proposal() {
        let mut engine = setup_engine();
        let id = ProposalId::new("nonexistent");
        let err = engine.vote(&id, Vote::approve("alice")).unwrap_err();
        assert!(matches!(err, ConsensusError::ProposalNotFound(_)));
    }

    #[test]
    fn vote_from_ineligible_voter() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();
        let err = engine.vote(&id, Vote::approve("dave")).unwrap_err();
        assert!(matches!(err, ConsensusError::VoterNotEligible(_)));
    }

    #[test]
    fn duplicate_vote() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();
        engine.vote(&id, Vote::approve("alice")).unwrap();
        let err = engine.vote(&id, Vote::reject("alice", "changed mind")).unwrap_err();
        assert!(matches!(err, ConsensusError::DuplicateVote(_)));
    }

    #[test]
    fn resolve_nonexistent_proposal() {
        let mut engine = setup_engine();
        let id = ProposalId::new("ghost");
        let err = engine.resolve(&id).unwrap_err();
        assert!(matches!(err, ConsensusError::ProposalNotFound(_)));
    }

    // ── Withdraw ──

    #[test]
    fn withdraw_proposal() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();
        engine.withdraw(&id, "alice").unwrap();
        assert_eq!(engine.get_proposal(&id).unwrap().status, ProposalStatus::Withdrawn);
    }

    #[test]
    fn withdraw_resolved_fails() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();
        engine.vote(&id, Vote::approve("alice")).unwrap();
        engine.vote(&id, Vote::approve("bob")).unwrap();
        engine.resolve(&id).unwrap();
        let err = engine.withdraw(&id, "alice").unwrap_err();
        assert!(matches!(err, ConsensusError::AlreadyResolved(_)));
    }

    // ── Check Quorum (non-destructive) ──

    #[test]
    fn check_quorum_without_resolving() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();
        engine.vote(&id, Vote::approve("alice")).unwrap();

        let result = engine.check_quorum(&id).unwrap();
        assert_eq!(result.approve_count, 1);
        assert_eq!(result.total_eligible, 3);
        // Not yet met (1/3 = 33%).
        assert!(!result.met);
        // Proposal still in voting.
        assert_eq!(engine.get_proposal(&id).unwrap().status, ProposalStatus::Voting);
    }

    // ── Queries ──

    #[test]
    fn active_proposals() {
        let mut engine = setup_engine();
        engine.propose(make_proposal()).unwrap();
        let p2 = Proposal::new("p2", "bob", "rename type", ChangeKind::Rename);
        engine.propose(p2).unwrap();
        assert_eq!(engine.active_proposals().len(), 2);
    }

    #[test]
    fn count_by_status() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();
        assert_eq!(engine.count_by_status(&ProposalStatus::Voting), 1);

        engine.vote(&id, Vote::approve("alice")).unwrap();
        engine.vote(&id, Vote::approve("bob")).unwrap();
        engine.resolve(&id).unwrap();
        assert_eq!(engine.count_by_status(&ProposalStatus::Accepted), 1);
        assert_eq!(engine.count_by_status(&ProposalStatus::Voting), 0);
    }

    // ── History / Audit ──

    #[test]
    fn history_records_events() {
        let mut engine = setup_engine();
        let id = engine.propose(make_proposal()).unwrap();
        engine.vote(&id, Vote::approve("alice")).unwrap();
        engine.vote(&id, Vote::approve("bob")).unwrap();
        engine.resolve(&id).unwrap();
        engine.integrate(&id).unwrap();

        let history = engine.history();
        assert_eq!(history.len(), 5); // propose + 2 votes + resolve + integrate
        assert_eq!(history[0].event_kind, EventKind::Proposed);
        assert!(matches!(history[1].event_kind, EventKind::VoteCast(VoteDecision::Approve)));
        assert_eq!(history[3].event_kind, EventKind::Resolved(true));
        assert_eq!(history[4].event_kind, EventKind::Integrated);
    }

    // ── Voter Management ──

    #[test]
    fn register_unregister_voter() {
        let mut engine = ConsensusEngine::new(QuorumRule::Majority);
        engine.register_voter("alice");
        assert_eq!(engine.voter_count(), 1);
        engine.unregister_voter("alice");
        assert_eq!(engine.voter_count(), 0);
    }

    // ── Change Quorum Rule ──

    #[test]
    fn change_quorum_rule_between_proposals() {
        let mut engine = setup_engine();

        // First proposal with majority.
        let id1 = engine.propose(make_proposal()).unwrap();
        engine.vote(&id1, Vote::approve("alice")).unwrap();
        engine.vote(&id1, Vote::approve("bob")).unwrap();
        assert!(engine.resolve(&id1).unwrap());

        // Switch to unanimous.
        engine.set_quorum_rule(QuorumRule::Unanimous);

        let p2 = Proposal::new("p2", "bob", "big change", ChangeKind::Restructure);
        let id2 = engine.propose(p2).unwrap();
        engine.vote(&id2, Vote::approve("alice")).unwrap();
        engine.vote(&id2, Vote::approve("bob")).unwrap();
        // 2/3 is not unanimous.
        assert!(!engine.resolve(&id2).unwrap());
    }

    // ── ConsensusError Display ──

    #[test]
    fn error_display() {
        let e = ConsensusError::ProposalNotFound("x".to_string());
        assert!(format!("{e}").contains("not found"));
        let e = ConsensusError::DuplicateVote("alice".to_string());
        assert!(format!("{e}").contains("duplicate"));
    }

    // ── EventKind Display ──

    #[test]
    fn event_kind_display() {
        assert_eq!(format!("{}", EventKind::Proposed), "proposed");
        assert_eq!(format!("{}", EventKind::Integrated), "integrated");
        assert_eq!(format!("{}", EventKind::Resolved(true)), "resolved:accepted");
        assert_eq!(format!("{}", EventKind::Resolved(false)), "resolved:rejected");
    }

    // ── Full Scenario with Weighted Quorum ──

    #[test]
    fn full_weighted_consensus() {
        let mut weights = BTreeMap::new();
        weights.insert("lead".to_string(), 5.0);
        weights.insert("dev1".to_string(), 1.0);
        weights.insert("dev2".to_string(), 1.0);
        weights.insert("dev3".to_string(), 1.0);

        let mut engine = ConsensusEngine::new(QuorumRule::Weighted {
            weights,
            threshold: 0.6,
        });
        engine.register_voter("lead");
        engine.register_voter("dev1");
        engine.register_voter("dev2");
        engine.register_voter("dev3");

        let proposal = Proposal::new("wp1", "lead", "redesign API", ChangeKind::ModifyPublicApi);
        let id = engine.propose(proposal).unwrap();

        // Lead alone approves: 5/8 = 62.5% >= 60%.
        engine.vote(&id, Vote::approve("lead")).unwrap();
        assert!(engine.resolve(&id).unwrap());
        engine.integrate(&id).unwrap();
    }
}
