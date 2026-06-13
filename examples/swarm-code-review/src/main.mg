// swarm-code-review — Multi-Agent Code Review System.
//
// A team of specialized AI agents reviews a pull request: an Architect
// checks design, a Security agent audits for vulnerabilities, a
// Performance agent profiles hot paths, and a Style agent enforces
// conventions. Each agent files findings, then they vote on whether
// to approve, request changes, or reject. Past decisions are cached
// in a memory store for future reference.
//
// Demonstrates:
//   - Agent roles with distinct expertise
//   - Swarm coordination patterns (scatter/gather)
//   - Consensus voting with quorum rules
//   - Memory recall API for review history
//   - Effect annotations (/ io, / db)
//   - Pattern matching on enums
//   - Contract specs on review functions

use std::col;
use std::fmt;
use std::io;

// ─────────────────────────────────────────────────────────────────────
// §1 — Code change representation
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data FileDiff {
    path: String,
    additions: u32,
    deletions: u32,
    hunks: [String]~,
}

#[derive(Debug, Clone)]
pub data PullRequest {
    id: u64,
    title: String,
    author: String,
    description: String,
    files: [FileDiff]~,
    labels: [String]~,
}

extend PullRequest {
    pub fn total_changes(&self) -> u32 {
        self.files.iter().map(|f| f.additions + f.deletions).sum()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn has_label(&self, label: &String) -> bool {
        self.labels.iter().any(|l| l == label)
    }
}

// ─────────────────────────────────────────────────────────────────────
// §2 — Review agent roles
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub data ReviewRole {
    Architect,
    Security,
    Performance,
    Style,
    Testing,
}

extend ReviewRole {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReviewRole::Architect   => write!(f, "🏗  Architect"),
            ReviewRole::Security    => write!(f, "🔒 Security"),
            ReviewRole::Performance => write!(f, "⚡ Performance"),
            ReviewRole::Style       => write!(f, "🎨 Style"),
            ReviewRole::Testing     => write!(f, "🧪 Testing"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// §3 — Findings: what each agent discovers
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub data Severity {
    Info,
    Warning,
    Error,
    Critical,
}

extend Severity {
    fn weight(&self) -> u32 {
        match self {
            Severity::Info     => 1,
            Severity::Warning  => 3,
            Severity::Error    => 8,
            Severity::Critical => 20,
        }
    }
}

extend Severity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Severity::Info     => write!(f, "INFO"),
            Severity::Warning  => write!(f, "WARN"),
            Severity::Error    => write!(f, "ERROR"),
            Severity::Critical => write!(f, "CRIT"),
        }
    }
}

#[derive(Debug, Clone)]
pub data Finding {
    role: ReviewRole,
    severity: Severity,
    file: String,
    line: ?u32,
    message: String,
    suggestion: ?String,
}

extend Finding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        val loc = match self.line {
            Some(l) => format!("{}:{}", self.file, l),
            None => self.file.clone(),
        };
        write!(f, "[{sev}] {role} @ {loc}: {msg}",
            sev = self.severity,
            role = self.role,
            loc = loc,
            msg = self.message)
    }
}

// ─────────────────────────────────────────────────────────────────────
// §4 — Specialized review agents
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub data ReviewAgent {
    id: u64,
    role: ReviewRole,
    findings: [Finding]~,
}

extend ReviewAgent {
    pub fn new(id: u64, role: ReviewRole) -> ReviewAgent {
        ReviewAgent { id: id, role: role, findings: []~.new() }
    }

    pub fn add_finding(&mut self, severity: Severity, file: String, line: ?u32, msg: String, suggestion: ?String) {
        self.findings.push(Finding {
            role: self.role.clone(),
            severity: severity,
            file: file,
            line: line,
            message: msg,
            suggestion: suggestion,
        });
    }

    pub fn risk_score(&self) -> u32 {
        self.findings.iter().map(|f| f.severity.weight()).sum()
    }

    /// Perform the review. Each role focuses on different aspects.
    ///
    /// @req  pr.file_count() > 0     "must have files to review"
    /// @ens  self.findings.len() >= 0
    pub fn review(&mut self, pr: &PullRequest) / io {
        println!("  {} reviewing {} files...", self.role, pr.file_count());

        match self.role {
            ReviewRole::Architect   => self.review_architecture(pr),
            ReviewRole::Security    => self.review_security(pr),
            ReviewRole::Performance => self.review_performance(pr),
            ReviewRole::Style       => self.review_style(pr),
            ReviewRole::Testing     => self.review_testing(pr),
        };

        println!("    → Found {} issue(s), risk score: {}", self.findings.len(), self.risk_score());
    }

    fn review_architecture(&mut self, pr: &PullRequest) {
        // Check for overly large changes.
        if pr.total_changes() > 500 {
            self.add_finding(
                Severity::Warning,
                "overall".to_string(),
                None,
                "PR is very large — consider splitting into smaller changes".to_string(),
                Some("Break into feature-flag-gated incremental PRs".to_string()),
            );
        }

        // Check for new public API surface.
        for file in &pr.files {
            for hunk in &file.hunks {
                if hunk.contains("pub fn ") || hunk.contains("pub struct ") || hunk.contains("pub trait ") {
                    self.add_finding(
                        Severity::Info,
                        file.path.clone(),
                        None,
                        "New public API surface detected".to_string(),
                        Some("Ensure backward compatibility".to_string()),
                    );
                }
            }
        }
    }

    fn review_security(&mut self, pr: &PullRequest) {
        for file in &pr.files {
            for hunk in &file.hunks {
                // Check for unsafe blocks.
                if hunk.contains("unsafe fn ") || hunk.contains("unsafe") {
                    self.add_finding(
                        Severity::Error,
                        file.path.clone(),
                        None,
                        "Unsafe code detected — requires manual audit".to_string(),
                        Some("Add @safety annotation with justification".to_string()),
                    );
                }

                // Check for raw SQL.
                if hunk.contains("raw_sql") || hunk.contains("exec_raw") {
                    self.add_finding(
                        Severity::Critical,
                        file.path.clone(),
                        None,
                        "Raw SQL query — potential injection vulnerability".to_string(),
                        Some("Use parameterized queries via db effect".to_string()),
                    );
                }

                // Check for hardcoded secrets.
                if hunk.contains("password =") || hunk.contains("api_key =") {
                    self.add_finding(
                        Severity::Critical,
                        file.path.clone(),
                        None,
                        "Possible hardcoded credential".to_string(),
                        Some("Use env::get() or a secret vault".to_string()),
                    );
                }
            }
        }
    }

    fn review_performance(&mut self, pr: &PullRequest) {
        for file in &pr.files {
            // Flag files with many additions (potential hot paths).
            if file.additions > 100 {
                self.add_finding(
                    Severity::Info,
                    file.path.clone(),
                    None,
                    format!("Large addition ({} lines) — profile for hot paths", file.additions),
                    Some("Add @perf benchmark annotation".to_string()),
                );
            }

            for hunk in &file.hunks {
                // Detect nested loops.
                if hunk.contains("for ") && hunk.matches("for ").count() > 1 {
                    self.add_finding(
                        Severity::Warning,
                        file.path.clone(),
                        None,
                        "Nested loops detected — potential O(n²) complexity".to_string(),
                        Some("Consider using a hash-based lookup".to_string()),
                    );
                }
            }
        }
    }

    fn review_style(&mut self, pr: &PullRequest) {
        for file in &pr.files {
            // Check for non-idiomatic patterns.
            for hunk in &file.hunks {
                if hunk.contains("println!(") {
                    // println! is the standard macro in C-like syntax — no issue.
                }
                if hunk.contains("unwrap()") {
                    self.add_finding(
                        Severity::Warning,
                        file.path.clone(),
                        None,
                        "Bare `.unwrap()` — prefer pattern matching or `?` operator".to_string(),
                        None,
                    );
                }
            }
        }
    }

    fn review_testing(&mut self, pr: &PullRequest) {
        var has_test_file = false;
        for file in &pr.files {
            if file.path.contains("test") || file.path.contains("spec") {
                has_test_file = true;
            }
        }
        if !has_test_file {
            self.add_finding(
                Severity::Error,
                "overall".to_string(),
                None,
                "No test files modified — all new code needs tests".to_string(),
                Some("Add a test module with at least one test per public function".to_string()),
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// §5 — Consensus voting
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub data VoteDecision {
    Approve,
    RequestChanges,
    Reject,
}

extend VoteDecision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VoteDecision::Approve        => write!(f, "✅ Approve"),
            VoteDecision::RequestChanges => write!(f, "🔄 Request Changes"),
            VoteDecision::Reject         => write!(f, "❌ Reject"),
        }
    }
}

#[derive(Debug, Clone)]
pub data Vote {
    agent_id: u64,
    role: ReviewRole,
    decision: VoteDecision,
    reason: String,
}

/// Compute each agent's vote based on their findings.
///
/// @req  agent.findings is available
/// @ens  result.decision reflects worst severity found
/// @fx   pure
fn compute_vote(agent: &ReviewAgent) -> Vote {
    val critical = agent.findings.iter().any(|f| f.severity == Severity::Critical);
    val errors = agent.findings.iter().filter(|f| f.severity == Severity::Error).count();
    val score = agent.risk_score();

    val (decision, reason) = if critical {
        (VoteDecision::Reject, "Critical issues found — cannot merge".to_string())
    } else if errors > 2 {
        (VoteDecision::RequestChanges, format!("Found {} errors (risk score: {})", errors, score))
    } else if score > 15 {
        (VoteDecision::RequestChanges, format!("High risk score ({}) — needs attention", score))
    } else {
        (VoteDecision::Approve, format!("Looks good (risk score: {})", score))
    };

    Vote {
        agent_id: agent.id,
        role: agent.role.clone(),
        decision: decision,
        reason: reason,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub data ReviewOutcome {
    Approved,
    ChangesRequested,
    Rejected,
}

extend ReviewOutcome {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReviewOutcome::Approved         => write!(f, "✅ APPROVED"),
            ReviewOutcome::ChangesRequested => write!(f, "🔄 CHANGES REQUESTED"),
            ReviewOutcome::Rejected         => write!(f, "❌ REJECTED"),
        }
    }
}

/// Apply supermajority consensus: approve only if >2/3 approve
/// and no one rejects.
///
/// @req  votes.len() > 0                  "need at least one vote"
/// @ens  result is deterministic
/// @fx   pure
fn consensus(votes: &[Vote]~) -> ReviewOutcome {
    val total = votes.len();
    val approvals = votes.iter().filter(|v| v.decision == VoteDecision::Approve).count();
    val rejections = votes.iter().filter(|v| v.decision == VoteDecision::Reject).count();

    if rejections > 0 {
        ReviewOutcome::Rejected
    } else if approvals * 3 > total * 2 {
        ReviewOutcome::Approved
    } else {
        ReviewOutcome::ChangesRequested
    }
}

// ─────────────────────────────────────────────────────────────────────
// §6 — Review history (memory recall)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data ReviewRecord {
    pr_id: u64,
    pr_title: String,
    outcome: ReviewOutcome,
    finding_count: usize,
    total_risk: u32,
}

#[derive(Debug)]
pub data ReviewHistory {
    records: [ReviewRecord]~,
}

extend ReviewHistory {
    pub fn new() -> ReviewHistory {
        ReviewHistory { records: []~.new() }
    }

    pub fn record(&mut self, pr: &PullRequest, outcome: ReviewOutcome,
              findings: &[Finding]~, risk: u32) {
        self.records.push(ReviewRecord {
            pr_id: pr.id,
            pr_title: pr.title.clone(),
            outcome: outcome,
            finding_count: findings.len(),
            total_risk: risk,
        });
    }

    pub fn approval_rate(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        val approved = self.records.iter()
            .filter(|r| r.outcome == ReviewOutcome::Approved)
            .count();
        approved as f64 / self.records.len() as f64 * 100.0
    }

    pub fn avg_risk(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        val total: u32 = self.records.iter().map(|r| r.total_risk).sum();
        total as f64 / self.records.len() as f64
    }

    pub fn report(&self) / io {
        println!("");
        println!("── Review History ─────────────────────────────────────");
        println!("  Total reviews:  {}", self.records.len());
        println!("  Approval rate:  {:.1}%", self.approval_rate());
        println!("  Avg risk score: {:.1}", self.avg_risk());
        println!("  ┌─────┬──────────────────────────┬─────────────────┐");
        println!("  │ PR  │ Title                    │ Outcome         │");
        println!("  ├─────┼──────────────────────────┼─────────────────┤");
        for rec in &self.records {
            println!("  │ {:<3} │ {:<24} │ {:<15} │", rec.pr_id, rec.pr_title, rec.outcome);
        }
        println!("  └─────┴──────────────────────────┴─────────────────┘");
    }
}

// ─────────────────────────────────────────────────────────────────────
// §7 — Entry point: run a full code review
// ─────────────────────────────────────────────────────────────────────

pub fn main() / io {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║  MAGE Swarm Code Review System                          ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!("");

    // Simulate a pull request.
    val pr = PullRequest {
        id: 42,
        title: "Add user authentication".to_string(),
        author: "developer-a".to_string(),
        description: "Implements JWT-based auth with refresh tokens".to_string(),
        files: vec![
            FileDiff {
                path: "src/auth/mod.mg".to_string(),
                additions: 120,
                deletions: 5,
                hunks: vec![
                    "pub fn verify_token(token: &String) -> Claims or AuthError".to_string(),
                    "pub struct Claims { user_id: u64, exp: u64 }".to_string(),
                    "val api_key = \"sk-test-12345\"".to_string(),
                ],
            },
            FileDiff {
                path: "src/auth/jwt.mg".to_string(),
                additions: 85,
                deletions: 0,
                hunks: vec![
                    "pub fn sign(payload: &Claims, secret: &String) -> String".to_string(),
                    "unsafe fn decode_raw(bytes: &[u8]) -> Claims".to_string(),
                ],
            },
            FileDiff {
                path: "src/middleware.mg".to_string(),
                additions: 40,
                deletions: 10,
                hunks: vec![
                    "pub fn auth_middleware(req: &Request) / net + io -> () or AuthError".to_string(),
                ],
            },
        ],
        labels: vec!["feature", "auth"].iter().map(|s| s.to_string()).collect(),
    };

    println!("Pull Request #{}: {}", pr.id, pr.title);
    println!("  Author: {}", pr.author);
    println!("  Files:  {}", pr.file_count());
    println!("  Changes: +{} / -{}", pr.files.iter().map(|f| f.additions).sum::<u32>(), pr.files.iter().map(|f| f.deletions).sum::<u32>());
    println!("");

    // Create the review swarm.
    println!("─── Assembling Review Swarm ──────────────────────────────");
    var agents: [ReviewAgent]~ = vec![
        ReviewAgent.new(1, ReviewRole::Architect),
        ReviewAgent.new(2, ReviewRole::Security),
        ReviewAgent.new(3, ReviewRole::Performance),
        ReviewAgent.new(4, ReviewRole::Style),
        ReviewAgent.new(5, ReviewRole::Testing),
    ];

    // Each agent reviews independently (scatter phase).
    println!("");
    println!("─── Scatter: Independent Reviews ─────────────────────────");
    for agent in &mut agents {
        agent.review(&pr);
    }

    // Gather all findings.
    println!("");
    println!("─── Gather: Consolidated Findings ────────────────────────");
    var all_findings: [Finding]~ = []~.new();
    for agent in &agents {
        for finding in &agent.findings {
            println!("  {}", finding);
            all_findings.push(finding.clone());
        }
    }
    println!("");
    println!("  Total findings: {}", all_findings.len());

    // Vote on the PR.
    println!("");
    println!("─── Consensus: Voting ────────────────────────────────────");
    var votes: [Vote]~ = []~.new();
    for agent in &agents {
        val vote = compute_vote(agent);
        println!("  {}: {} — {}", vote.role, vote.decision, vote.reason);
        votes.push(vote);
    }

    val outcome = consensus(&votes);
    val total_risk: u32 = agents.iter().map(|a| a.risk_score()).sum();
    println!("");
    println!("  ╔═══════════════════════════════════╗");
    println!("  ║  Review Outcome: {:<21}║", outcome);
    println!("  ║  Total Risk Score: {:<19}║", total_risk);
    println!("  ╚═══════════════════════════════════╝");

    // Record in history.
    var history = ReviewHistory.new();
    history.record(&pr, outcome, &all_findings, total_risk);

    // Simulate a second clean PR.
    val clean_pr = PullRequest {
        id: 43,
        title: "Fix typo in README".to_string(),
        author: "developer-b".to_string(),
        description: "Minor documentation fix".to_string(),
        files: vec![
            FileDiff {
                path: "README.md".to_string(),
                additions: 1,
                deletions: 1,
                hunks: vec!["Fixed spelling of 'authentication'".to_string()],
            },
            FileDiff {
                path: "tests/readme_test.mg".to_string(),
                additions: 5,
                deletions: 0,
                hunks: vec!["added doc link test".to_string()],
            },
        ],
        labels: vec!["docs"].iter().map(|s| s.to_string()).collect(),
    };
    history.record(&clean_pr, ReviewOutcome::Approved, &[]~.new(), 1);

    history.report();

    println!("");
    println!("═══════════════════════════════════════════════════════════");
    println!("  Review system complete.");
    println!("═══════════════════════════════════════════════════════════");
}
