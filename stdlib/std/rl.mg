//! # std::rl — Reinforcement Learning
//!
//! First-class support for reinforcement learning: environments,
//! policies, trajectories, and standard RL algorithms (PPO, A3C, DQN).

// ---------------------------------------------------------------------------
// Environment trait
// ---------------------------------------------------------------------------

/// A reinforcement learning environment.
pub trait Env {
    /// Observation type.
    type Obs;
    /// Action type.
    type Act;

    /// Reset the environment and return the initial observation.
    pub fn reset(&mut self) -> Self::Obs;

    /// Take a step with an action, returning (observation, reward, done).
    pub fn step(&mut self, action: Self::Act) -> (Self::Obs, f64, bool);

    /// Get the observation space dimensions.
    pub fn observation_space(&self) -> usize;

    /// Get the action space dimensions.
    pub fn action_space(&self) -> usize;

    /// Collect a full trajectory using a policy.
    pub fn rollout<P: Policy<Self::Obs, Self::Act>>(
        &mut self, policy: &P
    ) -> Trajectory<Self::Obs, Self::Act>;
}

// ---------------------------------------------------------------------------
// Policy trait
// ---------------------------------------------------------------------------

/// A policy maps observations to actions.
pub trait Policy<Obs, Act> {
    /// Select an action given an observation.
    pub fn act(&self, obs: &Obs) -> Act;

    /// Select an action with exploration (during training).
    pub fn act_explore(&self, obs: &Obs) -> Act / rng;

    /// Compute the log-probability of an action given observation.
    pub fn log_prob(&self, obs: &Obs, act: &Act) -> f64;
}

// ---------------------------------------------------------------------------
// Trajectory
// ---------------------------------------------------------------------------

/// A sequence of (observation, action, reward) tuples from one episode.
pub struct Trajectory<Obs, Act> {
    pub observations: Vec<Obs>,
    pub actions: Vec<Act>,
    pub rewards: Vec<f64>,
    pub dones: Vec<bool>,
}

impl<Obs, Act> Trajectory<Obs, Act> {
    /// Total reward for this trajectory.
    pub fn total_reward(&self) -> f64;

    /// Discounted return with a given gamma.
    pub fn discounted_return(&self, gamma: f64) -> f64;

    /// Number of steps.
    pub fn len(&self) -> usize;
}

// ---------------------------------------------------------------------------
// PPO (Proximal Policy Optimization)
// ---------------------------------------------------------------------------

/// PPO algorithm configuration and state.
pub struct PPO {
    obs_dim: usize,
    act_dim: usize,
    hidden: usize,
    lr: f64,
    gamma: f64,
    clip_ratio: f64,
    epochs_per_update: usize,
}

impl PPO {
    /// Create a new PPO agent.
    pub fn new(
        obs_dim: usize,
        act_dim: usize,
        hidden: usize,
        lr: f64,
    ) -> PPO;

    /// Update the policy from a trajectory.
    pub fn update<Obs, Act>(&mut self, trajectory: &Trajectory<Obs, Act>) -> RLMetrics / gpu;

    /// Get the learned policy.
    pub fn policy<Obs, Act>(&self) -> impl Policy<Obs, Act>;
}

// ---------------------------------------------------------------------------
// A3C (Asynchronous Advantage Actor-Critic)
// ---------------------------------------------------------------------------

/// A3C algorithm configuration.
pub struct A3C {
    obs_dim: usize,
    act_dim: usize,
    hidden: usize,
    lr: f64,
    gamma: f64,
    num_workers: usize,
}

impl A3C {
    pub fn new(
        obs_dim: usize,
        act_dim: usize,
        hidden: usize,
        lr: f64,
        num_workers: usize,
    ) -> A3C;

    pub fn train<E: Env>(&mut self, env_factory: fn() -> E, episodes: usize) -> RLMetrics / gpu, async;
    pub fn policy<Obs, Act>(&self) -> impl Policy<Obs, Act>;
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

/// Training metrics from RL algorithms.
pub struct RLMetrics {
    pub mean_reward: f64,
    pub std_reward: f64,
    pub max_reward: f64,
    pub min_reward: f64,
    pub episodes: usize,
    pub policy_loss: f64,
    pub value_loss: f64,
}
