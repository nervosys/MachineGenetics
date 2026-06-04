// framewerx::agentic::multi_agent — multi-agent coordination patterns.

// Debate: agents argue from assigned positions; judge picks winner.
S MultiAgentDebate {
    num_agents: usize,
    rounds: usize,
    judge_model: s,
}

I MultiAgentDebate {
    +f new(num_agents: usize, rounds: usize) -> MultiAgentDebate {
        @MultiAgentDebate { num_agents: num_agents, rounds: rounds, judge_model: "self" }
    }
}

// Constitutional AI: critic-revisor loop guided by a constitution.
S ConstitutionalAI {
    principles: [s]~,
    revision_iterations: usize,
}

// RLHF / RLAIF: reward model + PPO on the policy.
S RLHF {
    policy_model: s,
    reward_model: s,
    kl_coef: f32,
    clip_eps: f32,
}

S DPO {
    policy_model: s,
    reference_model: s,
    beta: f32,
}

// Hierarchical agents: planner agent delegates to worker agents.
S HierarchicalAgent {
    planner: s,
    workers: [s]~,
    delegation_depth: usize,
}

// Swarm orchestrator: shared workspace + role-based dispatch.
S SwarmOrchestrator {
    roles: [s]~,
    workspace: s,
    consensus: s,
}

// Skill library: reusable named skills the agent can compose.
S SkillLibrary {
    skills: [s]~,
    retrieval: s,
}

// World-model-based agent (Dreamer / MuZero style).
S WorldModelAgent {
    world_model: s,
    policy: s,
    rollout_horizon: usize,
}
