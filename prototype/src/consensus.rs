// ── Consensus Protocol ─────────────────────────────────────────────
//
// Five-phase consensus for shared interface changes among multiple agents.
//
//   1. Propose        – an agent proposes a change
//   2. Impact Analysis – compute affected regions & dependents
//   3. Vote           – relevant agents vote Accept / Reject / Abstain
//   4. Resolve        – tally votes, decide outcome
//   5. Integrate      – apply or discard the change
//
// Quorum requirement: majority of non-abstaining voters must accept.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ── IDs ────────────────────────────────────────────────────────────

pub type AgentId = String;
pub type ProposalId = u64;

// ── Proposal ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub id: ProposalId,
    pub proposer: AgentId,
    pub description: String,
    /// Regions affected by this proposal.
    pub affected_regions: Vec<String>,
    /// Source diff or new source text.
    pub change_payload: String,
}

// ── Phase ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Propose,
    ImpactAnalysis,
    Vote,
    Resolve,
    Integrate,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Phase::Propose => write!(f, "Propose"),
            Phase::ImpactAnalysis => write!(f, "ImpactAnalysis"),
            Phase::Vote => write!(f, "Vote"),
            Phase::Resolve => write!(f, "Resolve"),
            Phase::Integrate => write!(f, "Integrate"),
        }
    }
}

// ── Vote ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vote {
    Accept,
    Reject,
    Abstain,
}

impl fmt::Display for Vote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Vote::Accept => write!(f, "Accept"),
            Vote::Reject => write!(f, "Reject"),
            Vote::Abstain => write!(f, "Abstain"),
        }
    }
}

// ── Impact Report ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ImpactReport {
    pub affected_agents: BTreeSet<AgentId>,
    pub affected_regions: Vec<String>,
    pub breaking: bool,
    pub summary: String,
}

// ── Decision ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Accepted,
    Rejected,
    NoQuorum,
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Decision::Accepted => write!(f, "Accepted"),
            Decision::Rejected => write!(f, "Rejected"),
            Decision::NoQuorum => write!(f, "NoQuorum"),
        }
    }
}

// ── Errors ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsensusError {
    WrongPhase { expected: String, actual: String },
    ProposalNotFound(ProposalId),
    AlreadyVoted(AgentId),
    NotAVoter(AgentId),
    AlreadyResolved(ProposalId),
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConsensusError::WrongPhase { expected, actual } =>
                write!(f, "wrong phase: expected {expected}, in {actual}"),
            ConsensusError::ProposalNotFound(id) => write!(f, "proposal {id} not found"),
            ConsensusError::AlreadyVoted(agent) => write!(f, "agent {agent} already voted"),
            ConsensusError::NotAVoter(agent) => write!(f, "agent {agent} is not a voter"),
            ConsensusError::AlreadyResolved(id) => write!(f, "proposal {id} already resolved"),
        }
    }
}

// ── Consensus Round ────────────────────────────────────────────────

/// Tracks a single consensus round for one proposal.
pub struct ConsensusRound {
    pub proposal: Proposal,
    pub phase: Phase,
    pub impact: Option<ImpactReport>,
    /// Set of agents eligible to vote (determined during impact analysis).
    pub voters: BTreeSet<AgentId>,
    /// Votes cast so far.
    pub votes: BTreeMap<AgentId, Vote>,
    pub decision: Option<Decision>,
    pub integrated: bool,
}

impl ConsensusRound {
    fn new(proposal: Proposal) -> Self {
        Self {
            proposal,
            phase: Phase::Propose,
            impact: None,
            voters: BTreeSet::new(),
            votes: BTreeMap::new(),
            decision: None,
            integrated: false,
        }
    }
}

// ── Consensus Engine ───────────────────────────────────────────────

pub struct ConsensusEngine {
    next_id: ProposalId,
    rounds: BTreeMap<ProposalId, ConsensusRound>,
}

impl ConsensusEngine {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            rounds: BTreeMap::new(),
        }
    }

    /// Phase 1: Submit a proposal. Returns the proposal ID.
    pub fn propose(
        &mut self,
        proposer: AgentId,
        description: String,
        affected_regions: Vec<String>,
        change_payload: String,
    ) -> ProposalId {
        let id = self.next_id;
        self.next_id += 1;
        let proposal = Proposal {
            id,
            proposer,
            description,
            affected_regions,
            change_payload,
        };
        self.rounds.insert(id, ConsensusRound::new(proposal));
        id
    }

    /// Phase 2: Submit the impact analysis result. Advances to Vote phase.
    pub fn submit_impact(
        &mut self,
        proposal_id: ProposalId,
        report: ImpactReport,
    ) -> Result<(), ConsensusError> {
        let round = self.get_round_mut(proposal_id)?;
        if round.phase != Phase::Propose {
            return Err(ConsensusError::WrongPhase {
                expected: "Propose".into(),
                actual: round.phase.to_string(),
            });
        }
        round.voters = report.affected_agents.clone();
        // The proposer is always a voter.
        round.voters.insert(round.proposal.proposer.clone());
        round.impact = Some(report);
        round.phase = Phase::ImpactAnalysis;
        // Immediately advance to Vote.
        round.phase = Phase::Vote;
        Ok(())
    }

    /// Phase 3: Cast a vote.
    pub fn cast_vote(
        &mut self,
        proposal_id: ProposalId,
        agent: AgentId,
        vote: Vote,
    ) -> Result<(), ConsensusError> {
        let round = self.get_round_mut(proposal_id)?;
        if round.phase != Phase::Vote {
            return Err(ConsensusError::WrongPhase {
                expected: "Vote".into(),
                actual: round.phase.to_string(),
            });
        }
        if !round.voters.contains(&agent) {
            return Err(ConsensusError::NotAVoter(agent));
        }
        if round.votes.contains_key(&agent) {
            return Err(ConsensusError::AlreadyVoted(agent));
        }
        round.votes.insert(agent, vote);
        Ok(())
    }

    /// Phase 4: Resolve — tally votes and decide.
    /// Quorum: majority of non-abstaining voters must accept.
    pub fn resolve(&mut self, proposal_id: ProposalId) -> Result<Decision, ConsensusError> {
        let round = self.get_round_mut(proposal_id)?;
        if round.phase != Phase::Vote {
            return Err(ConsensusError::WrongPhase {
                expected: "Vote".into(),
                actual: round.phase.to_string(),
            });
        }
        if round.decision.is_some() {
            return Err(ConsensusError::AlreadyResolved(proposal_id));
        }

        let mut accepts = 0u32;
        let mut rejects = 0u32;
        for vote in round.votes.values() {
            match vote {
                Vote::Accept => accepts += 1,
                Vote::Reject => rejects += 1,
                Vote::Abstain => {}
            }
        }

        let decision = if accepts + rejects == 0 {
            Decision::NoQuorum
        } else if accepts > rejects {
            Decision::Accepted
        } else {
            Decision::Rejected
        };

        round.decision = Some(decision.clone());
        round.phase = Phase::Resolve;
        Ok(decision)
    }

    /// Phase 5: Integrate — mark the round as integrated (after applying the change).
    pub fn integrate(&mut self, proposal_id: ProposalId) -> Result<(), ConsensusError> {
        let round = self.get_round_mut(proposal_id)?;
        if round.phase != Phase::Resolve {
            return Err(ConsensusError::WrongPhase {
                expected: "Resolve".into(),
                actual: round.phase.to_string(),
            });
        }
        round.integrated = true;
        round.phase = Phase::Integrate;
        Ok(())
    }

    /// Get current phase of a proposal.
    pub fn phase(&self, proposal_id: ProposalId) -> Result<Phase, ConsensusError> {
        Ok(self.get_round(proposal_id)?.phase)
    }

    /// Get the decision for a resolved proposal.
    pub fn decision(&self, proposal_id: ProposalId) -> Result<Option<&Decision>, ConsensusError> {
        Ok(self.get_round(proposal_id)?.decision.as_ref())
    }

    /// List all active (non-integrated) proposals.
    pub fn active_proposals(&self) -> Vec<&Proposal> {
        self.rounds.values()
            .filter(|r| !r.integrated)
            .map(|r| &r.proposal)
            .collect()
    }

    /// JSON snapshot.
    pub fn to_json(&self) -> String {
        let mut entries = Vec::new();
        for r in self.rounds.values() {
            let decision_str = match &r.decision {
                Some(d) => format!("\"{}\"", d),
                None => "null".into(),
            };
            entries.push(format!(
                "{{\"id\":{},\"proposer\":\"{}\",\"phase\":\"{}\",\"decision\":{},\"votes\":{},\"integrated\":{}}}",
                r.proposal.id, r.proposal.proposer, r.phase, decision_str,
                r.votes.len(), r.integrated
            ));
        }
        format!("[{}]", entries.join(","))
    }

    // ── Internal ──────────────────────────────────────────────────

    fn get_round(&self, id: ProposalId) -> Result<&ConsensusRound, ConsensusError> {
        self.rounds.get(&id).ok_or(ConsensusError::ProposalNotFound(id))
    }

    fn get_round_mut(&mut self, id: ProposalId) -> Result<&mut ConsensusRound, ConsensusError> {
        self.rounds.get_mut(&id).ok_or(ConsensusError::ProposalNotFound(id))
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> ConsensusEngine {
        ConsensusEngine::new()
    }

    fn impact(agents: &[&str], breaking: bool) -> ImpactReport {
        ImpactReport {
            affected_agents: agents.iter().map(|a| a.to_string()).collect(),
            affected_regions: vec!["crate::api".into()],
            breaking,
            summary: "test impact".into(),
        }
    }

    // ── Full happy path ───────────────────────────────────────────

    #[test]
    fn full_five_phase_accepted() {
        let mut e = engine();
        let id = e.propose("alice".into(), "add method".into(), vec!["crate::api".into()], "f new_method() {}".into());
        assert_eq!(e.phase(id).unwrap(), Phase::Propose);

        e.submit_impact(id, impact(&["bob", "carol"], false)).unwrap();
        assert_eq!(e.phase(id).unwrap(), Phase::Vote);

        e.cast_vote(id, "alice".into(), Vote::Accept).unwrap();
        e.cast_vote(id, "bob".into(), Vote::Accept).unwrap();
        e.cast_vote(id, "carol".into(), Vote::Abstain).unwrap();

        let decision = e.resolve(id).unwrap();
        assert_eq!(decision, Decision::Accepted);

        e.integrate(id).unwrap();
        assert_eq!(e.phase(id).unwrap(), Phase::Integrate);
        assert_eq!(e.active_proposals().len(), 0);
    }

    #[test]
    fn full_five_phase_rejected() {
        let mut e = engine();
        let id = e.propose("alice".into(), "break api".into(), vec![], "...".into());
        e.submit_impact(id, impact(&["bob"], true)).unwrap();

        e.cast_vote(id, "alice".into(), Vote::Accept).unwrap();
        e.cast_vote(id, "bob".into(), Vote::Reject).unwrap();

        let decision = e.resolve(id).unwrap();
        assert_eq!(decision, Decision::Rejected);
    }

    // ── No quorum ─────────────────────────────────────────────────

    #[test]
    fn all_abstain_no_quorum() {
        let mut e = engine();
        let id = e.propose("alice".into(), "minor".into(), vec![], "".into());
        e.submit_impact(id, impact(&["bob"], false)).unwrap();
        e.cast_vote(id, "alice".into(), Vote::Abstain).unwrap();
        e.cast_vote(id, "bob".into(), Vote::Abstain).unwrap();
        let decision = e.resolve(id).unwrap();
        assert_eq!(decision, Decision::NoQuorum);
    }

    // ── Phase enforcement ─────────────────────────────────────────

    #[test]
    fn vote_before_impact_fails() {
        let mut e = engine();
        let id = e.propose("a".into(), "x".into(), vec![], "".into());
        let err = e.cast_vote(id, "a".into(), Vote::Accept).unwrap_err();
        assert!(matches!(err, ConsensusError::WrongPhase { .. }));
    }

    #[test]
    fn resolve_before_vote_fails() {
        let mut e = engine();
        let id = e.propose("a".into(), "x".into(), vec![], "".into());
        let err = e.resolve(id).unwrap_err();
        assert!(matches!(err, ConsensusError::WrongPhase { .. }));
    }

    #[test]
    fn integrate_before_resolve_fails() {
        let mut e = engine();
        let id = e.propose("a".into(), "x".into(), vec![], "".into());
        e.submit_impact(id, impact(&[], false)).unwrap();
        let err = e.integrate(id).unwrap_err();
        assert!(matches!(err, ConsensusError::WrongPhase { .. }));
    }

    // ── Duplicate vote ────────────────────────────────────────────

    #[test]
    fn double_vote_fails() {
        let mut e = engine();
        let id = e.propose("alice".into(), "x".into(), vec![], "".into());
        e.submit_impact(id, impact(&["bob"], false)).unwrap();
        e.cast_vote(id, "alice".into(), Vote::Accept).unwrap();
        let err = e.cast_vote(id, "alice".into(), Vote::Reject).unwrap_err();
        assert!(matches!(err, ConsensusError::AlreadyVoted(_)));
    }

    // ── Non-voter rejected ────────────────────────────────────────

    #[test]
    fn non_voter_rejected() {
        let mut e = engine();
        let id = e.propose("alice".into(), "x".into(), vec![], "".into());
        e.submit_impact(id, impact(&[], false)).unwrap();
        let err = e.cast_vote(id, "eve".into(), Vote::Accept).unwrap_err();
        assert!(matches!(err, ConsensusError::NotAVoter(_)));
    }

    // ── Proposal not found ────────────────────────────────────────

    #[test]
    fn missing_proposal() {
        let e = engine();
        let err = e.phase(999).unwrap_err();
        assert!(matches!(err, ConsensusError::ProposalNotFound(999)));
    }

    // ── Proposer is always a voter ────────────────────────────────

    #[test]
    fn proposer_always_voter() {
        let mut e = engine();
        let id = e.propose("alice".into(), "x".into(), vec![], "".into());
        // Impact with no other agents.
        e.submit_impact(id, impact(&[], false)).unwrap();
        // Alice can vote because she's the proposer.
        e.cast_vote(id, "alice".into(), Vote::Accept).unwrap();
        let decision = e.resolve(id).unwrap();
        assert_eq!(decision, Decision::Accepted);
    }

    // ── JSON ──────────────────────────────────────────────────────

    #[test]
    fn json_output() {
        let mut e = engine();
        e.propose("a".into(), "test".into(), vec![], "".into());
        let json = e.to_json();
        assert!(json.contains("\"proposer\":\"a\""));
        assert!(json.contains("\"phase\":\"Propose\""));
    }

    // ── Multiple proposals ────────────────────────────────────────

    #[test]
    fn multiple_proposals_independent() {
        let mut e = engine();
        let id1 = e.propose("a".into(), "first".into(), vec![], "".into());
        let id2 = e.propose("b".into(), "second".into(), vec![], "".into());
        assert_ne!(id1, id2);
        assert_eq!(e.active_proposals().len(), 2);
    }

    // ── Already resolved ──────────────────────────────────────────

    #[test]
    fn double_resolve_fails() {
        let mut e = engine();
        let id = e.propose("a".into(), "x".into(), vec![], "".into());
        e.submit_impact(id, impact(&[], false)).unwrap();
        e.cast_vote(id, "a".into(), Vote::Accept).unwrap();
        e.resolve(id).unwrap();
        // Phase is now Resolve, not Vote — so resolve again fails with WrongPhase.
        let err = e.resolve(id).unwrap_err();
        assert!(matches!(err, ConsensusError::WrongPhase { .. }));
    }
}
