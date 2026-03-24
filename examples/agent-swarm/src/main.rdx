// agent-swarm — Multi-agent task coordination.
//
// Demonstrates:
//   - Agent primitives (AgentId, SwarmConfig, Task)
//   - Semantic leases and region ownership
//   - Swarm orchestration patterns (map-reduce, pipeline)
//   - Consensus protocols
//   - Async coordination (pub async fn)
//   - Effect annotations (/ io, / net)
//   - Enums for agent roles and task states
//   - Trait-based agent dispatch

use std::agent;
use std::sync;
use std::col;
use std::fmt;
use std::io;

// ── Agent roles ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Role {
    Orchestrator,
    Architect,
    Synthesizer,
    Reviewer,
    Integrator,
    Verifier,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Role::Orchestrator => write!(f, "Orchestrator"),
            Role::Architect => write!(f, "Architect"),
            Role::Synthesizer => write!(f, "Synthesizer"),
            Role::Reviewer => write!(f, "Reviewer"),
            Role::Integrator => write!(f, "Integrator"),
            Role::Verifier => write!(f, "Verifier"),
        }
    }
}

// ── Semantic regions ─────────────────────────────────────────────────

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum SemanticRegion {
    Function(String),
    Module(String),
    TraitDef(String),
    ImplBlock(String),
    TypeDef(String),
}

impl fmt::Display for SemanticRegion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SemanticRegion::Function(name) => write!(f, "fn {name}"),
            SemanticRegion::Module(name) => write!(f, "mod {name}"),
            SemanticRegion::TraitDef(name) => write!(f, "trait {name}"),
            SemanticRegion::ImplBlock(name) => write!(f, "impl {name}"),
            SemanticRegion::TypeDef(name) => write!(f, "type {name}"),
        }
    }
}

// ── Semantic leases ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LeaseKind {
    SharedRead,
    ExclusiveWrite,
    Restructuring,
}

#[derive(Debug, Clone)]
pub struct Lease {
    region: SemanticRegion,
    kind: LeaseKind,
    holder: u64,
    version: u64,
}

impl Lease {
    pub fn new(region: SemanticRegion, kind: LeaseKind, holder: u64) -> Lease {
        Lease { region: region, kind: kind, holder: holder, version: 0 }
    }

    pub fn is_write(&self) -> bool {
        match self.kind {
            LeaseKind::ExclusiveWrite | LeaseKind::Restructuring => true,
            _ => false,
        }
    }
}

// ── Task definitions ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Review,
    Complete,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Task {
    id: u64,
    description: String,
    region: SemanticRegion,
    assigned_to: Option<u64>,
    status: TaskStatus,
    dependencies: Vec<u64>,
}

impl Task {
    pub fn new(id: u64, description: String, region: SemanticRegion) -> Task {
        Task {
            id: id,
            description: description,
            region: region,
            assigned_to: None,
            status: TaskStatus::Pending,
            dependencies: Vec::new(),
        }
    }

    pub fn with_deps(id: u64, description: String, region: SemanticRegion, deps: Vec<u64>) -> Task {
        Task {
            id: id,
            description: description,
            region: region,
            assigned_to: None,
            status: TaskStatus::Pending,
            dependencies: deps,
        }
    }

    pub fn is_ready(&self, completed: &HashSet<u64>) -> bool {
        self.dependencies.iter().all(|d| completed.contains(d))
    }
}

// ── Agent definition ─────────────────────────────────────────────────

#[derive(Debug)]
pub struct Agent {
    id: u64,
    role: Role,
    active_lease: Option<Lease>,
    tasks_completed: u64,
}

impl Agent {
    pub fn new(id: u64, role: Role) -> Agent {
        Agent { id: id, role: role, active_lease: None, tasks_completed: 0 }
    }

    pub fn acquire_lease(&mut self, region: SemanticRegion, kind: LeaseKind) -> Lease {
        let lease = Lease::new(region, kind, self.id);
        self.active_lease = Some(lease.clone());
        println!("  Agent {self.id} ({self.role}) acquired {lease.kind:?} on {lease.region}");
        lease
    }

    pub fn release_lease(&mut self) {
        match &self.active_lease {
            Some(lease) => println!("  Agent {self.id} ({self.role}) released lease on {lease.region}"),
            None => {},
        }
        self.active_lease = None;
    }

    pub async fn execute_task(&mut self, task: &mut Task) -> Result<(), String> {
        println!("  Agent {self.id} ({self.role}) working on: {task.description}");
        task.assigned_to = Some(self.id);
        task.status = TaskStatus::InProgress;

        // Simulate work.
        async_rt::sleep(100).await;

        task.status = TaskStatus::Complete;
        self.tasks_completed = self.tasks_completed + 1;
        println!("  Agent {self.id} ({self.role}) completed: {task.description}");
        Ok(())
    }
}

// ── Swarm ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Swarm {
    agents: Vec<Agent>,
    tasks: Vec<Task>,
    completed_ids: HashSet<u64>,
}

impl Swarm {
    pub fn new() -> Swarm {
        Swarm {
            agents: Vec::new(),
            tasks: Vec::new(),
            completed_ids: HashSet::new(),
        }
    }

    pub fn add_agent(&mut self, agent: Agent) {
        println!("Swarm: added agent {agent.id} ({agent.role})");
        self.agents.push(agent);
    }

    pub fn add_task(&mut self, task: Task) {
        println!("Swarm: queued task {task.id} — {task.description}");
        self.tasks.push(task);
    }

    /// Run all tasks respecting dependency ordering.
    pub async fn run(&mut self) -> Result<(), String> {
        println!("");
        println!("=== Swarm Execution ===");
        println!("  Agents: {self.agents.len()}");
        println!("  Tasks:  {self.tasks.len()}");
        println!("");

        let mut remaining: usize = self.tasks.len();

        for _ in 0..self.tasks.len() * 2 {
            if remaining == 0 {
                return Ok(());
            }

            // Find ready tasks.
            let mut ready_indices: Vec<usize> = Vec::new();
            for (i, task) in self.tasks.iter().enumerate() {
                if task.is_ready(&self.completed_ids) {
                    match task.status {
                        TaskStatus::Pending => ready_indices.push(i),
                        _ => {},
                    }
                }
            }

            // Assign ready tasks to available agents.
            let mut agent_idx: usize = 0;
            for task_idx in &ready_indices {
                if agent_idx >= self.agents.len() {
                    return Ok(());  // No more agents available this round.
                }

                let agent = &mut self.agents[agent_idx];
                let task = &mut self.tasks[*task_idx];

                // Acquire lease.
                agent.acquire_lease(task.region.clone(), LeaseKind::ExclusiveWrite);

                // Execute.
                agent.execute_task(task).await?;

                // Release lease.
                agent.release_lease();

                self.completed_ids.insert(task.id);
                remaining = remaining - 1;
                agent_idx = agent_idx + 1;
            }
        }

        if remaining > 0 {
            Err(format!("deadlock: {remaining} tasks could not be scheduled"))
        } else {
            Ok(())
        }
    }

    pub fn summary(&self) {
        println!("");
        println!("=== Swarm Summary ===");
        for agent in &self.agents {
            println!("  Agent {agent.id} ({agent.role}): {agent.tasks_completed} tasks completed");
        }
        println!("  Total tasks: {self.completed_ids.len()}");
    }
}

// ── Entry point ──────────────────────────────────────────────────────

pub async fn main() -> Result<(), String> {
    // Create a swarm with specialized agents.
    let mut swarm = Swarm::new();

    swarm.add_agent(Agent::new(1, Role::Architect));
    swarm.add_agent(Agent::new(2, Role::Synthesizer));
    swarm.add_agent(Agent::new(3, Role::Synthesizer));
    swarm.add_agent(Agent::new(4, Role::Verifier));

    // Define tasks with dependencies.
    // Task 1: Design the API (no deps — Architect does this).
    swarm.add_task(Task::new(
        1,
        "Design user module API".to_string(),
        SemanticRegion::Module("user".to_string()),
    ));

    // Tasks 2-3: Implement functions (depend on task 1).
    swarm.add_task(Task::with_deps(
        2,
        "Implement create_user".to_string(),
        SemanticRegion::Function("create_user".to_string()),
        vec![1],
    ));
    swarm.add_task(Task::with_deps(
        3,
        "Implement validate_email".to_string(),
        SemanticRegion::Function("validate_email".to_string()),
        vec![1],
    ));

    // Task 4: Verify all implementations (depends on 2 and 3).
    swarm.add_task(Task::with_deps(
        4,
        "Verify user module contracts".to_string(),
        SemanticRegion::Module("user".to_string()),
        vec![2, 3],
    ));

    // Task 5: Integration test (depends on 4).
    swarm.add_task(Task::with_deps(
        5,
        "Run integration tests".to_string(),
        SemanticRegion::Module("user".to_string()),
        vec![4],
    ));

    // Execute the swarm.
    swarm.run().await?;
    swarm.summary();

    Ok(())
}
