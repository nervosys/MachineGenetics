//! Self-healing: what to do when an action fails.
//!
//! "Self-healing" is usually a euphemism for "retries". Retrying is one of the
//! four strategies here, and the least interesting one.
//!
//! The useful framing is that a failure carries information about *which*
//! assumption broke, and each broken assumption has a different repair:
//!
//! | Failure | Broken assumption | Repair |
//! |---|---|---|
//! | [`ExecError::Transient`] | the infrastructure held still | retry, bounded |
//! | [`CasError::Corrupt`] | stored bytes stayed the bytes | evict and rebuild |
//! | [`ExecError::NoCapablePlatform`] | the fleet has this device | relax the pin, if sound |
//! | [`ExecError::Deterministic`] | the action was correct | escalate — the code is wrong |
//!
//! Only the last needs an agent. That is the point: a build system operated by
//! agents should consume agent attention only for failures that are actually
//! about the program. Everything else is infrastructure noise the system should
//! absorb silently and *report*, so the noise remains measurable.
//!
//! ## Relaxing a platform pin is the subtle one
//!
//! If a CUDA worker is unavailable, running the action on CPU is sound only when
//! the two produce identical bytes. For a *compilation* that is generally true —
//! lowering does not depend on the device. For a *kernel autotune* it is false by
//! definition: the output encodes device-specific choices.
//!
//! So the fallback is opt-in per action ([`Action`]s carry it via the
//! `ribosome.fallback` env key) rather than a global policy, and taking it
//! **changes the action, hence its key** — the CPU result is cached under the CPU
//! key and can never be served to something that asked for CUDA. A build system
//! that silently satisfied a GPU request from a CPU cache entry would be
//! returning wrong answers quickly, which is worse than being slow.

use super::cas::CasError;
use super::exec::ExecError;
use super::Action;
use serde::Serialize;

/// The env key an action sets to permit accelerator fallback.
pub const FALLBACK_KEY: &str = "ribosome.fallback";

/// What the healer decided to do about a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Remedy {
    /// Run the identical action again.
    Retry { attempt: u32, reason: String },
    /// Drop poisoned cache state, then run again.
    EvictAndRetry { attempt: u32, blob: String },
    /// Run a *modified* action — and therefore a different key.
    Substitute { attempt: u32, reason: String },
    /// Nothing mechanical will help. Hand it to an agent.
    Escalate { reason: String },
}

/// A healing attempt, recorded for the build report.
///
/// Recorded even when healing succeeds, because a build that quietly heals the
/// same action every run is a defect wearing a disguise — the record is what
/// makes it visible, and what a fitness function can penalize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealEvent {
    pub action: String,
    pub failure: String,
    pub remedy: Remedy,
}

/// Policy for how hard to try.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealPolicy {
    /// Attempts for a transient failure before escalating.
    pub max_transient_retries: u32,
    /// Attempts to rebuild through corrupt cache state.
    pub max_corruption_retries: u32,
    /// Whether accelerator fallback is permitted at all (per-action opt-in still
    /// required — this is the fleet-wide off switch).
    pub allow_platform_fallback: bool,
}

impl Default for HealPolicy {
    fn default() -> Self {
        HealPolicy {
            // Two retries covers the overwhelming majority of genuine blips
            // without turning a hard failure into a slow one.
            max_transient_retries: 2,
            // Corruption should be singular. Repeated corruption at the same
            // blob means the disk or the fleet is sick, and grinding on it
            // hides that.
            max_corruption_retries: 1,
            allow_platform_fallback: true,
        }
    }
}

/// Decides remedies. A trait so an agent-backed healer — one that proposes a
/// source fix via the compiler's existing 17 repair patterns
/// (`prototype/src/heal.rs`) — can be substituted for the mechanical one.
pub trait Healer: Send + Sync {
    fn on_exec_error(&self, action: &Action, err: &ExecError, attempt: u32) -> Remedy;
    fn on_cas_error(&self, action: &Action, err: &CasError, attempt: u32) -> Remedy;
}

/// The mechanical healer: infrastructure repairs only, escalating anything that
/// looks like a real defect.
pub struct DefaultHealer {
    pub policy: HealPolicy,
}

impl DefaultHealer {
    pub fn new(policy: HealPolicy) -> Self {
        DefaultHealer { policy }
    }
}

impl Default for DefaultHealer {
    fn default() -> Self {
        DefaultHealer::new(HealPolicy::default())
    }
}

impl Healer for DefaultHealer {
    fn on_exec_error(&self, action: &Action, err: &ExecError, attempt: u32) -> Remedy {
        match err {
            ExecError::Transient(msg) => {
                if attempt < self.policy.max_transient_retries {
                    Remedy::Retry { attempt: attempt + 1, reason: msg.clone() }
                } else {
                    Remedy::Escalate {
                        reason: format!(
                            "transient failure persisted across {} attempts: {msg}",
                            attempt + 1
                        ),
                    }
                }
            }

            ExecError::NoCapablePlatform(p) => {
                let opted_in = action.env.get(FALLBACK_KEY).map(|v| v == "1").unwrap_or(false);
                if self.policy.allow_platform_fallback && opted_in && p.accelerator.is_some() {
                    Remedy::Substitute {
                        attempt: attempt + 1,
                        reason: format!(
                            "no `{}` worker available; retrying device-independent",
                            p.accelerator.as_deref().unwrap_or("?")
                        ),
                    }
                } else {
                    Remedy::Escalate {
                        reason: format!(
                            "no worker satisfies `{}` and this action did not opt into fallback",
                            p.tag()
                        ),
                    }
                }
            }

            // A tool that is absent, an input that was not supplied, an output
            // that was not produced, a genuine non-zero exit: all statements
            // about the build definition or the code. Retrying is superstition.
            ExecError::ToolNotFound(t) => {
                Remedy::Escalate { reason: format!("tool `{t}` is not registered on any worker") }
            }
            ExecError::MissingInput(p) => {
                Remedy::Escalate { reason: format!("declared input `{p}` was never produced") }
            }
            ExecError::MissingOutput(p) => {
                Remedy::Escalate { reason: format!("action did not produce promised output `{p}`") }
            }
            ExecError::Deterministic { exit_code, stderr } => Remedy::Escalate {
                reason: format!("action failed deterministically (exit {exit_code}): {stderr}"),
            },
        }
    }

    fn on_cas_error(&self, _action: &Action, err: &CasError, attempt: u32) -> Remedy {
        match err {
            CasError::Corrupt { want, .. } => {
                if attempt < self.policy.max_corruption_retries {
                    Remedy::EvictAndRetry { attempt: attempt + 1, blob: want.0.clone() }
                } else {
                    Remedy::Escalate {
                        reason: format!(
                            "blob {} corrupt again after eviction — suspect the storage layer",
                            want.short()
                        ),
                    }
                }
            }
            // A missing blob is recoverable the same way: forget the claim that
            // it exists and rebuild what produced it.
            CasError::Missing(d) => {
                if attempt < self.policy.max_corruption_retries {
                    Remedy::EvictAndRetry { attempt: attempt + 1, blob: d.0.clone() }
                } else {
                    Remedy::Escalate {
                        reason: format!("blob {} still missing after rebuild", d.short()),
                    }
                }
            }
            CasError::Io(e) => Remedy::Escalate { reason: format!("storage io error: {e}") },
        }
    }
}

/// Apply a [`Remedy::Substitute`] to an action: drop the accelerator pin.
///
/// Returns a new action, so the caller re-keys it. That re-keying is the safety
/// property — see the module note.
pub fn relax_platform(action: &Action) -> Action {
    let mut a = action.clone();
    a.platform.accelerator = None;
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Digest, Platform};

    fn act() -> Action {
        Action::new("compile", "mage-parse@0.2.0")
    }

    #[test]
    fn transient_failures_retry_then_escalate() {
        let h = DefaultHealer::default();
        let e = ExecError::Transient("worker vanished".into());
        assert!(matches!(h.on_exec_error(&act(), &e, 0), Remedy::Retry { attempt: 1, .. }));
        assert!(matches!(h.on_exec_error(&act(), &e, 1), Remedy::Retry { attempt: 2, .. }));
        assert!(matches!(h.on_exec_error(&act(), &e, 2), Remedy::Escalate { .. }));
    }

    #[test]
    fn deterministic_failures_never_retry() {
        let h = DefaultHealer::default();
        let e = ExecError::Deterministic { exit_code: 1, stderr: "type error".into() };
        assert!(
            matches!(h.on_exec_error(&act(), &e, 0), Remedy::Escalate { .. }),
            "retrying a compile error is superstition"
        );
    }

    #[test]
    fn corruption_evicts_then_escalates_if_it_recurs() {
        let h = DefaultHealer::default();
        let e = CasError::Corrupt { want: Digest::of(b"a"), got: Digest::of(b"b") };
        assert!(matches!(h.on_cas_error(&act(), &e, 0), Remedy::EvictAndRetry { .. }));
        assert!(
            matches!(h.on_cas_error(&act(), &e, 1), Remedy::Escalate { .. }),
            "repeated corruption is a sick disk, not a blip"
        );
    }

    #[test]
    fn platform_fallback_requires_opt_in() {
        let h = DefaultHealer::default();
        let pinned = act().platform(Platform::any().with_accelerator("cuda"));
        let err = ExecError::NoCapablePlatform(pinned.platform.clone());

        assert!(
            matches!(h.on_exec_error(&pinned, &err, 0), Remedy::Escalate { .. }),
            "an autotune must not silently fall back to CPU"
        );

        let opted = pinned.env(FALLBACK_KEY, "1");
        assert!(matches!(h.on_exec_error(&opted, &err, 0), Remedy::Substitute { .. }));
    }

    #[test]
    fn fallback_can_be_disabled_fleet_wide() {
        let h = DefaultHealer::new(HealPolicy {
            allow_platform_fallback: false,
            ..HealPolicy::default()
        });
        let opted = act()
            .platform(Platform::any().with_accelerator("cuda"))
            .env(FALLBACK_KEY, "1");
        let err = ExecError::NoCapablePlatform(opted.platform.clone());
        assert!(matches!(h.on_exec_error(&opted, &err, 0), Remedy::Escalate { .. }));
    }

    #[test]
    fn relaxing_a_platform_changes_the_key() {
        let pinned = act().platform(Platform::any().with_accelerator("cuda"));
        let relaxed = relax_platform(&pinned);
        assert_eq!(relaxed.platform.accelerator, None);
        assert_ne!(
            pinned.key(),
            relaxed.key(),
            "a CPU result must never be servable to a request that asked for CUDA"
        );
    }

    #[test]
    fn missing_tool_escalates_immediately() {
        let h = DefaultHealer::default();
        let e = ExecError::ToolNotFound("ghost@1".into());
        assert!(matches!(h.on_exec_error(&act(), &e, 0), Remedy::Escalate { .. }));
    }

    #[test]
    fn heal_events_serialize_for_agents() {
        let ev = HealEvent {
            action: "compile".into(),
            failure: "transient".into(),
            remedy: Remedy::Retry { attempt: 1, reason: "blip".into() },
        };
        let v: serde_json::Value = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["remedy"]["decision"], "retry");
    }
}
